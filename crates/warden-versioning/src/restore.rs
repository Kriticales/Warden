//! "Voltar para esta versão" e "Recuperar" um ponto de segurança, de forma transacional
//! (ARCHITECTURE §11; SPEC T17, CA-T17-01 a CA-T17-03).
//!
//! 1. Confere que o histórico aceita escrita e acha a árvore de destino.
//! 2. Grava um ponto de segurança com a pasta como está (é por ele que o estado anterior pode
//!    ser recuperado depois).
//! 3. Calcula as operações: apagar o que não existe no destino e gravar o que falta ou
//!    difere. Arquivos excluídos pelo `.gitignore` (e fora do histórico) não são tocados.
//! 4. **Preparação:** grava o conteúdo novo de cada arquivo numa pasta de preparação na raiz
//!    do pack (`.warden-restore-<pid>-<n>.warden-tmp`, no mesmo disco), com `fsync`. Uma falha
//!    aqui não mudou nada no pack.
//! 5. **Aplicação:** primeiro as remoções, depois as gravações, só com renomeações: o arquivo
//!    atual vai para a pasta de preparação (reserva) e o novo toma o lugar dele. Pastas que
//!    faltam são criadas e pastas vazias no lugar de um arquivo, removidas, cada uma como um
//!    passo. Um arquivo aberto pelo jogo ou pelo antivírus faz a renomeação falhar
//!    ([`Error::FileInUse`]).
//! 6. **Desfazer:** em qualquer falha, os passos já feitos são revertidos de trás para frente,
//!    renomeando as reservas de volta (bytes idênticos aos de antes). Se uma reserva não puder
//!    voltar, o arquivo é regravado a partir do ponto de segurança. Só se isso também falhar a
//!    operação devolve [`Error::RollbackFailed`], com o nome do ponto de segurança.
//! 7. Sucesso: a pasta de preparação é apagada e as pastas que ficaram vazias, removidas.
//!
//! Não move a branch nem as tags: depois de voltar, "Salvar versão" cria uma versão nova a
//! partir dali.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use git2::Oid;
use serde::{Deserialize, Serialize};
use warden_core::{TEMP_SUFFIX, fault, resolve_inside};

use crate::error::{Error, GitContext as _, Result};
use crate::moment::Moment;
use crate::repo::{Identity, PackRepo};
use crate::safety::{SafetyPoint, SafetyReason};
use crate::worktree::TreeFile;

/// Contador para nomes temporários únicos dentro do processo.
static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Para onde voltar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RestoreTarget {
    /// Uma versão salva (`1.2.0`).
    Version(String),
    /// Um ponto de segurança, pelo nome.
    SafetyPoint(String),
}

/// Resultado de uma restauração bem-sucedida.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RestoreReport {
    /// Ponto de segurança com o estado de antes da restauração (CA-T17-03).
    pub safety_point: SafetyPoint,
    /// Arquivos gravados (novos ou com conteúdo trocado).
    pub written: Vec<String>,
    /// Arquivos apagados.
    pub deleted: Vec<String>,
    /// Temporários que não puderam ser apagados no fim (o antivírus pode segurá-los por um
    /// instante); a limpeza de sobras `.warden-tmp` os apaga depois.
    pub leftover_temp_files: Vec<String>,
}

/// Uma operação sobre um arquivo.
#[derive(Debug)]
enum Op {
    Delete {
        path: String,
        absolute: PathBuf,
    },
    Write {
        path: String,
        absolute: PathBuf,
        oid: Oid,
    },
}

/// Um passo já aplicado, para desfazer.
#[derive(Debug)]
enum Applied {
    /// O arquivo foi para a reserva.
    MovedAside {
        path: String,
        absolute: PathBuf,
        backup: PathBuf,
    },
    /// O conteúdo novo tomou o lugar do arquivo (que não existia ou já está na reserva).
    Placed { path: String, absolute: PathBuf },
    /// Pasta criada para um arquivo novo.
    CreatedDir(PathBuf),
    /// Pasta vazia removida para dar lugar a um arquivo.
    RemovedDir(PathBuf),
}

