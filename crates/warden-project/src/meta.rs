//! Informações do pack: nome, autor e descrição no `pack.toml` (SPEC T11; CA-T11-01).
//!
//! Só os campos que mudaram são editados, com a edição mínima da `warden-packwiz`: o resto do
//! arquivo fica byte a byte igual. O `pack.toml` não entra no índice, então a gravação não
//! chama o `packwiz refresh`.

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use warden_packwiz::edit::{PackTextField, set_pack_text};
use warden_packwiz::{PACK_FILE, PackManifest};

use crate::transaction::PackTransaction;
use crate::{Error, ProjectErrorCode as Code, Result};

/// Limite do nome e do autor, em caracteres.
pub const MAX_NAME_CHARS: usize = 100;

/// Limite da descrição, em caracteres.
pub const MAX_DESCRIPTION_CHARS: usize = 2000;

/// Informações do pack lidas do `pack.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PackMeta {
    /// Nome.
    pub name: String,
    /// Autor (vazio se ausente).
    pub author: String,
    /// Descrição (vazia se ausente).
    pub description: String,
    /// Versão do pack.
    pub version: String,
    /// Versão do Minecraft.
    pub minecraft: Option<String>,
    /// Chave do loader (`fabric`, `forge`, `neoforge`…), quando há exatamente um.
    pub loader: Option<String>,
    /// Versão desse loader.
    pub loader_version: Option<String>,
}

/// Campos editáveis em "Editar informações".
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MetaUpdate {
    /// Nome (sem espaços nas pontas, não vazio, até 100 caracteres).
    pub name: String,
    /// Autor (até 100 caracteres; vazio remove).
    pub author: String,
    /// Descrição (até 2000 caracteres; vazia remove).
    pub description: String,
}

/// Lê as informações do `pack.toml`. Erro [`Code::InvalidPack`] se ele não puder ser lido.
pub fn read_meta(root: &Path) -> Result<PackMeta> {
    let (_, manifest) = read_manifest(root)?;
    Ok(meta_of(&manifest))
}

/// Grava nome, autor e descrição, mudando só as linhas dos campos alterados. Devolve as
/// informações relidas do disco.
///
/// Erros: [`Code::InvalidInput`] com o parâmetro `field` (`name`, `author` ou `description`) e
/// `reason` (`empty` ou `tooLong`); [`Code::PackChangedExternally`] se o arquivo mudou durante a
/// gravação.
pub async fn update_meta(root: &Path, update: &MetaUpdate) -> Result<PackMeta> {
    let name = update.name.trim();
    let author = update.author.trim();
    let description = update.description.trim();
    if name.is_empty() {
        return Err(invalid("name", "empty"));
    }
    for (field, value, limit) in [
        ("name", name, MAX_NAME_CHARS),
        ("author", author, MAX_NAME_CHARS),
        ("description", description, MAX_DESCRIPTION_CHARS),
    ] {
        if value.chars().count() > limit {
            return Err(invalid(field, "tooLong").param("max", limit.to_string()));
        }
    }
    let (text, manifest) = read_manifest(root)?;
    let mut edited = text.clone();
    for (field, current, wanted) in [
        (PackTextField::Name, manifest.name.as_str(), name),
        (PackTextField::Author, manifest.author.as_str(), author),
        (
            PackTextField::Description,
            manifest.description.as_str(),
            description,
        ),
    ] {
        if current != wanted {
            edited = set_pack_text(&edited, field, wanted)
                .map_err(|e| Error::new(Code::InvalidPack, e.to_string()))?;
        }
    }
    if edited != text {
        let mut tx = PackTransaction::new(root.to_path_buf());
        tx.write(PACK_FILE, edited.into_bytes())?;
        tx.commit_without_refresh().await?;
    }
    read_meta(root)
}

fn invalid(field: &str, reason: &str) -> Error {
    Error::new(Code::InvalidInput, format!("{field}: {reason}"))
        .param("field", field)
        .param("reason", reason)
}

