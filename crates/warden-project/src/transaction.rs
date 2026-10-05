//! Escrita atômica e reversível na pasta de um pack.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use warden_core::{CancellationToken, atomic_write, resolve_inside};
use warden_packwiz::hygiene::ensure_required_block;
use warden_packwiz_cli::{Packwiz, RunContext};

use crate::{Error, ProjectErrorCode as Code, Result};

enum Change {
    Write(Vec<u8>),
    Delete,
}

/// Plano de mutação. O chamador mantém a trava de escrita do pack até `commit` terminar.
pub struct PackTransaction {
    root: PathBuf,
    changes: BTreeMap<PathBuf, Change>,
    expected: BTreeMap<PathBuf, Option<Vec<u8>>>,
    #[cfg(test)]
    fail_after: Option<usize>,
}

impl PackTransaction {
    /// Inicia a transação sobre uma pasta já existente.
    #[must_use]
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            changes: BTreeMap::new(),
            expected: BTreeMap::new(),
            #[cfg(test)]
            fail_after: None,
        }
    }

    /// Planeja escrever bytes em um caminho relativo seguro.
    pub fn write(&mut self, relative: &str, bytes: Vec<u8>) -> Result<()> {
        let path = self.path(relative)?;
        self.remember(&path)?;
        self.changes.insert(path, Change::Write(bytes));
        Ok(())
    }

    /// Planeja apagar um arquivo relativo seguro.
    pub fn delete(&mut self, relative: &str) -> Result<()> {
        let path = self.path(relative)?;
        self.remember(&path)?;
        self.changes.insert(path, Change::Delete);
        Ok(())
    }

    fn path(&self, relative: &str) -> Result<PathBuf> {
        if relative.is_empty() || relative == "." {
            return Err(Error::new(Code::InvalidInput, "caminho vazio"));
        }
        resolve_inside(&self.root, relative)
            .map_err(|e| Error::new(Code::InvalidInput, e.to_string()))
    }

    fn remember(&mut self, path: &Path) -> Result<()> {
        if self.expected.contains_key(path) {
            return Ok(());
        }
        self.expected
            .insert(path.to_path_buf(), read_optional(path)?);
        Ok(())
    }

    fn required_block(&mut self) -> Result<()> {
        let path = self.root.join(".packwizignore");
        let original = match self.changes.get(&path) {
            Some(Change::Write(bytes)) => Some(bytes.clone()),
            _ => read_optional(&path)?,
        };
        let text = original
            .as_deref()
            .map(std::str::from_utf8)
            .transpose()
            .map_err(|e| Error::new(Code::InvalidPack, e.to_string()))?;
        let fixed = ensure_required_block(text);
        if !fixed.added.is_empty() {
            self.write(".packwizignore", fixed.text.into_bytes())?;
        }
        Ok(())
    }

    /// Confere os bytes planejados, grava cada arquivo, atualiza o índice e reverte em falha.
    pub async fn commit(
        self,
        packwiz: &Packwiz,
        cancel: &CancellationToken,
    ) -> Result<Vec<String>> {
        self.commit_inner(Some((packwiz, cancel))).await
    }

    /// Grava arquivos de controle de uma importação sem alterar o índice existente.
    pub async fn commit_without_refresh(self) -> Result<Vec<String>> {
        self.commit_inner(None).await
    }

    async fn commit_inner(
        mut self,
        refresh: Option<(&Packwiz, &CancellationToken)>,
    ) -> Result<Vec<String>> {
        self.required_block()?;
        for (path, expected) in &self.expected {
            if &read_optional(path)? != expected {
                return Err(Error::new(
                    Code::PackChangedExternally,
                    path.display().to_string(),
                ));
            }
        }
        let mut previous = BTreeMap::new();
        let mut touched = Vec::new();
        let mut created_dirs = Vec::new();
        let manifest_path = self.root.join("pack.toml");
        let index_path = fs::read_to_string(&manifest_path)
            .ok()
            .and_then(|text| warden_packwiz::PackManifest::parse(&text).ok())
            .and_then(|pack| resolve_inside(&self.root, &pack.value.index.file).ok())
            .unwrap_or_else(|| self.root.join("index.toml"));
        let refresh_files = [manifest_path, index_path];
        for path in &refresh_files {
            previous.insert(path.clone(), read_optional(path)?);
        }
        for (step, (path, change)) in self.changes.iter().enumerate() {
            if let Some(parent) = path.parent() {
                let mut cursor = parent;
                while cursor.starts_with(&self.root) && cursor != self.root && !cursor.exists() {
                    created_dirs.push(cursor.to_path_buf());
                    cursor = cursor.parent().unwrap_or(&self.root);
                }
            }
            previous.insert(
                path.clone(),
                self.expected.get(path).cloned().unwrap_or(None),
            );
            touched.push(path.clone());
            let result = apply(path, change);
            #[cfg(test)]
            let result = result.and_then(|()| {
                if self.fail_after == Some(step) {
                    Err(Error::new(Code::Internal, "falha injetada"))
                } else {
                    Ok(())
                }
            });
            #[cfg(not(test))]
            let _ = step;
            if let Err(error) = result {
                restore(&previous, &touched);
                remove_created_dirs(&created_dirs);
                return Err(error);
            }
        }
        if let Some((packwiz, cancel)) = refresh
            && let Err(error) = packwiz
                .refresh(&self.root, false, RunContext::new(cancel))
                .await
        {
            let mut rollback = touched.clone();
            rollback.extend(refresh_files);
            restore(&previous, &rollback);
            let _ = packwiz
                .refresh(&self.root, false, RunContext::new(cancel))
                .await;
            restore(&previous, &rollback);
            remove_created_dirs(&created_dirs);
            return Err(Error::new(Code::Internal, error.to_string()));
        }
        Ok(touched
            .iter()
            .filter_map(|p| p.strip_prefix(&self.root).ok())
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .collect())
    }
}