impl PackRepo {
    /// Volta a pasta do pack para a árvore de uma versão salva ou de um ponto de segurança,
    /// de forma transacional (veja a documentação do módulo).
    ///
    /// Erros: os de [`PackRepo::check_writable`]; [`Error::VersionNotFound`] e
    /// [`Error::SafetyPointNotFound`] antes de qualquer mudança; [`Error::FileInUse`] e erros
    /// de disco com o pack já de volta ao estado anterior; [`Error::RollbackFailed`] se nem o
    /// desfazer funcionou.
    pub fn restore(
        &self,
        target: &RestoreTarget,
        identity: &Identity,
        when: Moment,
    ) -> Result<RestoreReport> {
        self.check_writable()?;
        let (target_commit, reason) = match target {
            RestoreTarget::Version(version) => (
                self.version_commit(version)?,
                SafetyReason::RestoreVersion {
                    version: version.clone(),
                },
            ),
            RestoreTarget::SafetyPoint(name) => {
                let point = self.safety_point(name)?;
                let commit = Oid::from_str(&point.commit).ctx("ler o ponto de segurança")?;
                (
                    commit,
                    SafetyReason::RecoverSafetyPoint {
                        created_at: point.created_at,
                    },
                )
            }
        };
        let target_tree = self
            .repo
            .find_commit(target_commit)
            .ctx("ler o commit de destino")?
            .tree_id();

        let current_tree = self.snapshot_tree()?;
        let safety_point =
            self.create_safety_point_from_tree(current_tree, &reason, identity, when)?;

        let current = self.tree_files(current_tree)?;
        let wanted = self.tree_files(target_tree)?;
        let ops = self.plan(&current, &wanted)?;

        let mut transaction = Transaction {
            repo: self,
            snapshot: &current,
            staging: None,
            staged: Vec::new(),
            applied: Vec::new(),
        };
        let outcome = transaction
            .stage(&ops)
            .and_then(|()| transaction.apply(&ops));
        if let Err(error) = outcome {
            let failed = transaction.rollback();
            return Err(if failed.is_empty() {
                error
            } else {
                Error::RollbackFailed {
                    safety_point: safety_point.name,
                    paths: failed,
                    source: Box::new(error),
                }
            });
        }
        let leftover = transaction.finish(&ops);
        let mut report = RestoreReport {
            safety_point,
            written: Vec::new(),
            deleted: Vec::new(),
            leftover_temp_files: leftover,
        };
        for op in &ops {
            match op {
                Op::Delete { path, .. } => report.deleted.push(path.clone()),
                Op::Write { path, .. } => report.written.push(path.clone()),
            }
        }
        report.deleted.sort();
        Ok(report)
    }

    /// Remoções (as mais fundas primeiro) e depois gravações (em ordem de caminho).
    fn plan(
        &self,
        current: &BTreeMap<String, TreeFile>,
        wanted: &BTreeMap<String, TreeFile>,
    ) -> Result<Vec<Op>> {
        let mut ops = Vec::new();
        for path in current
            .keys()
            .rev()
            .filter(|path| !wanted.contains_key(*path))
        {
            ops.push(Op::Delete {
                absolute: resolve_inside(&self.root, path)?,
                path: path.clone(),
            });
        }
        for (path, file) in wanted {
            if current.get(path).map(|current| current.oid) != Some(file.oid) {
                ops.push(Op::Write {
                    absolute: resolve_inside(&self.root, path)?,
                    path: path.clone(),
                    oid: file.oid,
                });
            }
        }
        Ok(ops)
    }
}

struct Transaction<'a> {
    repo: &'a PackRepo,
    /// Arquivos da pasta antes da restauração (árvore do ponto de segurança).
    snapshot: &'a BTreeMap<String, TreeFile>,
    /// Pasta de preparação, depois de criada.
    staging: Option<PathBuf>,
    /// Conteúdos novos preparados, na ordem das gravações.
    staged: Vec<PathBuf>,
    /// Passos já aplicados, na ordem.
    applied: Vec<Applied>,
}

