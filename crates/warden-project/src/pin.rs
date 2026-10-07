//! Fixar versão: `pin = true` no `.pw.toml` (SPEC T11).
//!
//! Item fixado não é atualizado em lote ("Atualizar todos") e mostra o cadeado na lista. Só a
//! linha `pin` do metafile muda; todos os itens pedidos entram numa transação com um único
//! `packwiz refresh`.

use std::collections::BTreeSet;
use std::path::Path;

use warden_core::CancellationToken;
use warden_packwiz::edit::set_metafile_pin;
use warden_packwiz_cli::Packwiz;

use crate::inventory::{ItemState, scan};
use crate::transaction::PackTransaction;
use crate::{Error, ProjectErrorCode as Code, Result};

/// Os caminhos dos itens fixados, em ordem. É a lista que "Atualizar todos" precisa pular.
pub fn pinned_paths(root: &Path) -> Result<Vec<String>> {
    Ok(scan(root)?
        .inventory
        .items
        .into_iter()
        .filter(|item| item.state == ItemState::Ok && item.pinned)
        .map(|item| item.path)
        .collect())
}

/// Fixa ou solta a versão dos metafiles dados (caminhos relativos à pasta do pack). Os que já
/// estão no estado pedido ficam como estão; sem nada a mudar, o packwiz nem é chamado e o
/// resultado é vazio. Devolve os arquivos gravados.
///
/// Erros: [`Code::InvalidInput`] (parâmetro `path`) para caminho que não é um metafile válido
/// do índice; [`Code::PackChangedExternally`] se um arquivo mudou durante a operação.
pub async fn set_pins(
    root: &Path,
    paths: &[String],
    pinned: bool,
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
            .filter(|item| item.state == ItemState::Ok)
            .ok_or_else(|| not_editable(path))?;
        if !seen.insert(item.path.clone()) {
            continue;
        }
        let metafile = scan
            .metafiles
            .get(&item.path)
            .ok_or_else(|| not_editable(path))?;
        if metafile.pin == pinned {
            continue;
        }
        let text = std::fs::read_to_string(root.join(&item.path))
            .map_err(|e| Error::new(Code::Internal, format!("{}: {e}", item.path)))?;
        let edited = set_metafile_pin(&text, pinned)
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
