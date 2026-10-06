//! Lado de instalação (cliente, servidor ou os dois), de um item ou de vários de uma vez
//! (SPEC T06; CA-T06-03).
//!
//! Cada `.pw.toml` muda só a linha `side` (edição mínima da `warden-packwiz`), e todos entram
//! numa única transação: um só `packwiz refresh` atualiza o índice.

use std::collections::BTreeSet;
use std::path::Path;

use serde::{Deserialize, Serialize};
use warden_core::CancellationToken;
use warden_packwiz::Side;
use warden_packwiz::edit::set_metafile_side;
use warden_packwiz_cli::Packwiz;

use crate::inventory::{ItemState, scan};
use crate::transaction::PackTransaction;
use crate::{Error, ProjectErrorCode as Code, Result};

/// Lado escolhido na interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum SideChoice {
    /// Cliente e servidor.
    Both,
    /// Só cliente.
    Client,
    /// Só servidor.
    Server,
}

impl SideChoice {
    /// O valor gravado no `side`.
    #[must_use]
    pub fn side(self) -> Side {
        match self {
            Self::Both => Side::Both,
            Self::Client => Side::Client,
            Self::Server => Side::Server,
        }
    }

    /// Se o lado atual já é este (`side` ausente conta como os dois).
    #[must_use]
    pub fn matches(self, side: &Side) -> bool {
        match self {
            Self::Both => matches!(side, Side::Both | Side::Unset),
            Self::Client => *side == Side::Client,
            Self::Server => *side == Side::Server,
        }
    }
}

/// Muda o lado dos metafiles dados (caminhos relativos à pasta do pack). Os que já estão no
/// lado pedido ficam como estão; sem nada a mudar, o packwiz nem é chamado e o resultado é
/// vazio. Devolve os arquivos gravados.
///
/// Erros: [`Code::InvalidInput`] (parâmetro `path`) para caminho que não é um metafile válido
/// do índice; [`Code::PackChangedExternally`] se um arquivo mudou durante a operação.
pub async fn set_sides(
    root: &Path,
    paths: &[String],
    side: SideChoice,
    packwiz: &Packwiz,
    cancel: &CancellationToken,
) -> Result<Vec<String>> {
    let scan = scan(root)?;
    if let Some(error) = &scan.inventory.index_error {
        return Err(Error::new(Code::InvalidPack, error.clone()));
    }
    let mut tx = PackTransaction::new(root.to_path_buf());
    let mut changed = false;
    let mut seen = BTreeSet::new();
    for path in paths {
        let item = scan
            .item(path)
            .filter(|item| item.state == ItemState::Ok && item.side_editable)
            .ok_or_else(|| not_editable(path))?;
        if !seen.insert(item.path.clone()) {
            continue;
        }
        let metafile = scan
            .metafiles
            .get(&item.path)
            .ok_or_else(|| not_editable(path))?;
        if side.matches(&metafile.side) {
            continue;
        }
        let text = std::fs::read_to_string(root.join(&item.path))
            .map_err(|e| Error::new(Code::Internal, format!("{}: {e}", item.path)))?;
        let edited = set_metafile_side(&text, &side.side())
            .map_err(|e| Error::new(Code::InvalidPack, e.to_string()).param("path", &item.path))?;
        tx.write(&item.path, edited.into_bytes())?;
        changed = true;
    }
    if !changed {
        return Ok(Vec::new());
    }
    tx.commit(packwiz, cancel).await
}

fn not_editable(path: &str) -> Error {
    Error::new(
        Code::InvalidInput,
        format!("{path} não é um metafile válido do índice"),
    )
    .param("path", path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ausente_conta_como_os_dois() {
        assert!(SideChoice::Both.matches(&Side::Unset));
        assert!(SideChoice::Both.matches(&Side::Both));
        assert!(!SideChoice::Client.matches(&Side::Unset));
        assert!(SideChoice::Server.matches(&Side::Server));
        assert!(!SideChoice::Both.matches(&Side::Other("x".into())));
        assert_eq!(SideChoice::Client.side(), Side::Client);
    }

    #[tokio::test]
    async fn caminho_fora_do_indice_e_recusado_e_nada_a_mudar_nao_chama_o_packwiz() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::write(
            root.join("pack.toml"),
            warden_packwiz::PackManifest::new("T", "1.20.1").to_toml_string(),
        )
        .unwrap();
        let index = warden_packwiz::PackIndex {
            hash_format: "sha256".into(),
            files: vec![warden_packwiz::IndexEntry::new("mods/a.pw.toml", "00")],
        };
        std::fs::write(root.join("index.toml"), index.to_toml_string()).unwrap();
        std::fs::create_dir(root.join("mods")).unwrap();
        let metafile =
            warden_packwiz::Metafile::url("A", "https://example.org/a.jar", "00").unwrap();
        std::fs::write(root.join("mods/a.pw.toml"), metafile.to_toml_string()).unwrap();
        // Binário inexistente: se fosse chamado, a operação falharia.
        let cli = Packwiz::new(
            root.join("inexistente.exe"),
            root.join("cache"),
            root.join("config.toml"),
        );
        let cancel = CancellationToken::new();
        let unchanged = set_sides(
            root,
            &["mods/a.pw.toml".into()],
            SideChoice::Both,
            &cli,
            &cancel,
        )
        .await
        .unwrap();
        assert!(unchanged.is_empty());
        let error = set_sides(
            root,
            &["mods/b.pw.toml".into()],
            SideChoice::Client,
            &cli,
            &cancel,
        )
        .await
        .unwrap_err();
        assert_eq!(error.code, Code::InvalidInput);
        assert_eq!(error.params["path"], "mods/b.pw.toml");
        assert!(!root.join(".packwizignore").exists());
    }
}
