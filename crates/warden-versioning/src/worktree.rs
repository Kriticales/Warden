//! Árvore de trabalho e árvores gravadas: alterações não salvas, retrato completo da pasta e
//! leitura de arquivos de uma versão.
//!
//! "Alterações não salvas" é a diferença entre a árvore de trabalho e o último commit
//! (`HEAD`), como o `git diff HEAD` com os arquivos novos: o índice só serve de cache, então
//! um índice deixado diferente por fora não muda o resultado. Arquivos excluídos pelo
//! `.gitignore` não contam, a menos que já estejam no histórico.

use std::collections::BTreeMap;

use git2::{Delta, DiffOptions, FileMode, ObjectType, Oid, Tree, TreeWalkMode, TreeWalkResult};
use serde::{Deserialize, Serialize};

use crate::error::{Error, GitContext as _, Result};
use crate::repo::PackRepo;

/// O que aconteceu com um arquivo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum FileChangeKind {
    /// Arquivo novo.
    Added,
    /// Conteúdo alterado.
    Modified,
    /// Arquivo apagado.
    Deleted,
}

/// Um arquivo alterado, com o caminho relativo à raiz do pack (separador `/`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FileChange {
    /// Caminho relativo (separador `/`).
    pub path: String,
    /// O que aconteceu.
    pub kind: FileChangeKind,
}

/// Arquivo de uma árvore gravada.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TreeFile {
    pub(crate) oid: Oid,
    pub(crate) mode: i32,
}

/// De onde vem um lado de uma comparação.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Side {
    /// Nada (antes da primeira versão).
    Empty,
    /// Uma árvore gravada no repositório.
    Tree(Oid),
    /// A pasta do pack como está agora.
    WorkingTree,
}

impl PackRepo {
    /// Alterações não salvas: diferença entre a árvore de trabalho e o último commit, em
    /// ordem de caminho. Vazio quando não há nada para salvar.
    pub fn unsaved_changes(&self) -> Result<Vec<FileChange>> {
        let head_tree = self.head_tree_id()?;
        self.diff(head_tree.map_or(Side::Empty, Side::Tree), Side::WorkingTree)
    }

    /// Diferença entre dois lados, em ordem de caminho.
    pub(crate) fn diff(&self, old: Side, new: Side) -> Result<Vec<FileChange>> {
        let old_tree = self.side_tree(old)?;
        let mut options = DiffOptions::new();
        options
            .include_untracked(true)
            .recurse_untracked_dirs(true)
            .include_ignored(false)
            .include_typechange(true)
            .ignore_submodules(true);
        let diff = match new {
            Side::WorkingTree => {
                // Caminho rápido: com o índice igual à árvore (o normal depois de salvar), a
                // libgit2 usa a data e o tamanho guardados no índice e só lê os arquivos que
                // mudaram. Com o índice diferente, juntar "árvore → índice" e "índice → pasta"
                // erra arquivos novos (um arquivo fora do índice aparece como apagado), então
                // a comparação é direta, lendo todos os arquivos.
                let mut index = self.fresh_index()?;
                let index_tree = if index.has_conflicts() {
                    None
                } else {
                    index.write_tree().ok()
                };
                match &old_tree {
                    Some(tree) if index_tree == Some(tree.id()) => self
                        .repo
                        .diff_tree_to_workdir_with_index(Some(tree), Some(&mut options))
                        .ctx("comparar com a pasta do pack")?,
                    _ => self
                        .repo
                        .diff_tree_to_workdir(old_tree.as_ref(), Some(&mut options))
                        .ctx("comparar com a pasta do pack")?,
                }
            }
            Side::Empty | Side::Tree(_) => {
                let new_tree = self.side_tree(new)?;
                self.repo
                    .diff_tree_to_tree(old_tree.as_ref(), new_tree.as_ref(), Some(&mut options))
                    .ctx("comparar versões")?
            }
        };
        let mut changes = Vec::new();
        for delta in diff.deltas() {
            let kind = match delta.status() {
                Delta::Added | Delta::Untracked => FileChangeKind::Added,
                Delta::Deleted => FileChangeKind::Deleted,
                Delta::Modified | Delta::Typechange => FileChangeKind::Modified,
                Delta::Unmodified | Delta::Ignored => continue,
                other => {
                    return Err(Error::Internal(format!(
                        "estado de diferença inesperado: {other:?}"
                    )));
                }
            };
            let file = match kind {
                FileChangeKind::Deleted => delta.old_file(),
                FileChangeKind::Added | FileChangeKind::Modified => delta.new_file(),
            };
            let path = file
                .path_bytes()
                .ok_or_else(|| Error::Internal("diferença sem caminho".into()))?;
            changes.push(FileChange {
                path: path_text(path)?,
                kind,
            });
        }
        changes.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(changes)
    }

