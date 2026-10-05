//! Inspeção e limpeza dos arquivos que não devem chegar aos jogadores.

use std::fs;
use std::path::Path;

use serde::Serialize;
use warden_core::{CancellationToken, resolve_inside};
use warden_packwiz::hygiene::{
    HygieneReason, PatternCategory, TEMPLATE_SECTIONS, missing_patterns, scan_dir,
};
use warden_packwiz::{PackIndex, read_pack};
use warden_packwiz_cli::Packwiz;
use warden_versioning::{Identity, Moment, PackRepo, SafetyReason};

use crate::transaction::PackTransaction;
use crate::{Error, ProjectErrorCode as Code, Result};

/// Item da higiene, pronto para o IPC.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct HygieneFinding {
    /// Caminho relativo.
    pub path: String,
    /// Pasta inteira.
    pub is_dir: bool,
    /// Existe no disco.
    pub on_disk: bool,
    /// Está no índice distribuído.
    pub in_index: bool,
    /// Tamanho no disco em bytes (a soma dos arquivos, numa pasta); `0` fora do disco.
    pub bytes: f64,
    /// Por que foi apontado; a interface escreve a frase.
    pub reasons: Vec<HygieneCause>,
}

/// Grupo do padrão do modelo que apontou o item (ARCHITECTURE §6.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum HygieneGroup {
    /// Bloco obrigatório (arquivos do próprio Warden).
    Required,
    /// Dados de execução do jogo e do launcher.
    Runtime,
    /// Segredos e arquivos de ferramentas.
    SecretsAndTools,
    /// Dados de servidor e do instalador.
    ServerOnly,
    /// Lixo em qualquer profundidade.
    AnyDepth,
}

impl From<PatternCategory> for HygieneGroup {
    fn from(category: PatternCategory) -> Self {
        match category {
            PatternCategory::Required => Self::Required,
            PatternCategory::Runtime => Self::Runtime,
            PatternCategory::SecretsAndTools => Self::SecretsAndTools,
            PatternCategory::ServerOnly => Self::ServerOnly,
            PatternCategory::AnyDepth => Self::AnyDepth,
        }
    }
}

/// Motivo de um item da higiene, sem texto: a frase fica no catálogo da interface.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum HygieneCause {
    /// Casa com uma linha do modelo do Warden.
    Pattern {
        /// A linha do modelo.
        pattern: String,
        /// O grupo da linha.
        group: HygieneGroup,
    },
    /// Item na raiz que não é arquivo de controle nem pasta de conteúdo conhecida.
    UnknownRootItem,
    /// Arquivo grande que não é jar.
    LargeFile,
    /// Pasta desconhecida com muitos arquivos.
    CacheLikeFolder {
        /// Quantos arquivos ela tem.
        files: u32,
    },
    /// Link simbólico ou junção.
    Symlink,
}

/// Varre pasta e índice sem alterar o pack.
pub fn scan(root: &Path) -> Result<Vec<HygieneFinding>> {
    let pack = read_pack(root).map_err(|e| Error::new(Code::InvalidPack, e.to_string()))?;
    let index: Option<&PackIndex> = pack.index.as_ref().ok().map(|i| &i.value);
    findings(root, index)
}

/// Converte a varredura do `warden-packwiz` nos itens do IPC, com o tamanho de cada um.
pub(crate) fn findings(root: &Path, index: Option<&PackIndex>) -> Result<Vec<HygieneFinding>> {
    scan_dir(root, index)
        .map_err(|e| Error::new(Code::InvalidPack, e.to_string()))
        .map(|items| {
            items
                .into_iter()
                .map(|item| HygieneFinding {
                    bytes: if item.on_disk {
                        size_on_disk(&root.join(&item.path))
                    } else {
                        0.0
                    },
                    path: item.path,
                    is_dir: item.is_dir,
                    on_disk: item.on_disk,
                    in_index: item.in_index,
                    reasons: item.reasons.iter().map(cause).collect(),
                })
                .collect()
        })
}

fn cause(reason: &HygieneReason) -> HygieneCause {
    match reason {
        HygieneReason::MatchesPattern { pattern, category } => HygieneCause::Pattern {
            pattern: (*pattern).to_owned(),
            group: (*category).into(),
        },
        HygieneReason::UnknownRootItem => HygieneCause::UnknownRootItem,
        HygieneReason::LargeFile { .. } => HygieneCause::LargeFile,
        HygieneReason::CacheLikeFolder { files } => HygieneCause::CacheLikeFolder {
            files: u32::try_from(*files).unwrap_or(u32::MAX),
        },
        HygieneReason::Symlink => HygieneCause::Symlink,
    }
}

