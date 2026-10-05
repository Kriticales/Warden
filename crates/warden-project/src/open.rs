//! Inspeção e entrada de packs packwiz existentes.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;
use toml_edit::{DocumentMut, value};
use warden_core::{PackId, remove_temp_files};
use warden_packwiz::hygiene::{GITATTRIBUTES_TEMPLATE, gitignore_template, packwizignore_template};
use warden_packwiz::{Loader, read_pack};
use warden_versioning::{Identity, InitialPoint, Moment, PackRepo};

use crate::create::project_toml;
use crate::hygiene::{HygieneFinding, findings};
use crate::registry::{PackRecord, Registry};
use crate::transaction::PackTransaction;
use crate::{Error, ProjectErrorCode as Code, Result};

/// Diagnóstico de uma pasta antes de importar, sem alterações no disco.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ImportPreview {
    /// Pasta proposta.
    pub path: PathBuf,
    /// Nome do pack.
    pub name: String,
    /// Minecraft.
    pub minecraft: Option<String>,
    /// Loader ou `null` para vanilla.
    pub loader: Option<String>,
    /// Arquivos que a higiene propõe limpar, com tamanho e motivo (SPEC T04, passo 3).
    pub hygiene: Vec<HygieneFinding>,
    /// Controles padrão ausentes; o bloco obrigatório é aplicado sempre.
    pub missing_controls: Vec<String>,
    /// Proposta para cada arquivo de controle ausente ou diferente.
    pub control_diffs: Vec<ControlDiff>,
    /// Git existente em estado que impede alterações.
    pub read_only_reason: Option<String>,
}

/// Mudança proposta em um arquivo de controle, apresentada antes de aplicar.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ControlDiff {
    /// Caminho relativo.
    pub path: String,
    /// Conteúdo atual, ou `null` se não existe.
    pub current: Option<String>,
    /// Conteúdo proposto, preservando padrões adicionais do usuário.
    pub suggested: String,
}

/// Resultado da importação.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ImportedPack {
    /// Identificador registrado.
    pub id: PackId,
    /// Pasta registrada.
    pub path: PathBuf,
    /// Um ID de uma cópia foi substituído.
    pub copied_id_replaced: bool,
    /// Linhas acrescentadas ao bloco obrigatório.
    pub required_ignore_added: Vec<String>,
    /// Estado somente leitura do git.
    pub read_only_reason: Option<String>,
}

/// Lê o identificador de `.warden/project.toml` sem descartar tabelas futuras.
pub fn project_id(path: &Path) -> Result<Option<PackId>> {
    let text = match fs::read_to_string(path.join(".warden/project.toml")) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(Error::new(Code::InvalidPack, e.to_string())),
    };
    let doc = text
        .parse::<DocumentMut>()
        .map_err(|e| Error::new(Code::InvalidPack, format!(".warden/project.toml: {e}")))?;
    let id = doc
        .get("id")
        .and_then(|value| value.as_str())
        .ok_or_else(|| Error::new(Code::InvalidPack, ".warden/project.toml sem id"))?
        .parse()
        .map_err(|e| Error::new(Code::InvalidPack, format!("id inválido: {e}")))?;
    Ok(Some(id))
}

/// Confere o manifesto e o loader antes de qualquer escrita.
pub fn preview(path: &Path) -> Result<ImportPreview> {
    let pack = read_pack(path).map_err(|e| Error::new(Code::InvalidPack, e.to_string()))?;
    let loaders = pack.pack.value.loaders();
    if loaders.len() > 1 {
        return Err(
            Error::new(Code::UnsupportedLoader, "o pack contém mais de um loader")
                .param("reason", "multiple"),
        );
    }
    if let Some((loader, _)) = loaders.first()
        && matches!(loader, Loader::Quilt | Loader::LiteLoader)
    {
        return Err(Error::new(
            Code::UnsupportedLoader,
            format!("{} ainda não é suportado", loader.key()),
        )
        .param("loader", loader.key()));
    }
    let unknown_loader = pack.pack.value.versions.as_ref().is_some_and(|versions| {
        versions
            .keys()
            .any(|key| key != "minecraft" && !Loader::ALL.iter().any(|loader| loader.key() == key))
    });
    if unknown_loader {
        return Err(
            Error::new(Code::UnsupportedLoader, "loader desconhecido").param("reason", "unknown")
        );
    }
    let hygiene = findings(path, pack.index.as_ref().ok().map(|index| &index.value))?;
    let missing_controls = [".gitattributes", ".packwizignore", ".gitignore"]
        .into_iter()
        .filter(|name| !path.join(name).exists())
        .map(str::to_owned)
        .collect();
    let control_diffs = control_diffs(path)?;
    let read_only_reason = PackRepo::open(path)
        .ok()
        .and_then(|repo| repo.check_writable().err().map(|e| e.to_string()));
    Ok(ImportPreview {
        path: path.to_path_buf(),
        name: pack.pack.value.name.clone(),
        minecraft: pack.pack.value.minecraft_version().map(str::to_owned),
        loader: loaders.first().map(|(loader, _)| loader.key().to_owned()),
        hygiene,
        missing_controls,
        control_diffs,
        read_only_reason,
    })
}

