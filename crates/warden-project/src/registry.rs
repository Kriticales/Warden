//! Registro local dos packs, sem guardar estado de máquina dentro deles.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use warden_core::{PackId, atomic_write, resolve_inside};
use warden_packwiz::read_pack;
use warden_versioning::{FileNames, PackRepo};

use crate::{Error, ProjectErrorCode as Code, Result};

/// Dados locais de um pack. Campos futuros são preservados ao gravar.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackRecord {
    /// Identificador estável.
    pub id: PackId,
    /// Último nome legível, inclusive se a pasta sumir.
    #[serde(default)]
    pub name: String,
    /// Pasta atual.
    pub path: PathBuf,
    /// Estado do último teste, registrado por L-04.
    #[serde(default)]
    pub last_test: Option<String>,
    /// Preferências e campos de versões futuras.
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Situação de uma linha de Meus packs.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum PackStatus {
    /// Pack legível e pasta presente.
    Ready,
    /// Pasta registrada não existe mais.
    FolderMissing,
    /// Manifesto não pôde ser lido.
    InvalidPack,
}

/// Linha pronta para a lista, sem executar diagnóstico.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PackRow {
    /// Identificador.
    pub id: PackId,
    /// Pasta.
    pub path: PathBuf,
    /// Nome legível, quando o manifesto puder ser lido.
    pub name: String,
    /// Versão do Minecraft.
    pub minecraft: Option<String>,
    /// Loader.
    pub loader: Option<String>,
    /// Versão do loader.
    pub loader_version: Option<String>,
    /// Versão do pack.
    pub version: Option<String>,
    /// Última alteração conhecida dos arquivos do pack, em milissegundos Unix.
    pub modified_at_ms: Option<f64>,
    /// Estado da pasta.
    pub status: PackStatus,
    /// Detalhe técnico se o manifesto for inválido.
    pub detail: Option<String>,
    /// Motivo pelo qual o repositório só pode ser aberto para leitura.
    pub read_only_reason: Option<String>,
    /// Quantidade de arquivos alterados desde a última versão salva.
    pub unsaved_files: u32,
    /// Último teste.
    pub last_test: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RegistryData {
    #[serde(default = "schema_version")]
    schema_version: u32,
    #[serde(default)]
    packs: Vec<PackRecord>,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

fn schema_version() -> u32 {
    1
}

/// Registro protegido para comandos concorrentes.
#[derive(Debug)]
pub struct Registry {
    path: PathBuf,
    data: Mutex<RegistryData>,
}

impl Registry {
    /// Abre `packs.json`; arquivo ausente significa lista vazia.
    pub fn open(path: PathBuf) -> Result<Self> {
        let data = match fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map_err(|e| Error::new(Code::InvalidInput, format!("packs.json: {e}")))?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => RegistryData {
                schema_version: 1,
                ..RegistryData::default()
            },
            Err(e) => return Err(Error::new(Code::Internal, format!("ler packs.json: {e}"))),
        };
        Ok(Self {
            path,
            data: Mutex::new(data),
        })
    }

    /// Cópia dos registros, na ordem persistida.
    pub fn records(&self) -> Vec<PackRecord> {
        self.data
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .packs
            .clone()
    }

    /// Busca pelo identificador.
    pub fn get(&self, id: PackId) -> Result<PackRecord> {
        self.records()
            .into_iter()
            .find(|p| p.id == id)
            .ok_or_else(|| Error::new(Code::PackNotFound, id.to_string()))
    }

    /// Registra um pack sem substituir outro ID ou caminho.
    pub fn insert(&self, record: PackRecord) -> Result<()> {
        let mut data = self.data.lock().unwrap_or_else(PoisonError::into_inner);
        if data
            .packs
            .iter()
            .any(|p| p.id == record.id || same_path(&p.path, &record.path))
        {
            return Err(Error::new(
                Code::AlreadyRegistered,
                record.path.display().to_string(),
            ));
        }
        let mut updated = data.clone();
        updated.packs.push(record);
        self.save(&updated)?;
        *data = updated;
        Ok(())
    }

    /// Retira só da lista; não escreve na pasta do pack.
    pub fn forget(&self, id: PackId) -> Result<()> {
        let mut data = self.data.lock().unwrap_or_else(PoisonError::into_inner);
        let before = data.packs.len();
        let mut updated = data.clone();
        updated.packs.retain(|p| p.id != id);
        if updated.packs.len() == before {
            return Err(Error::new(Code::PackNotFound, id.to_string()));
        }
        self.save(&updated)?;
        *data = updated;
        Ok(())
    }

    /// Aponta um pack perdido para a sua pasta nova, preservando os dados locais.
    pub fn relocate(&self, id: PackId, path: PathBuf) -> Result<()> {
        let mut data = self.data.lock().unwrap_or_else(PoisonError::into_inner);
        if data
            .packs
            .iter()
            .any(|p| p.id != id && same_path(&p.path, &path))
        {
            return Err(Error::new(
                Code::AlreadyRegistered,
                path.display().to_string(),
            ));
        }
        let mut updated = data.clone();
        let item = updated
            .packs
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or_else(|| Error::new(Code::PackNotFound, id.to_string()))?;
        item.path = path;
        self.save(&updated)?;
        *data = updated;
        Ok(())
    }

    /// Monta a lista tolerando pasta ausente e manifesto inválido.
    pub fn list(&self) -> Vec<PackRow> {
        self.records().iter().map(row).collect()
    }

    fn save(&self, data: &RegistryData) -> Result<()> {
        let bytes = serde_json::to_vec_pretty(data)
            .map_err(|e| Error::new(Code::Internal, e.to_string()))?;
        atomic_write(&self.path, &bytes).map_err(|e| Error::new(Code::Internal, e.to_string()))
    }
}