/// Tamanho de um arquivo, ou a soma dos arquivos de uma pasta (sem seguir links).
#[allow(clippy::cast_precision_loss)] // tamanhos de arquivo cabem com folga em f64
fn size_on_disk(path: &Path) -> f64 {
    fn walk(path: &Path) -> u64 {
        let Ok(meta) = fs::symlink_metadata(path) else {
            return 0;
        };
        if meta.is_file() {
            return meta.len();
        }
        if !meta.is_dir() {
            return 0;
        }
        fs::read_dir(path).map_or(0, |entries| {
            entries
                .filter_map(std::result::Result::ok)
                .map(|entry| walk(&entry.path()))
                .sum()
        })
    }
    walk(path) as f64
}

/// Apaga apenas os achados escolhidos, com ponto de segurança e `refresh` reversível.
pub async fn fix(
    root: &Path,
    chosen: &[String],
    author: &str,
    packwiz: &Packwiz,
    cancel: &CancellationToken,
) -> Result<Vec<String>> {
    let findings = scan(root)?;
    let mut tx = PackTransaction::new(root.to_path_buf());
    let mut deleted = Vec::new();
    for relative in chosen {
        let finding = findings
            .iter()
            .find(|item| &item.path == relative)
            .ok_or_else(|| {
                Error::new(
                    Code::InvalidInput,
                    format!("item não encontrado na higiene: {relative}"),
                )
            })?;
        let path = resolve_inside(root, relative)
            .map_err(|e| Error::new(Code::InvalidInput, e.to_string()))?;
        if finding.is_dir {
            collect_files(root, &path, &mut deleted)?;
        } else if path.is_file() {
            deleted.push(relative.clone());
        }
    }
    deleted.sort();
    deleted.dedup();
    for path in &deleted {
        tx.delete(path)?;
    }
    for name in [".packwizignore", ".gitignore"] {
        let path = root.join(name);
        let original = fs::read_to_string(&path).unwrap_or_default();
        let patterns: Vec<&str> = TEMPLATE_SECTIONS
            .iter()
            .flat_map(|section| section.patterns.iter().copied())
            .filter(|pattern| {
                name != ".gitignore"
                    || !warden_packwiz::hygiene::GIT_TRACKED_PATTERNS.contains(pattern)
            })
            .collect();
        let missing = missing_patterns(&original, &patterns);
        if !missing.is_empty() {
            let mut text = original;
            if !text.is_empty() && !text.ends_with('\n') {
                text.push('\n');
            }
            for line in missing {
                text.push_str(line);
                text.push('\n');
            }
            tx.write(name, text.into_bytes())?;
        }
    }
    let repo = PackRepo::open(root).map_err(|e| Error::new(Code::Internal, e.to_string()))?;
    repo.check_writable()
        .map_err(|e| Error::new(Code::ReadOnly, e.to_string()))?;
    repo.create_safety_point(
        &SafetyReason::HygieneCleanup,
        &Identity::for_pack(author, None),
        Moment::now_utc(),
    )
    .map_err(|e| Error::new(Code::Internal, e.to_string()))?;
    tx.commit(packwiz, cancel).await?;
    Ok(deleted)
}

fn collect_files(root: &Path, path: &Path, out: &mut Vec<String>) -> Result<()> {
    for entry in fs::read_dir(path).map_err(|e| Error::new(Code::Internal, e.to_string()))? {
        let entry = entry.map_err(|e| Error::new(Code::Internal, e.to_string()))?;
        let kind = entry
            .file_type()
            .map_err(|e| Error::new(Code::Internal, e.to_string()))?;
        if kind.is_symlink() {
            return Err(Error::new(
                Code::InvalidInput,
                "limpeza de link simbólico exige remoção manual",
            ));
        }
        if kind.is_dir() {
            collect_files(root, &entry.path(), out)?;
        } else if kind.is_file() {
            let entry_path = entry.path();
            let relative = entry_path
                .strip_prefix(root)
                .map_err(|e| Error::new(Code::Internal, e.to_string()))?;
            out.push(relative.to_string_lossy().replace('\\', "/"));
        }
    }
    Ok(())
}