fn read_optional(path: &Path) -> Result<Option<Vec<u8>>> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(Error::new(
            Code::Internal,
            format!("{}: {e}", path.display()),
        )),
    }
}

fn apply(path: &Path, change: &Change) -> Result<()> {
    match change {
        Change::Write(bytes) => {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| Error::new(Code::Internal, e.to_string()))?;
            }
            atomic_write(path, bytes).map_err(|e| Error::new(Code::Internal, e.to_string()))
        }
        Change::Delete => match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(Error::new(Code::Internal, e.to_string())),
        },
    }
}

fn restore(previous: &BTreeMap<PathBuf, Option<Vec<u8>>>, touched: &[PathBuf]) {
    for path in touched.iter().rev() {
        match previous.get(path) {
            Some(Some(bytes)) => {
                let _ = atomic_write(path, bytes);
            }
            Some(None) => {
                let _ = fs::remove_file(path);
            }
            None => {}
        }
    }
}

fn remove_created_dirs(paths: &[PathBuf]) {
    let mut sorted = paths.to_vec();
    sorted.sort_by_key(|path| std::cmp::Reverse(path.components().count()));
    sorted.dedup();
    for path in sorted {
        let _ = fs::remove_dir(path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn reverte_falha_injetada_em_cada_escrita() {
        for failed_step in 0..=3 {
            let dir = tempfile::tempdir().unwrap();
            let root = dir.path();
            fs::write(root.join("a.txt"), b"antes").unwrap();
            let mut tx = PackTransaction::new(root.to_path_buf());
            tx.write("a.txt", b"depois".to_vec()).unwrap();
            tx.write("b.txt", b"novo".to_vec()).unwrap();
            tx.write(".warden/project.toml", b"id = 'x'".to_vec())
                .unwrap();
            tx.fail_after = Some(failed_step);
            let error = tx.commit_without_refresh().await.unwrap_err();
            assert_eq!(error.code, Code::Internal);
            assert_eq!(fs::read(root.join("a.txt")).unwrap(), b"antes");
            assert!(!root.join("b.txt").exists());
            assert!(!root.join(".packwizignore").exists());
            assert!(!root.join(".warden").exists());
        }
    }

    #[tokio::test]
    async fn detecta_alteracao_externa_antes_de_escrever() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::write(root.join("a.txt"), b"antes").unwrap();
        let mut tx = PackTransaction::new(root.to_path_buf());
        tx.write("a.txt", b"depois".to_vec()).unwrap();
        fs::write(root.join("a.txt"), b"externo").unwrap();
        let error = tx.commit_without_refresh().await.unwrap_err();
        assert_eq!(error.code, Code::PackChangedExternally);
        assert_eq!(fs::read(root.join("a.txt")).unwrap(), b"externo");
    }

    #[tokio::test]
    async fn falha_do_refresh_restaura_manifesto_indice_e_arquivos() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let manifest = warden_packwiz::PackManifest::new("Teste", "1.20.1").to_toml_string();
        let index = warden_packwiz::PackIndex::default().to_toml_string();
        fs::write(root.join("pack.toml"), &manifest).unwrap();
        fs::write(root.join("index.toml"), &index).unwrap();
        let mut tx = PackTransaction::new(root.to_path_buf());
        tx.write("config/teste.txt", b"novo".to_vec()).unwrap();
        let cli = Packwiz::new(
            root.join("inexistente.exe"),
            root.join("cache"),
            root.join("config.toml"),
        );
        assert!(tx.commit(&cli, &CancellationToken::new()).await.is_err());
        assert_eq!(
            fs::read_to_string(root.join("pack.toml")).unwrap(),
            manifest
        );
        assert_eq!(fs::read_to_string(root.join("index.toml")).unwrap(), index);
        assert!(!root.join("config").exists());
        assert!(!root.join(".packwizignore").exists());
    }
}
