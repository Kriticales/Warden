//! O que ficou para revisar depois de importar (SPEC T24: o pack abre "com o aviso 'N itens
//! para revisar' em Problemas").
//!
//! A importação grava os itens na tabela `[import]` de `.warden/project.toml` (`review` e
//! `local-files`; veja [`crate::apply`]). Aqui eles são lidos de volta para o bloco de
//! Problemas. Itens que a pessoa já tirou do pack não contam mais.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::model::ImportReviewReason;

/// Arquivo do Warden dentro do pack.
const PROJECT_FILE: &str = ".warden/project.toml";

/// Por que um item do pack importado pede revisão.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum ImportPendingReason {
    /// `env` fora da especificação: entrou como Cliente e servidor.
    UnknownSide,
    /// Opcional no modpack: entrou opcional e marcado.
    Optional,
    /// Opcional desligado no modpack: entrou opcional e desmarcado.
    OptionalDisabled,
    /// Veio de `client-overrides/`: entrou para todos.
    ClientOverride,
    /// Jar que não está em nenhuma loja: ficou como arquivo local (confira a licença).
    LocalFile,
}

impl From<ImportReviewReason> for ImportPendingReason {
    fn from(reason: ImportReviewReason) -> Self {
        match reason {
            ImportReviewReason::UnknownSide => Self::UnknownSide,
            ImportReviewReason::Optional => Self::Optional,
            ImportReviewReason::OptionalDisabled => Self::OptionalDisabled,
            ImportReviewReason::ClientOverride => Self::ClientOverride,
        }
    }
}

/// Um item para revisar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ImportPendingItem {
    /// Caminho no pack (`mods/x.pw.toml`, `mods/x.jar`, `config/y.toml`).
    pub path: String,
    /// Nome para a tela (o do mod, ou o do arquivo).
    pub name: String,
    /// Motivo.
    pub reason: ImportPendingReason,
}

/// Os itens para revisar de um pack importado.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ImportPending {
    /// Nome do arquivo ou da pasta de origem.
    pub file: String,
    /// Itens, na ordem gravada (revisão primeiro, depois os arquivos locais).
    pub items: Vec<ImportPendingItem>,
}

#[derive(Deserialize)]
struct Stored {
    import: Option<StoredImport>,
}

#[derive(Deserialize)]
struct StoredImport {
    #[serde(default)]
    file: String,
    #[serde(default)]
    review: Vec<StoredReview>,
    #[serde(default, rename = "local-files")]
    local_files: Vec<StoredLocal>,
}

#[derive(Deserialize)]
struct StoredReview {
    path: String,
    #[serde(default)]
    name: String,
    reason: ImportReviewReason,
}

#[derive(Deserialize)]
struct StoredLocal {
    path: String,
}

fn file_name(path: &str) -> String {
    path.rsplit('/').next().unwrap_or(path).to_owned()
}

/// Os itens para revisar do pack em `root`, ou `None` se o pack não foi importado ou não tem
/// nada para revisar. Itens cujo arquivo não existe mais no pack ficam de fora; caminhos que
/// sairiam do pack são ignorados.
///
/// Erros: [`crate::ImportErrorCode::Internal`] para `.warden/project.toml` ilegível.
pub fn pending_review(root: &Path) -> Result<Option<ImportPending>> {
    let text = match std::fs::read_to_string(root.join(PROJECT_FILE)) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(Error::internal(format!("{PROJECT_FILE}: {}", error.kind())));
        }
    };
    let stored: Stored = toml::from_str(&text)
        .map_err(|error| Error::internal(format!("{PROJECT_FILE}: {}", error.message())))?;
    let Some(import) = stored.import else {
        return Ok(None);
    };
    let exists =
        |path: &str| warden_core::resolve_inside(root, path).is_ok_and(|full| full.is_file());
    let mut items: Vec<ImportPendingItem> = import
        .review
        .into_iter()
        .filter(|item| exists(&item.path))
        .map(|item| ImportPendingItem {
            name: if item.name.trim().is_empty() {
                file_name(&item.path)
            } else {
                item.name
            },
            path: item.path,
            reason: item.reason.into(),
        })
        .collect();
    for local in import.local_files {
        if exists(&local.path) && !items.iter().any(|item| item.path == local.path) {
            items.push(ImportPendingItem {
                name: file_name(&local.path),
                path: local.path,
                reason: ImportPendingReason::LocalFile,
            });
        }
    }
    if items.is_empty() {
        return Ok(None);
    }
    Ok(Some(ImportPending {
        file: import.file,
        items,
    }))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use std::fs;

    use super::*;

    fn pack(project: &str, files: &[&str]) -> tempfile::TempDir {
        let temp = tempfile::tempdir().unwrap();
        fs::create_dir_all(temp.path().join(".warden")).unwrap();
        fs::write(temp.path().join(PROJECT_FILE), project).unwrap();
        for file in files {
            let path = temp.path().join(file);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, b"x").unwrap();
        }
        temp
    }

    const PROJECT: &str = r#"format = 1

[import]
source = "mrpack"
file = "Create+ 6.0.0.mrpack"

[[import.local-files]]
path = "mods/caseiro.jar"
sha256 = "aa"

[[import.review]]
path = "mods/create.pw.toml"
name = "Create"
reason = "unknownSide"

[[import.review]]
path = "config/cliente.toml"
reason = "clientOverride"

[[import.review]]
path = "mods/removido.pw.toml"
name = "Removido"
reason = "optional"
"#;

    #[test]
    fn le_os_itens_e_deixa_de_fora_os_que_sairam_do_pack() {
        let temp = pack(
            PROJECT,
            &[
                "mods/create.pw.toml",
                "config/cliente.toml",
                "mods/caseiro.jar",
            ],
        );
        let pending = pending_review(temp.path()).unwrap().unwrap();
        assert_eq!(pending.file, "Create+ 6.0.0.mrpack");
        assert_eq!(
            pending.items,
            vec![
                ImportPendingItem {
                    path: "mods/create.pw.toml".into(),
                    name: "Create".into(),
                    reason: ImportPendingReason::UnknownSide,
                },
                ImportPendingItem {
                    path: "config/cliente.toml".into(),
                    name: "cliente.toml".into(),
                    reason: ImportPendingReason::ClientOverride,
                },
                ImportPendingItem {
                    path: "mods/caseiro.jar".into(),
                    name: "caseiro.jar".into(),
                    reason: ImportPendingReason::LocalFile,
                },
            ]
        );
    }

    #[test]
    fn sem_importacao_ou_sem_itens_nao_ha_aviso() {
        let temp = pack("format = 1\n", &[]);
        assert_eq!(pending_review(temp.path()).unwrap(), None);
        let temp = pack(PROJECT, &[]);
        assert_eq!(pending_review(temp.path()).unwrap(), None);
        let temp = tempfile::tempdir().unwrap();
        assert_eq!(pending_review(temp.path()).unwrap(), None);
        let temp = pack(
            "[import]\nfile = \"x\"\n[[import.review]]\npath = \"../fora.toml\"\nreason = \"optional\"\n",
            &[],
        );
        fs::write(temp.path().parent().unwrap().join("fora.toml"), b"x").ok();
        assert_eq!(pending_review(temp.path()).unwrap(), None);
    }

    #[test]
    fn arquivo_ilegivel_e_erro_sem_a_pasta() {
        let temp = pack("[import\n", &[]);
        let error = pending_review(temp.path()).unwrap_err();
        assert!(
            !error
                .to_string()
                .contains(&temp.path().display().to_string())
        );
    }
}