fn read_manifest(root: &Path) -> Result<(String, PackManifest)> {
    let text = fs::read_to_string(root.join(PACK_FILE))
        .map_err(|e| Error::new(Code::InvalidPack, format!("{PACK_FILE}: {e}")))?;
    let manifest = PackManifest::parse(&text)
        .map_err(|e| Error::new(Code::InvalidPack, e.in_file(PACK_FILE).to_string()))?
        .value;
    Ok((text, manifest))
}

fn meta_of(manifest: &PackManifest) -> PackMeta {
    let loaders = manifest.loaders();
    let single = match loaders.as_slice() {
        [(loader, version)] => Some((loader.key().to_owned(), (*version).to_owned())),
        _ => None,
    };
    PackMeta {
        name: manifest.name.clone(),
        author: manifest.author.clone(),
        description: manifest.description.clone(),
        version: manifest.version.clone(),
        minecraft: manifest.minecraft_version().map(str::to_owned),
        loader: single.as_ref().map(|(loader, _)| loader.clone()),
        loader_version: single.map(|(_, version)| version),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use warden_packwiz::Loader;

    fn pack(dir: &Path) -> String {
        let mut manifest = PackManifest::new("Antigo", "1.20.1");
        manifest.author = "Autor".into();
        manifest.description = "Uma descrição".into();
        manifest.set_loader_version(Loader::Fabric, "0.16.14");
        let text = manifest.to_toml_string();
        fs::write(dir.join("pack.toml"), &text).unwrap();
        text
    }

    fn update(name: &str, author: &str, description: &str) -> MetaUpdate {
        MetaUpdate {
            name: name.into(),
            author: author.into(),
            description: description.into(),
        }
    }

    #[test]
    fn le_loader_unico() {
        let dir = tempfile::tempdir().unwrap();
        pack(dir.path());
        let meta = read_meta(dir.path()).unwrap();
        assert_eq!(meta.name, "Antigo");
        assert_eq!(meta.minecraft.as_deref(), Some("1.20.1"));
        assert_eq!(meta.loader.as_deref(), Some("fabric"));
        assert_eq!(meta.loader_version.as_deref(), Some("0.16.14"));
        assert_eq!(
            read_meta(&dir.path().join("nada")).unwrap_err().code,
            Code::InvalidPack
        );
    }

    #[tokio::test]
    async fn valida_campos_sem_gravar() {
        let dir = tempfile::tempdir().unwrap();
        let before = pack(dir.path());
        for (input, field, reason) in [
            (update("  ", "", ""), "name", "empty"),
            (update(&"n".repeat(101), "", ""), "name", "tooLong"),
            (update("Ok", &"a".repeat(101), ""), "author", "tooLong"),
            (
                update("Ok", "", &"d".repeat(2001)),
                "description",
                "tooLong",
            ),
        ] {
            let error = update_meta(dir.path(), &input).await.unwrap_err();
            assert_eq!(error.code, Code::InvalidInput);
            assert_eq!(error.params["field"], field);
            assert_eq!(error.params["reason"], reason);
        }
        assert_eq!(
            fs::read_to_string(dir.path().join("pack.toml")).unwrap(),
            before
        );
        // Limites exatos são aceitos (contados em caracteres, não bytes).
        let meta = update_meta(dir.path(), &update(&"é".repeat(100), "", &"ç".repeat(2000)))
            .await
            .unwrap();
        assert_eq!(meta.name.chars().count(), 100);
    }

    #[tokio::test]
    async fn sem_mudanca_nao_grava_e_descricao_vazia_remove_a_chave() {
        let dir = tempfile::tempdir().unwrap();
        let before = pack(dir.path());
        update_meta(dir.path(), &update(" Antigo ", "Autor", "Uma descrição"))
            .await
            .unwrap();
        assert_eq!(
            fs::read_to_string(dir.path().join("pack.toml")).unwrap(),
            before
        );
        assert!(!dir.path().join(".packwizignore").exists());
        let meta = update_meta(dir.path(), &update("Antigo", "", ""))
            .await
            .unwrap();
        assert_eq!(meta.author, "");
        let text = fs::read_to_string(dir.path().join("pack.toml")).unwrap();
        assert!(!text.contains("author") && !text.contains("description"));
        assert!(text.contains("name = \"Antigo\""));
    }
}