fn same_path(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

fn row(record: &PackRecord) -> PackRow {
    let mut result = PackRow {
        id: record.id,
        path: record.path.clone(),
        name: record.name.clone(),
        minecraft: None,
        loader: None,
        loader_version: None,
        version: None,
        modified_at_ms: None,
        status: PackStatus::Ready,
        detail: None,
        read_only_reason: None,
        unsaved_files: 0,
        last_test: record.last_test.clone(),
    };
    if !record.path.is_dir() {
        result.status = PackStatus::FolderMissing;
        return result;
    }
    match read_pack(&record.path) {
        Ok(pack) => {
            let manifest = pack.pack.value;
            result.name.clone_from(&manifest.name);
            result.minecraft = manifest.minecraft_version().map(str::to_owned);
            if let Some((loader, version)) = manifest.loaders().first() {
                result.loader = Some(loader.key().to_owned());
                result.loader_version = Some((*version).to_owned());
            }
            result.modified_at_ms = ["pack.toml", manifest.index.file.as_str()]
                .iter()
                .filter_map(|name| resolve_inside(&record.path, name).ok())
                .filter_map(|path| modified_at_ms(&path))
                .reduce(f64::max);
            result.version = Some(manifest.version);
            if let Ok(repo) = PackRepo::open(&record.path) {
                result.read_only_reason =
                    repo.check_writable().err().map(|error| error.to_string());
                if let Ok(changes) = repo.changes_since_last_version(&FileNames) {
                    result.unsaved_files = u32::try_from(changes.files.len()).unwrap_or(u32::MAX);
                    for changed in &changes.files {
                        if let Ok(path) = resolve_inside(&record.path, &changed.path)
                            && let Some(when) = modified_at_ms(&path)
                        {
                            result.modified_at_ms =
                                Some(result.modified_at_ms.map_or(when, |old| old.max(when)));
                        }
                    }
                }
            }
        }
        Err(e) => {
            result.status = PackStatus::InvalidPack;
            result.detail = Some(e.to_string());
        }
    }
    result
}

fn modified_at_ms(path: &Path) -> Option<f64> {
    let time = fs::metadata(path).ok()?.modified().ok()?;
    let elapsed = time.duration_since(std::time::UNIX_EPOCH).ok()?;
    Some(elapsed.as_secs_f64() * 1000.0)
}