impl Transaction<'_> {
    /// Caminho novo dentro da pasta de preparação (criada na primeira chamada).
    fn staging_file(&mut self) -> Result<PathBuf> {
        let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = if let Some(dir) = &self.staging {
            dir.clone()
        } else {
            let dir = self.repo.root.join(format!(
                ".warden-restore-{}-{counter}{TEMP_SUFFIX}",
                std::process::id()
            ));
            fs::create_dir(&dir).map_err(|e| Error::io("criar a pasta", &dir, e))?;
            self.staging = Some(dir.clone());
            dir
        };
        Ok(dir.join(format!("{counter}")))
    }

    /// Grava o conteúdo novo de cada arquivo na pasta de preparação.
    fn stage(&mut self, ops: &[Op]) -> Result<()> {
        for op in ops {
            let Op::Write { oid, .. } = op else {
                continue;
            };
            let blob = self
                .repo
                .repo
                .find_blob(*oid)
                .ctx("ler um arquivo da versão")?;
            let temp = self.staging_file()?;
            fault::check("versioning.restore.stage").map_err(|e| Error::io("gravar", &temp, e))?;
            write_new_file(&temp, blob.content()).map_err(|e| Error::io("gravar", &temp, e))?;
            self.staged.push(temp);
        }
        Ok(())
    }

    /// Remoções e depois gravações, só com renomeações.
    fn apply(&mut self, ops: &[Op]) -> Result<()> {
        let mut staged = std::mem::take(&mut self.staged).into_iter();
        for op in ops {
            let absolute = match op {
                Op::Delete { absolute, .. } | Op::Write { absolute, .. } => absolute,
            };
            fault::check("versioning.restore.apply")
                .map_err(|e| Error::io("aplicar a restauração em", absolute, e))?;
            match op {
                Op::Delete { path, absolute } => self.move_aside(path, absolute)?,
                Op::Write { path, absolute, .. } => {
                    let temp = staged.next().ok_or_else(|| {
                        Error::Internal("conteúdo preparado da restauração faltando".into())
                    })?;
                    if let Some(parent) = absolute.parent() {
                        self.create_dirs(parent)?;
                    }
                    match fs::symlink_metadata(absolute) {
                        Ok(meta) if meta.is_dir() => {
                            // Pasta que a versão de destino tem como arquivo: só sai se estiver
                            // vazia (os arquivos dela já foram para a reserva).
                            fs::remove_dir(absolute)
                                .map_err(|e| Error::io("remover a pasta", absolute, e))?;
                            self.applied.push(Applied::RemovedDir(absolute.clone()));
                        }
                        Ok(_) => self.move_aside(path, absolute)?,
                        Err(_) => {}
                    }
                    rename(&temp, absolute).map_err(|e| file_error(path, absolute, e))?;
                    self.applied.push(Applied::Placed {
                        path: path.clone(),
                        absolute: absolute.clone(),
                    });
                }
            }
        }
        Ok(())
    }

    /// Cria as pastas que faltam até `dir`, cada uma como um passo.
    fn create_dirs(&mut self, dir: &Path) -> Result<()> {
        let mut missing = Vec::new();
        let mut current = Some(dir);
        while let Some(path) = current {
            if path == self.repo.root || path.is_dir() {
                break;
            }
            missing.push(path.to_path_buf());
            current = path.parent();
        }
        for path in missing.into_iter().rev() {
            fs::create_dir(&path).map_err(|e| Error::io("criar a pasta", &path, e))?;
            self.applied.push(Applied::CreatedDir(path));
        }
        Ok(())
    }

    fn move_aside(&mut self, path: &str, absolute: &Path) -> Result<()> {
        let backup = self.staging_file()?;
        rename(absolute, &backup).map_err(|e| file_error(path, absolute, e))?;
        self.applied.push(Applied::MovedAside {
            path: path.to_owned(),
            absolute: absolute.to_path_buf(),
            backup,
        });
        Ok(())
    }

    /// Desfaz os passos aplicados e apaga a pasta de preparação. Devolve os caminhos que não
    /// voltaram ao estado anterior.
    fn rollback(&mut self) -> Vec<String> {
        let mut failed = BTreeSet::new();
        while let Some(step) = self.applied.pop() {
            match step {
                Applied::Placed { path, absolute } => {
                    let removed = fault::check("versioning.restore.rollback")
                        .and_then(|()| remove_file(&absolute));
                    if removed.is_err() {
                        failed.insert(path);
                    }
                }
                Applied::MovedAside {
                    path,
                    absolute,
                    backup,
                } => {
                    let back = fault::check("versioning.restore.rollback")
                        .and_then(|()| rename(&backup, &absolute));
                    if back.is_ok() || self.rewrite_from_snapshot(&path, &absolute).is_ok() {
                        failed.remove(&path);
                    } else {
                        failed.insert(path);
                    }
                }
                Applied::CreatedDir(dir) => {
                    // Só sai se estiver vazia; se algo ficou dentro, o caminho já está na
                    // lista de falhas.
                    let _ = fs::remove_dir(&dir);
                }
                Applied::RemovedDir(dir) => {
                    if fs::create_dir(&dir).is_err() && !dir.is_dir() {
                        failed.insert(dir.display().to_string());
                    }
                }
            }
        }
        if let Some(staging) = self.staging.take() {
            // Com falhas, a pasta de preparação guarda as reservas que não voltaram: fica para
            // a pessoa (e o ponto de segurança tem tudo). Sem falhas, a limpeza de sobras
            // `.warden-tmp` apaga o que não sair agora.
            if failed.is_empty() {
                let _ = fs::remove_dir_all(&staging);
            }
        }
        failed.into_iter().collect()
    }

    /// Regrava um arquivo com o conteúdo que tinha antes (árvore do ponto de segurança).
    fn rewrite_from_snapshot(&self, path: &str, absolute: &Path) -> Result<()> {
        let file = self
            .snapshot
            .get(path)
            .ok_or_else(|| Error::Internal(format!("{path} fora do ponto de segurança")))?;
        let blob = self
            .repo
            .repo
            .find_blob(file.oid)
            .ctx("ler um arquivo do ponto de segurança")?;
        if let Some(parent) = absolute.parent() {
            fs::create_dir_all(parent).map_err(|e| Error::io("criar a pasta", parent, e))?;
        }
        warden_core::atomic_write(absolute, blob.content())?;
        Ok(())
    }

    /// Apaga a pasta de preparação e as pastas que ficaram vazias. Devolve o que sobrou.
    fn finish(&mut self, ops: &[Op]) -> Vec<String> {
        let mut leftover = Vec::new();
        if let Some(staging) = self.staging.take()
            && let Err(error) = remove_dir_all(&staging)
            && error.kind() != io::ErrorKind::NotFound
        {
            leftover.push(staging.display().to_string());
        }
        let mut dirs = BTreeSet::new();
        for op in ops {
            if let Op::Delete { absolute, .. } = op {
                let mut current = absolute.parent();
                while let Some(dir) = current {
                    if dir == self.repo.root || !dir.starts_with(&self.repo.root) {
                        break;
                    }
                    dirs.insert(dir.to_path_buf());
                    current = dir.parent();
                }
            }
        }
        // Mais fundas primeiro, só as vazias; uma pasta com algo dentro (inclusive arquivos
        // ignorados) fica.
        let mut dirs: Vec<PathBuf> = dirs.into_iter().collect();
        dirs.sort_by_key(|dir| std::cmp::Reverse(dir.components().count()));
        for dir in dirs {
            let _ = fs::remove_dir(&dir);
        }
        leftover
    }
}

