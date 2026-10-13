//! Validação do `.mrpack` gerado: o esquema do formato e as regras do Prism Launcher 11.x lidas
//! do código dele (ADR-0050): caminhos seguros, endereços https, hashes e `overrides/`.
//!
//! Esta validação é própria da E-04. Quando o validador de formatos da E-02 estiver integrado,
//! a chamada dele pode ser somada a esta (registrado no handoff).

use std::collections::BTreeSet;
use std::io::Read;

use serde_json::Value;
use url::Url;

use super::plan::MODRINTH_CDN_HOST;
use crate::{Error, ExportErrorCode, Result};

/// O que o arquivo gerado deve conter.
pub(super) struct Expect<'a> {
    pub(super) minecraft: &'a str,
    /// Chave em `dependencies` e versão exata do loader.
    pub(super) loader: Option<(&'a str, &'a str)>,
    /// Caminhos que podem aparecer em `overrides/` (arquivos do índice que não são referência).
    pub(super) override_paths: &'a BTreeSet<String>,
    /// Caminhos que o Prism baixa (`files[].path`): nunca podem repetir em `overrides/`.
    pub(super) file_paths: &'a BTreeSet<String>,
}

/// Resumo do que foi conferido.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Checked {
    pub(super) files: usize,
    pub(super) overrides: usize,
    /// Itens que o Prism lista como "não confiáveis".
    pub(super) untrusted: usize,
}

fn invalid(reason: impl Into<String>) -> Error {
    let reason = reason.into();
    Error::new(
        ExportErrorCode::PrismInvalidOutput,
        format!("instância inválida: {reason}"),
    )
    .with_detail(reason)
}

fn safe_path(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !path.contains('\\')
        && !path.contains(':')
        && path
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
}

fn is_hex(text: &str, len: usize) -> bool {
    text.len() == len
        && text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Confere `files[]`: devolve quantos são e quantos o Prism lista como não confiáveis.
fn check_files(json: &Value, expect: &Expect<'_>) -> Result<(usize, usize)> {
    let files = json["files"]
        .as_array()
        .ok_or_else(|| invalid("faltam os files"))?;
    let mut seen = BTreeSet::new();
    let mut untrusted = 0;
    for file in files {
        let path = file["path"].as_str().unwrap_or_default();
        if !safe_path(path) || !seen.insert(path.to_owned()) {
            return Err(invalid(format!("caminho inválido ou repetido: {path}")));
        }
        if !expect.file_paths.contains(path) {
            return Err(invalid(format!("arquivo fora do pack: {path}")));
        }
        let hashes = &file["hashes"];
        if !is_hex(hashes["sha1"].as_str().unwrap_or_default(), 40)
            || !is_hex(hashes["sha512"].as_str().unwrap_or_default(), 128)
        {
            return Err(invalid(format!("{path}: sha1/sha512 inválidos")));
        }
        if file["fileSize"].as_u64().unwrap_or(0) == 0 {
            return Err(invalid(format!("{path}: fileSize ausente")));
        }
        for side in ["client", "server"] {
            let value = file["env"][side].as_str().unwrap_or("required");
            if !matches!(value, "required" | "optional" | "unsupported") {
                return Err(invalid(format!("{path}: env.{side} = {value}")));
            }
        }
        let downloads = file["downloads"].as_array().filter(|d| !d.is_empty());
        let downloads = downloads.ok_or_else(|| invalid(format!("{path}: sem downloads")))?;
        let mut trusted = true;
        for download in downloads {
            let url = download.as_str().and_then(|u| Url::parse(u).ok());
            let url = url.ok_or_else(|| invalid(format!("{path}: endereço inválido")))?;
            if url.scheme() != "https" || url.host_str().is_none() {
                return Err(invalid(format!("{path}: o Prism só baixa por https")));
            }
            trusted &= url.host_str() == Some(MODRINTH_CDN_HOST);
        }
        untrusted += usize::from(!trusted);
    }
    Ok((files.len(), untrusted))
}

/// Confere o conteúdo do zip.
pub(super) fn validate(bytes: &[u8], expect: &Expect<'_>) -> Result<Checked> {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes))
        .map_err(|e| invalid(format!("o zip não abre: {e}")))?;
    let mut index_text = String::new();
    let mut overrides = BTreeSet::new();
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|e| invalid(e.to_string()))?;
        let name = entry.name().to_owned();
        if entry.is_dir() {
            continue;
        }
        if !safe_path(&name) {
            return Err(invalid(format!("caminho inseguro no zip: {name}")));
        }
        if name == "modrinth.index.json" {
            entry
                .read_to_string(&mut index_text)
                .map_err(|e| invalid(format!("modrinth.index.json: {e}")))?;
        } else if let Some(rest) = name.strip_prefix("overrides/") {
            if !overrides.insert(rest.to_owned()) {
                return Err(invalid(format!("arquivo repetido no zip: {name}")));
            }
        } else {
            return Err(invalid(format!("arquivo inesperado no zip: {name}")));
        }
    }
    if index_text.is_empty() {
        return Err(invalid("falta o modrinth.index.json"));
    }
    let json: Value =
        serde_json::from_str(&index_text).map_err(|e| invalid(format!("JSON inválido: {e}")))?;
    if json["formatVersion"] != 1 || json["game"] != "minecraft" {
        return Err(invalid("formatVersion/game inesperados"));
    }
    for key in ["versionId", "name"] {
        if json[key].as_str().is_none_or(|v| v.trim().is_empty()) {
            return Err(invalid(format!("{key} vazio")));
        }
    }
    let deps = json["dependencies"]
        .as_object()
        .ok_or_else(|| invalid("faltam as dependencies"))?;
    if deps.get("minecraft").and_then(Value::as_str) != Some(expect.minecraft) {
        return Err(invalid("a versão do Minecraft não confere com o pack.toml"));
    }
    let expected_deps = 1 + usize::from(expect.loader.is_some());
    if deps.len() != expected_deps {
        return Err(invalid("dependencies com itens a mais"));
    }
    if let Some((key, version)) = expect.loader
        && deps.get(key).and_then(Value::as_str) != Some(version)
    {
        return Err(invalid("a versão do loader não confere com o pack.toml"));
    }
    let (files, mut untrusted) = check_files(&json, expect)?;
    for path in &overrides {
        if !expect.override_paths.contains(path) || expect.file_paths.contains(path) {
            return Err(invalid(format!(
                "overrides/{path} não vem do índice do pack"
            )));
        }
        // O Prism lista jars soltos em overrides/mods/ entre os não confiáveis.
        untrusted += usize::from(super::plan::is_loose_mod_jar(path));
    }
    if overrides.len() != expect.override_paths.len() {
        return Err(invalid("faltam arquivos em overrides/"));
    }
    Ok(Checked {
        files,
        overrides: overrides.len(),
        untrusted,
    })
}
