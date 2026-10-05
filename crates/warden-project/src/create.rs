//! Criação de um pack novo sem `packwiz init`.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use warden_catalog::Loader;
use warden_core::{CancellationToken, PackId};
use warden_packwiz::hygiene::{GITATTRIBUTES_TEMPLATE, gitignore_template, packwizignore_template};
use warden_packwiz::{PackIndex, PackManifest, slugify_name};
use warden_packwiz_cli::Packwiz;
use warden_versioning::{Identity, InitialPoint, Moment, PackRepo};

use crate::registry::{PackRecord, Registry};
use crate::transaction::PackTransaction;
use crate::{Error, ProjectErrorCode as Code, Result};

/// Pedido final do assistente de criação. Mods iniciais são acrescentados pela P1-18.
#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CreatePack {
    /// Nome, de 1 a 64 caracteres.
    pub name: String,
    /// Autor.
    pub author: String,
    /// Descrição opcional.
    pub description: String,
    /// Pasta escolhida; vazia significa usar a pasta padrão e o slug do nome.
    pub destination: Option<PathBuf>,
    /// Id exato do manifesto da Mojang.
    pub minecraft: String,
    /// `None` = vanilla.
    pub loader: Option<Loader>,
    /// Versão exata do loader, obrigatória se houver loader.
    pub loader_version: Option<String>,
}

/// Resultado de uma criação.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CreatedPack {
    /// Identificador estável.
    pub id: PackId,
    /// Pasta final.
    pub path: PathBuf,
}

/// Confere campos e destino sem escrever nada.
pub fn validate(request: &CreatePack, default_dir: &Path) -> Result<PathBuf> {
    let name = request.name.trim();
    if name.is_empty() || name.chars().count() > 64 || name.chars().any(char::is_control) {
        return Err(Error::new(
            Code::InvalidInput,
            "nome deve ter de 1 a 64 caracteres",
        ));
    }
    if request.minecraft.trim().is_empty()
        || request.minecraft.len() > 64
        || request.minecraft.chars().any(char::is_control)
    {
        return Err(Error::new(
            Code::InvalidInput,
            "versão do Minecraft inválida",
        ));
    }
    if request.author.len() > 256 || request.description.len() > 4096 {
        return Err(Error::new(
            Code::InvalidInput,
            "autor ou descrição longo demais",
        ));
    }
    if request.loader.is_some() != request.loader_version.is_some()
        || request
            .loader_version
            .as_deref()
            .is_some_and(|v| v.trim().is_empty() || v.len() > 128)
    {
        return Err(Error::new(
            Code::InvalidLoaderVersion,
            "loader e versão precisam ser escolhidos juntos",
        ));
    }
    let slug = slugify_name(name);
    if slug.is_empty() {
        return Err(Error::new(
            Code::InvalidInput,
            "nome não gera uma pasta válida",
        ));
    }
    let path = request
        .destination
        .clone()
        .unwrap_or_else(|| default_dir.join(slug));
    if !path.is_absolute() || path.parent().is_none() {
        return Err(Error::new(Code::InvalidInput, "destino deve ser absoluto"));
    }
    match fs::read_dir(&path) {
        Ok(entries) => {
            if entries.count() > 0 {
                return Err(
                    Error::new(Code::DestinationNotEmpty, path.display().to_string())
                        .param("path", path.display().to_string()),
                );
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(Error::new(Code::Internal, e.to_string())),
    }
    Ok(path)
}

/// Cria em pasta irmã temporária, valida com o packwiz, grava o ponto inicial e só então publica.
pub async fn create(
    request: &CreatePack,
    default_dir: &Path,
    registry: &Registry,
    packwiz: &Packwiz,
    cancel: &CancellationToken,
) -> Result<CreatedPack> {
    let path = validate(request, default_dir)?;
    let parent = path
        .parent()
        .ok_or_else(|| Error::new(Code::InvalidInput, "destino sem pasta pai"))?;
    fs::create_dir_all(parent).map_err(|e| Error::new(Code::Internal, e.to_string()))?;
    let id = PackId::new();
    let stage = parent.join(format!(".warden-creating-{id}"));
    fs::create_dir(&stage).map_err(|e| Error::new(Code::Internal, e.to_string()))?;
    let result = async {
        let mut manifest = PackManifest::new(request.name.trim(), &request.minecraft);
        manifest.author = request.author.trim().to_owned();
        manifest.description = request.description.trim().to_owned();
        "0.1.0".clone_into(&mut manifest.version);
        if let (Some(loader), Some(version)) = (request.loader, &request.loader_version) {
            let key = match loader {
                Loader::Forge => "forge",
                Loader::NeoForge => "neoforge",
                Loader::Fabric => "fabric",
            };
            manifest
                .versions
                .get_or_insert_with(BTreeMap::new)
                .insert(key.to_owned(), version.clone());
        }
        let mut tx = PackTransaction::new(stage.clone());
        tx.write("pack.toml", manifest.to_toml_string().into_bytes())?;
        tx.write(
            "index.toml",
            PackIndex::default().to_toml_string().into_bytes(),
        )?;
        tx.write(".gitattributes", GITATTRIBUTES_TEMPLATE.as_bytes().to_vec())?;
        tx.write(".packwizignore", packwizignore_template().into_bytes())?;
        tx.write(".gitignore", gitignore_template().into_bytes())?;
        tx.write(".warden/project.toml", project_toml(id).into_bytes())?;
        tx.write("CHANGELOG.md", b"# Historico de versoes\n".to_vec())?;
        tx.commit(packwiz, cancel).await?;
        PackRepo::init(
            &stage,
            InitialPoint::Created,
            &Identity::for_pack(&request.author, None),
            Moment::now_utc(),
        )
        .map_err(|e| Error::new(Code::Internal, e.to_string()))?;
        let existed = path.is_dir();
        if existed {
            fs::remove_dir(&path).map_err(|e| Error::new(Code::Internal, e.to_string()))?;
        }
        if let Err(e) = fs::rename(&stage, &path) {
            if existed {
                let _ = fs::create_dir(&path);
            }
            return Err(Error::new(Code::Internal, e.to_string()));
        }
        let record = PackRecord {
            id,
            name: request.name.trim().to_owned(),
            path: path.clone(),
            last_test: None,
            extra: BTreeMap::new(),
        };
        if let Err(e) = registry.insert(record) {
            let _ = fs::rename(&path, &stage);
            if existed {
                let _ = fs::create_dir(&path);
            }
            return Err(e);
        }
        Ok(CreatedPack { id, path })
    }
    .await;
    if result.is_err() {
        let _ = fs::remove_dir_all(&stage);
    }
    result
}

/// Documento mínimo do Warden. Leitores futuros preservam as demais tabelas.
#[must_use]
pub fn project_toml(id: PackId) -> String {
    format!("schemaVersion = 1\nid = \"{id}\"\n")
}
