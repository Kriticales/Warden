//! Inspeção e limpeza dos arquivos que não devem chegar aos jogadores.

use std::fs;
use std::path::Path;

use serde::Serialize;
use warden_core::{CancellationToken, resolve_inside};
use warden_packwiz::hygiene::{HygieneReason, TEMPLATE_SECTIONS, missing_patterns, scan_dir};
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
    /// Motivos legíveis.
    pub reasons: Vec<String>,
}

/// Varre pasta e índice sem alterar o pack.
pub fn scan(root: &Path) -> Result<Vec<HygieneFinding>> {
    let pack = read_pack(root).map_err(|e| Error::new(Code::InvalidPack, e.to_string()))?;
    let index: Option<&PackIndex> = pack.index.as_ref().ok().map(|i| &i.value);
    scan_dir(root, index)
        .map_err(|e| Error::new(Code::InvalidPack, e.to_string()))
        .map(|items| {
            items
                .into_iter()
                .map(|item| HygieneFinding {
                    path: item.path,
                    is_dir: item.is_dir,
                    on_disk: item.on_disk,
                    in_index: item.in_index,
                    reasons: item.reasons.iter().map(reason).collect(),
                })
                .collect()
        })
}

fn reason(reason: &HygieneReason) -> String {
    match reason {
        HygieneReason::MatchesPattern { pattern, .. } => format!("corresponde a {pattern}"),
        HygieneReason::UnknownRootItem => "item desconhecido na raiz".to_owned(),
        HygieneReason::LargeFile { bytes } => format!("arquivo grande ({bytes} bytes)"),
        HygieneReason::CacheLikeFolder { files } => format!("pasta com {files} arquivos"),
        HygieneReason::Symlink => "link simbólico".to_owned(),
    }
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