    /// Grava no repositório uma árvore com a pasta do pack como está agora (todos os arquivos
    /// que o `.gitignore` não exclui, mais os que já estão no histórico), sem mexer no índice
    /// nem em nenhuma referência. Base dos pontos de segurança.
    pub(crate) fn snapshot_tree(&self) -> Result<Oid> {
        let head_tree = self.head_tree_id()?;
        let base_id = if let Some(id) = head_tree {
            id
        } else {
            self.repo
                .treebuilder(None)
                .ctx("montar a árvore vazia")?
                .write()
                .ctx("gravar a árvore vazia")?
        };
        let base = self.repo.find_tree(base_id).ctx("ler a árvore do HEAD")?;
        let changes = self.diff(head_tree.map_or(Side::Empty, Side::Tree), Side::WorkingTree)?;
        if changes.is_empty() {
            return Ok(base.id());
        }
        // Em duas passadas: a libgit2 não troca uma pasta por um arquivo (ou o contrário) na
        // mesma atualização, então as remoções vêm antes.
        let mut removals = git2::build::TreeUpdateBuilder::new();
        let mut upserts = git2::build::TreeUpdateBuilder::new();
        for change in &changes {
            match change.kind {
                FileChangeKind::Deleted => {
                    removals.remove(change.path.as_str());
                }
                FileChangeKind::Added | FileChangeKind::Modified => {
                    let absolute = warden_core::resolve_inside(&self.root, &change.path)?;
                    let blob = self
                        .repo
                        .blob_path(&absolute)
                        .ctx("guardar um arquivo no ponto de segurança")?;
                    upserts.upsert(change.path.as_str(), blob, FileMode::Blob);
                }
            }
        }
        let without_removed = removals
            .create_updated(&self.repo, &base)
            .ctx("gravar a árvore do ponto de segurança")?;
        let without_removed = self
            .repo
            .find_tree(without_removed)
            .ctx("ler a árvore do ponto de segurança")?;
        upserts
            .create_updated(&self.repo, &without_removed)
            .ctx("gravar a árvore do ponto de segurança")
    }

    /// Árvore do commit do HEAD (`None` sem commits).
    pub(crate) fn head_tree_id(&self) -> Result<Option<Oid>> {
        Ok(self.head_commit()?.map(|commit| commit.tree_id()))
    }

    fn side_tree(&self, side: Side) -> Result<Option<Tree<'_>>> {
        match side {
            Side::Tree(id) => Ok(Some(self.repo.find_tree(id).ctx("ler uma árvore")?)),
            Side::Empty | Side::WorkingTree => Ok(None),
        }
    }

    /// Todos os arquivos de uma árvore gravada, por caminho (sem submódulos).
    pub(crate) fn tree_files(&self, tree: Oid) -> Result<BTreeMap<String, TreeFile>> {
        let tree = self.repo.find_tree(tree).ctx("ler uma árvore")?;
        let mut files = BTreeMap::new();
        let mut failure = None;
        tree.walk(TreeWalkMode::PreOrder, |dir, entry| {
            if entry.kind() != Some(ObjectType::Blob) {
                return TreeWalkResult::Ok;
            }
            let mut path = dir.as_bytes().to_vec();
            path.extend_from_slice(entry.name_bytes());
            match path_text(&path) {
                Ok(path) => {
                    files.insert(
                        path,
                        TreeFile {
                            oid: entry.id(),
                            mode: entry.filemode(),
                        },
                    );
                    TreeWalkResult::Ok
                }
                Err(error) => {
                    failure = Some(error);
                    TreeWalkResult::Abort
                }
            }
        })
        .ctx("percorrer uma árvore")?;
        match failure {
            Some(error) => Err(error),
            None => Ok(files),
        }
    }
}

/// Caminho do git (bytes UTF-8) como texto. Caminhos que não são UTF-8 não aparecem em packs
/// gravados no Windows; se aparecerem, a operação para em vez de adivinhar.
pub(crate) fn path_text(bytes: &[u8]) -> Result<String> {
    std::str::from_utf8(bytes)
        .map(ToOwned::to_owned)
        .map_err(|_| {
            Error::Internal(format!(
                "caminho que não é UTF-8 no repositório: {}",
                String::from_utf8_lossy(bytes)
            ))
        })
}