fn write_new_file(path: &Path, contents: &[u8]) -> io::Result<()> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(contents)?;
    file.sync_all()
}

/// Arquivo em uso por outro programa vira [`Error::FileInUse`]; o resto, erro de disco.
fn file_error(path: &str, absolute: &Path, error: io::Error) -> Error {
    if is_in_use(&error) {
        Error::FileInUse {
            path: path.to_owned(),
            source: error,
        }
    } else {
        Error::io("substituir", absolute, error)
    }
}

/// `ERROR_SHARING_VIOLATION` (32), `ERROR_LOCK_VIOLATION` (33) e "acesso negado" (5), que o
/// Windows devolve ao renomear um arquivo aberto sem permissão de remoção compartilhada.
fn is_in_use(error: &io::Error) -> bool {
    cfg!(windows) && matches!(error.raw_os_error(), Some(5 | 32 | 33))
}

/// Renomeia com algumas tentativas curtas no Windows, onde o antivírus e o indexador abrem o
/// arquivo recém-gravado por instantes (mesma regra do `atomic_write` da `warden-core`).
fn rename(from: &Path, to: &Path) -> io::Result<()> {
    retry(|| fs::rename(from, to))
}

/// Apaga uma pasta inteira; arquivos somente leitura (reservas de arquivos assim) têm o
/// atributo tirado antes, porque o Windows não os apaga.
fn remove_dir_all(dir: &Path) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if fs::symlink_metadata(&path)?.is_dir() {
            remove_dir_all(&path)?;
        } else {
            remove_file(&path)?;
        }
    }
    retry(|| fs::remove_dir(dir))
}

