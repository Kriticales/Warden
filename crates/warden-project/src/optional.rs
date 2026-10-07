//! Mods opcionais: a tabela `[option]` do `.pw.toml` (SPEC T11; CA-T11-02).
//!
//! `optional = true` faz o packwiz-installer perguntar ao jogador (ou, sem janela, seguir o
//! `default`); o Warden faz o mesmo na instância de teste, onde as escolhas ficam à parte
//! (`warden-instance::OptionalChoices`). Só a tabela `[option]` do metafile muda (edição mínima
//! da `warden-packwiz`) e a mudança entra numa transação com um único `packwiz refresh`.
//!
//! Aviso fixo da interface, porque a perda acontece nos outros formatos e não aqui: o app do
//! Modrinth instala todos os opcionais, e o formato da CurseForge não suporta lado.

use std::path::Path;

use serde::{Deserialize, Serialize};
use warden_core::CancellationToken;
use warden_packwiz::ModOption;
use warden_packwiz::edit::set_metafile_option;
use warden_packwiz_cli::Packwiz;

use crate::inventory::{ItemState, scan};
use crate::transaction::PackTransaction;
use crate::{Error, ProjectErrorCode as Code, Result};

/// Tamanho máximo da descrição mostrada ao jogador, em caracteres.
pub const DESCRIPTION_MAX_CHARS: usize = 300;

/// A configuração de opcional de um item, como a interface a edita.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct OptionSettings {
    /// Texto mostrado ao jogador (pode ser vazio).
    pub description: String,
    /// Se vem ligado por padrão.
    pub default: bool,
}

impl OptionSettings {
    fn validated(&self) -> Result<ModOption> {
        let description = self.description.trim();
        let invalid = |reason: &str| {
            Error::new(Code::InvalidInput, format!("descrição do opcional: {reason}"))
                .param("field", "description")
        };
        if description.chars().count() > DESCRIPTION_MAX_CHARS {
            return Err(invalid("longa demais").param("max", DESCRIPTION_MAX_CHARS.to_string()));
        }
        if description.chars().any(char::is_control) {
            return Err(invalid("caractere de controle"));
        }
        Ok(ModOption {
            optional: true,
            description: description.to_owned(),
            default: self.default,
        })
    }
}

impl From<&ModOption> for OptionSettings {
    fn from(option: &ModOption) -> Self {
        Self {
            description: option.description.clone(),
            default: option.default,
        }
    }
}

/// Um item opcional do pack.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct OptionalItem {
    /// Caminho do `.pw.toml` (a chave das escolhas da instância).
    pub path: String,
    /// Nome do item.
    pub name: String,
    /// Texto mostrado ao jogador.
    pub description: String,
    /// Se vem ligado por padrão.
    pub default: bool,
}

/// Os itens opcionais do pack, na ordem da lista de Mods.
pub fn optional_items(root: &Path) -> Result<Vec<OptionalItem>> {
    let scan = scan(root)?;
    Ok(scan
        .inventory
        .items
        .iter()
        .filter(|item| item.state == ItemState::Ok)
        .filter_map(|item| {
            let option = scan.metafiles.get(&item.path)?.option.as_ref()?;
            option.optional.then(|| OptionalItem {
                path: item.path.clone(),
                name: item.name.clone(),
                description: option.description.clone(),
                default: option.default,
            })
        })
        .collect())
}

/// A configuração de opcional de um item (`None`: o item não é opcional).
pub fn option_of(root: &Path, path: &str) -> Result<Option<OptionSettings>> {
    let scan = scan(root)?;
    let item = scan.require(path)?;
    Ok(scan
        .metafiles
        .get(&item.path)
        .and_then(|metafile| metafile.option.as_ref())
        .filter(|option| option.optional)
        .map(OptionSettings::from))
}

/// Marca (`Some`) ou desmarca (`None`) um item como opcional. Devolve os arquivos gravados;
/// vazio quando o metafile já estava assim (o packwiz nem é chamado).
///
/// Erros: [`Code::InvalidInput`] para caminho que não é um metafile válido do índice ou
/// descrição inválida; [`Code::PackChangedExternally`] se o arquivo mudou durante a operação.
pub async fn set_optional(
    root: &Path,
    path: &str,
    settings: Option<&OptionSettings>,
    packwiz: &Packwiz,
    cancel: &CancellationToken,
) -> Result<Vec<String>> {
    let wanted = settings.map(OptionSettings::validated).transpose()?;
    let scan = scan(root)?;
    if let Some(error) = &scan.inventory.index_error {
        return Err(Error::new(Code::InvalidPack, error.clone()));
    }
    let item = scan
        .item(path)
        .filter(|item| item.state == ItemState::Ok)
        .ok_or_else(|| not_editable(path))?;
    let metafile = scan
        .metafiles
        .get(&item.path)
        .ok_or_else(|| not_editable(path))?;
    let current = metafile.option.as_ref().filter(|option| option.optional);
    if current == wanted.as_ref() {
        return Ok(Vec::new());
    }
    let text = std::fs::read_to_string(root.join(&item.path))
        .map_err(|e| Error::new(Code::Internal, format!("{}: {e}", item.path)))?;
    let edited = set_metafile_option(&text, wanted.as_ref())
        .map_err(|e| Error::new(Code::InvalidPack, e.to_string()).param("path", &item.path))?;
    let mut tx = PackTransaction::new(root.to_path_buf());
    tx.write(&item.path, edited.into_bytes())?;
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

    fn settings(description: &str, default: bool) -> OptionSettings {
        OptionSettings {
            description: description.into(),
            default,
        }
    }

    #[test]
    fn valida_a_descricao() {
        assert_eq!(
            settings("  Sombras bonitas  ", true)
                .validated()
                .unwrap()
                .description,
            "Sombras bonitas"
        );
        let long = "a".repeat(DESCRIPTION_MAX_CHARS + 1);
        let error = settings(&long, true).validated().unwrap_err();
        assert_eq!(error.code, Code::InvalidInput);
        assert_eq!(error.params["field"], "description");
        let error = settings("duas\nlinhas", true).validated().unwrap_err();
        assert_eq!(error.params["field"], "description");
    }
}
