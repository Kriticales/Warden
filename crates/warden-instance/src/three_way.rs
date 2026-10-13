//! Conflito de três vias da captura (ARCHITECTURE §8.3 item 7; SPEC T15; CA-T15-05).
//!
//! Base = a linha de base (o arquivo no início do teste), "pack" = o arquivo do pack agora,
//! "instância" = o arquivo do jogo agora. Há conflito quando a config mudou no jogo **e** o
//! arquivo do pack mudou desde o início do teste (editado no Warden com o jogo aberto, por
//! exemplo). A tela mostra as três versões com "Manter a do pack", "Usar a do jogo", "Juntar as
//! duas" (quando as chaves mudadas são diferentes) e "Editar e combinar".
//!
//! O lado do pack é lido na hora ([`capture_view`]): a captura gravada só tem o lado do jogo, e
//! o pack pode mudar depois que o jogo fechou. "Trazer" confere o SHA-256 do pack mostrado
//! ([`crate::FileDecision::expected_pack_sha256`]).

use std::fs;
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};
use warden_configs::{ConfigDocument, ConfigEdit, ConfigFormat, SemanticChange, compare};
use warden_core::resolve_inside;

use crate::baseline::{Baseline, BlobStore, instance_path, read_pack_file, sha256_hex};
use crate::capture::{
    CaptureError, CaptureGroup, CaptureStatus, CapturedFile, ChangeKind, ValueChange, load_capture,
    option_values, text_of, value_change,
};
use crate::error::Error;
use crate::materialize::InstanceDirs;

/// O arquivo do pack mudou durante o teste (CA-T15-05).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PackConflict {
    /// O que mudou no pack desde o início do teste, por valor (formatos conhecidos; no
    /// `options.txt`, as chaves).
    pub pack_values: Vec<ValueChange>,
    /// Se "Juntar as duas" é possível: formato conhecido e as chaves mudadas no jogo não foram
    /// mexidas no pack.
    pub mergeable: bool,
}

/// Um arquivo da captura com o lado do pack lido agora.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FileView {
    /// A mudança no jogo.
    pub file: CapturedFile,
    /// SHA-256 do arquivo de destino no pack agora (`None`: não existe). Volta no pedido de
    /// "Trazer" para conferir que o pack não mudou depois.
    pub pack_sha256: Option<String>,
    /// O conflito, se o arquivo também mudou no pack durante o teste.
    pub conflict: Option<PackConflict>,
}

/// A captura como a tela a mostra.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CaptureView {
    /// A sessão do teste.
    pub session_id: String,
    /// Situação.
    pub status: CaptureStatus,
    /// Se o Testar pergunta antes e o cabeçalho avisa.
    pub needs_review: bool,
    /// Quantos arquivos o jogo só reformatou.
    pub reformatted: u32,
    /// As mudanças.
    pub files: Vec<FileView>,
}

/// As três versões de um arquivo, para "Ver diferença de texto" e "Editar e combinar".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CaptureTexts {
    /// No início do teste (`None`: não existia ou não é texto).
    pub base: Option<String>,
    /// No jogo agora.
    pub instance: Option<String>,
    /// No pack agora.
    pub pack: Option<String>,
    /// SHA-256 do pack lido (para gravar o texto combinado).
    pub pack_sha256: Option<String>,
    /// Fim de linha do arquivo (`\r\n` ou `\n`).
    pub line_ending: String,
}

/// A captura com o lado do pack de agora. `None` sem captura.
///
/// Erros: disco.
pub fn capture_view(dirs: &InstanceDirs, pack_root: &Path) -> Result<Option<CaptureView>, Error> {
    let Some(capture) = load_capture(&dirs.state_dir)? else {
        return Ok(None);
    };
    let baseline = Baseline::load(&dirs.state_dir)?;
    let blobs = BlobStore::of(&dirs.state_dir);
    let pending = capture.status == CaptureStatus::Pending;
    let mut files = Vec::with_capacity(capture.files.len());
    for file in &capture.files {
        let mut view = FileView {
            file: file.clone(),
            pack_sha256: None,
            conflict: None,
        };
        if pending && let Some(target) = &file.pack_path {
            let pack_now = read_pack_file(pack_root, target)?;
            view.pack_sha256 = pack_now.as_deref().map(sha256_hex);
            view.conflict = conflict(&blobs, baseline.as_ref(), file, pack_now.as_deref())?;
        }
        files.push(view);
    }
    Ok(Some(CaptureView {
        session_id: capture.session_id.clone(),
        status: capture.status,
        needs_review: capture.needs_review(),
        reformatted: capture.reformatted,
        files,
    }))
}