fn control_diffs(path: &Path) -> Result<Vec<ControlDiff>> {
    let mut result = Vec::new();
    for (name, template) in [
        (".gitattributes", GITATTRIBUTES_TEMPLATE.to_owned()),
        (".packwizignore", packwizignore_template()),
        (".gitignore", gitignore_template()),
    ] {
        let current = match fs::read_to_string(path.join(name)) {
            Ok(text) => Some(text),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(Error::new(Code::InvalidPack, format!("{name}: {e}"))),
        };
        let suggested = if name == ".gitattributes" || current.is_none() {
            template
        } else {
            append_missing_lines(current.as_deref().unwrap_or_default(), &template)
        };
        if current.as_deref() != Some(suggested.as_str()) {
            result.push(ControlDiff {
                path: name.to_owned(),
                current,
                suggested,
            });
        }
    }
    Ok(result)
}

fn append_missing_lines(current: &str, template: &str) -> String {
    let present: std::collections::BTreeSet<_> = current.lines().map(str::trim).collect();
    let mut text = current.to_owned();
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    for line in template.lines() {
        if line.starts_with('#') || line.is_empty() || present.contains(line) {
            continue;
        }
        text.push_str(line);
        text.push('\n');
    }
    text
}

/// Registra uma pasta. `add_controls` inclui modelos ausentes; com `false`, só o bloco obrigatório.
pub async fn import(path: &Path, add_controls: bool, registry: &Registry) -> Result<ImportedPack> {
    let inspection = preview(path)?;
    if registry
        .records()
        .iter()
        .any(|p| p.path.canonicalize().ok() == path.canonicalize().ok())
    {
        return Err(Error::new(
            Code::AlreadyRegistered,
            path.display().to_string(),
        ));
    }
    let project_path = path.join(".warden/project.toml");
    let old_project = match fs::read_to_string(&project_path) {
        Ok(text) => Some(text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(Error::new(Code::Internal, e.to_string())),
    };
    let original_id = old_project
        .as_deref()
        .and_then(|text| text.parse::<DocumentMut>().ok())
        .and_then(|doc| doc.get("id")?.as_str()?.parse::<PackId>().ok());
    let copied = original_id.is_some_and(|id| registry.records().iter().any(|p| p.id == id));
    let id = if copied {
        PackId::new()
    } else {
        original_id.unwrap_or_else(PackId::new)
    };
    if inspection.read_only_reason.is_some() {
        registry.insert(PackRecord {
            id,
            name: inspection.name.clone(),
            path: path.to_path_buf(),
            last_test: None,
            extra: BTreeMap::new(),
        })?;
        return Ok(ImportedPack {
            id,
            path: path.to_path_buf(),
            copied_id_replaced: copied,
            required_ignore_added: Vec::new(),
            read_only_reason: inspection.read_only_reason,
        });
    }
    cleanup_temp_files(path)?;
    let mut tx = PackTransaction::new(path.to_path_buf());
    if old_project.is_none() {
        tx.write(".warden/project.toml", project_toml(id).into_bytes())?;
    } else if copied {
        let mut doc = old_project
            .unwrap_or_default()
            .parse::<DocumentMut>()
            .map_err(|e| Error::new(Code::InvalidPack, format!(".warden/project.toml: {e}")))?;
        doc["id"] = value(id.to_string());
        tx.write(".warden/project.toml", doc.to_string().into_bytes())?;
    }
    if add_controls {
        for diff in &inspection.control_diffs {
            tx.write(&diff.path, diff.suggested.as_bytes().to_vec())?;
        }
    }
    let before_ignore = fs::read_to_string(path.join(".packwizignore")).ok();
    let required_ignore_added =
        warden_packwiz::hygiene::ensure_required_block(before_ignore.as_deref())
            .added
            .into_iter()
            .map(str::to_owned)
            .collect();
    tx.commit_without_refresh().await?;
    if !path.join(".git").exists() {
        PackRepo::init(
            path,
            InitialPoint::Imported,
            &Identity::for_pack(&inspection.name, None),
            Moment::now_utc(),
        )
        .map_err(|e| Error::new(Code::Internal, e.to_string()))?;
    }
    let record = PackRecord {
        id,
        name: inspection.name.clone(),
        path: path.to_path_buf(),
        last_test: None,
        extra: BTreeMap::new(),
    };
    registry.insert(record)?;
    Ok(ImportedPack {
        id,
        path: path.to_path_buf(),
        copied_id_replaced: copied,
        required_ignore_added,
        read_only_reason: inspection.read_only_reason,
    })
}

fn cleanup_temp_files(dir: &Path) -> Result<usize> {
    let mut removed =
        remove_temp_files(dir).map_err(|e| Error::new(Code::Internal, e.to_string()))?;
    for entry in fs::read_dir(dir).map_err(|e| Error::new(Code::Internal, e.to_string()))? {
        let entry = entry.map_err(|e| Error::new(Code::Internal, e.to_string()))?;
        let kind = entry
            .file_type()
            .map_err(|e| Error::new(Code::Internal, e.to_string()))?;
        if kind.is_dir() && entry.file_name() != ".git" {
            removed += cleanup_temp_files(&entry.path())?;
        }
    }
    Ok(removed)
}