/// Apaga um arquivo (já ausente conta como apagado). No Windows, um arquivo somente leitura
/// não pode ser apagado: o atributo é tirado e a remoção, repetida.
fn remove_file(path: &Path) -> io::Result<()> {
    let remove = || match fs::remove_file(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        other => other,
    };
    match retry(remove) {
        Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {
            let Ok(meta) = fs::symlink_metadata(path) else {
                return Err(error);
            };
            let mut permissions = meta.permissions();
            if !permissions.readonly() {
                return Err(error);
            }
            // Só tira o "somente leitura" de um arquivo que o Warden vai apagar em seguida (a
            // reserva de um arquivo já restaurado); no Linux o atributo não impede a remoção.
            #[allow(clippy::permissions_set_readonly_false)]
            permissions.set_readonly(false);
            fs::set_permissions(path, permissions)?;
            retry(remove)
        }
        other => other,
    }
}

fn retry(mut action: impl FnMut() -> io::Result<()>) -> io::Result<()> {
    const ATTEMPTS: u32 = 5;
    let mut attempt = 1;
    loop {
        match action() {
            Ok(()) => return Ok(()),
            Err(error)
                if cfg!(windows)
                    && error.kind() == io::ErrorKind::PermissionDenied
                    && attempt < ATTEMPTS =>
            {
                std::thread::sleep(std::time::Duration::from_millis(20 * u64::from(attempt)));
                attempt += 1;
            }
            Err(error) => return Err(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn so_erros_de_compartilhamento_do_windows_sao_arquivo_em_uso() {
        let sharing = io::Error::from_raw_os_error(32);
        assert_eq!(is_in_use(&sharing), cfg!(windows));
        assert!(!is_in_use(&io::Error::other("x")));
        let error = file_error("a", Path::new("C:/p/a"), io::Error::other("x"));
        assert!(matches!(error, Error::Core(_)));
        let error = file_error("a", Path::new("C:/p/a"), sharing);
        assert_eq!(matches!(error, Error::FileInUse { .. }), cfg!(windows));
    }

    #[test]
    fn apagar_pasta_com_arquivo_somente_leitura() {
        let dir = tempfile::tempdir().unwrap();
        let inner = dir.path().join("preparo");
        fs::create_dir_all(inner.join("sub")).unwrap();
        let file = inner.join("sub").join("reserva");
        fs::write(&file, b"x").unwrap();
        let mut permissions = fs::metadata(&file).unwrap().permissions();
        permissions.set_readonly(true);
        fs::set_permissions(&file, permissions).unwrap();
        remove_dir_all(&inner).unwrap();
        assert!(!inner.exists());
        // Arquivo ausente conta como apagado.
        remove_file(&file).unwrap();
    }
}