/// O conflito de uma config do pack (ou do `options.txt` do pack).
fn conflict(
    blobs: &BlobStore,
    baseline: Option<&Baseline>,
    file: &CapturedFile,
    pack_now: Option<&[u8]>,
) -> Result<Option<PackConflict>, Error> {
    if !matches!(file.group, CaptureGroup::PackConfig | CaptureGroup::Options) {
        return Ok(None);
    }
    let Some(start) = baseline.and_then(|base| base.pack.get(&file.path)) else {
        return Ok(None);
    };
    let now_sha = pack_now.map(sha256_hex);
    if now_sha == start.sha256 {
        return Ok(None);
    }
    let pack_start = match (&start.sha256, start.stored) {
        (Some(sha), true) => blobs.get(sha)?,
        _ => None,
    };
    if file.group == CaptureGroup::Options {
        let old = option_values(pack_start.as_deref().unwrap_or_default());
        let new = option_values(pack_now.unwrap_or_default());
        let pack_values: Vec<ValueChange> = new
            .iter()
            .filter(|(key, value)| old.get(*key) != Some(*value))
            .map(|(key, value)| ValueChange {
                key: key.clone(),
                before: old.get(key).cloned(),
                after: Some(value.clone()),
            })
            .collect();
        // As chaves do jogo vão por cima, uma a uma: sempre dá para juntar.
        return Ok(Some(PackConflict {
            pack_values,
            mergeable: true,
        }));
    }
    let format = ConfigFormat::detect(Path::new(&file.path), pack_now.unwrap_or_default());
    let pack_values = match (format, pack_start.as_deref(), pack_now) {
        (Some(format), Some(old), Some(new)) => compare(format, old, new)
            .map(|diff| diff.changes.iter().map(value_change).collect())
            .unwrap_or_default(),
        _ => Vec::new(),
    };
    let mergeable =
        file.change == ChangeKind::Modified && merge_bytes(blobs, file, pack_now)?.is_some();
    Ok(Some(PackConflict {
        pack_values,
        mergeable,
    }))
}

/// "Juntar as duas" de um arquivo da captura.
pub(crate) fn merge_file(
    blobs: &BlobStore,
    file: &CapturedFile,
    pack_now: Option<&[u8]>,
) -> Result<Option<Vec<u8>>, CaptureError> {
    Ok(merge_bytes(blobs, file, pack_now)?)
}

fn merge_bytes(
    blobs: &BlobStore,
    file: &CapturedFile,
    pack_now: Option<&[u8]>,
) -> Result<Option<Vec<u8>>, Error> {
    let (Some(before), Some(after), Some(pack)) = (
        file.before_sha256.as_deref(),
        file.after_sha256.as_deref(),
        pack_now,
    ) else {
        return Ok(None);
    };
    let (Some(base), Some(instance)) = (blobs.get(before)?, blobs.get(after)?) else {
        return Ok(None);
    };
    let Some(format) = ConfigFormat::detect(Path::new(&file.path), pack) else {
        return Ok(None);
    };
    Ok(merge(format, &base, &instance, pack))
}

/// Junta as mudanças do jogo (base → instância) com o pack de agora, quando dá: só valores
/// alterados (nenhuma chave nova ou apagada no jogo) e nenhuma dessas chaves mexida no pack.
/// O resultado é o arquivo do pack com só os valores dessas chaves trocados.
#[must_use]
pub fn merge(format: ConfigFormat, base: &[u8], instance: &[u8], pack: &[u8]) -> Option<Vec<u8>> {
    let changes = compare(format, base, instance).ok()?.changes;
    if changes.is_empty() {
        return None;
    }
    let document = ConfigDocument::parse(format, pack).ok()?;
    let mut edits = Vec::with_capacity(changes.len());
    for change in changes {
        let SemanticChange::Changed {
            path,
            before,
            after,
        } = change
        else {
            return None;
        };
        let current = document.get(&path)?.value.as_ref()?;
        if !current.same_as(&before) {
            return None;
        }
        edits.push(ConfigEdit::new(path, after));
    }
    document.apply(&edits).ok().map(ConfigDocument::into_bytes)
}

/// As três versões de um arquivo da captura (texto UTF-8; binário vira `None`).
///
/// Erros: [`CaptureError::NotFound`] (sem captura desse teste),
/// [`CaptureError::InvalidItem`] (o arquivo não está na captura), disco.
pub fn capture_texts(
    dirs: &InstanceDirs,
    pack_root: &Path,
    session_id: &str,
    path: &str,
) -> Result<CaptureTexts, CaptureError> {
    let capture = load_capture(&dirs.state_dir)?
        .filter(|capture| capture.session_id == session_id)
        .ok_or(CaptureError::NotFound)?;
    let file = capture
        .file(path)
        .ok_or_else(|| CaptureError::InvalidItem {
            path: path.to_owned(),
        })?;
    let blobs = BlobStore::of(&dirs.state_dir);
    let base = match file.before_sha256.as_deref() {
        Some(sha) => blobs.get(sha)?,
        None => None,
    };
    let instance = match file.after_sha256.as_deref() {
        Some(sha) => match blobs.get(sha)? {
            Some(bytes) => Some(bytes),
            None => read_instance(dirs, path)?,
        },
        None => None,
    };
    let pack = match &file.pack_path {
        Some(target) => read_pack_file(pack_root, target)?,
        None => None,
    };
    let any_crlf = [&base, &instance, &pack]
        .iter()
        .filter_map(|bytes| bytes.as_deref())
        .any(|bytes| bytes.windows(2).any(|pair| pair == b"\r\n"));
    let as_text = |bytes: Option<&[u8]>| text_of(bytes).map(ToOwned::to_owned);
    Ok(CaptureTexts {
        base: as_text(base.as_deref()),
        instance: as_text(instance.as_deref()),
        pack_sha256: pack.as_deref().map(sha256_hex),
        pack: as_text(pack.as_deref()),
        line_ending: if any_crlf { "\r\n" } else { "\n" }.to_owned(),
    })
}

fn read_instance(dirs: &InstanceDirs, path: &str) -> Result<Option<Vec<u8>>, Error> {
    let full = resolve_inside(&dirs.game_dir, path)?;
    match fs::read(&full) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(Error::io("ler", instance_path(path), error)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn junta_chaves_diferentes() {
        let base = b"[a]\nx = 1\ny = 1\n";
        let instance = b"[a]\nx = 2\ny = 1\n";
        let pack = b"# editado no pack\n[a]\nx = 1\ny = 5\n";
        let merged = merge(ConfigFormat::Toml, base, instance, pack).unwrap();
        assert_eq!(merged, b"# editado no pack\n[a]\nx = 2\ny = 5\n");
    }

    #[test]
    fn nao_junta_a_mesma_chave_nem_chave_nova() {
        let base = b"x = 1\n";
        assert!(merge(ConfigFormat::Toml, base, b"x = 2\n", b"x = 3\n").is_none());
        assert!(merge(ConfigFormat::Toml, base, b"x = 1\ny = 2\n", b"x = 1\n").is_none());
        assert!(merge(ConfigFormat::Toml, base, base, b"x = 1\n").is_none());
        assert!(merge(ConfigFormat::Toml, base, b"x = 2\n", b"x = [quebrado").is_none());
    }

    #[test]
    fn junta_mantendo_crlf() {
        let base = b"{\r\n  \"a\": 1,\r\n  \"b\": 1\r\n}\r\n";
        let instance = b"{\"a\": 2, \"b\": 1}";
        let pack = b"{\r\n  \"a\": 1,\r\n  \"b\": 3\r\n}\r\n";
        let merged = merge(ConfigFormat::Json, base, instance, pack).unwrap();
        assert_eq!(merged, b"{\r\n  \"a\": 2,\r\n  \"b\": 3\r\n}\r\n");
    }
}
