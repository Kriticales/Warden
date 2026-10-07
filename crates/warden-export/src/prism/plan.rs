//! Leitura do pack para a instância pronta: o que o Prism baixa, o que vai em `overrides/`.

use std::collections::BTreeMap;

use url::Url;
use warden_http::ExpectedHash;
use warden_instance::{GameRequirement, game_requirement};
use warden_packwiz::{Metafile, PACK_FILE, PackManifest, Side, check_relative_path};

use crate::{Error, ExportErrorCode, Result};

/// Servidor dos arquivos do Modrinth: o único que o Prism não trata como "não confiável".
pub(super) const MODRINTH_CDN_HOST: &str = "cdn.modrinth.com";

/// Pastas cujo conteúdo local é de terceiros (pede confirmação de licença).
const THIRD_PARTY_DIRS: [&str; 3] = ["mods/", "resourcepacks/", "shaderpacks/"];

/// De onde o Prism baixa o arquivo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum RefSource {
    /// Endereço do `cdn.modrinth.com` gravado no metafile.
    Modrinth { url: String },
    /// Arquivo da CurseForge: o endereço da CDN é montado do ID e do nome.
    Curseforge { project: u32, file: u32 },
    /// Qualquer outro endereço do metafile.
    Link { url: String },
}

/// Um mod, resource pack ou shader de referência (`.pw.toml`).
#[derive(Debug, Clone)]
pub(super) struct RefItem {
    pub(super) meta_path: String,
    pub(super) name: String,
    /// Caminho do arquivo no jogo (`mods/x.jar`).
    pub(super) dest: String,
    pub(super) filename: String,
    pub(super) side: Side,
    pub(super) optional: bool,
    pub(super) source: RefSource,
    pub(super) expected: ExpectedHash,
}

/// O pack lido.
#[derive(Debug)]
pub(super) struct PackPlan {
    pub(super) manifest: PackManifest,
    pub(super) game: GameRequirement,
    pub(super) refs: Vec<RefItem>,
    /// Arquivos do índice que vão em `overrides/`, com o conteúdo.
    pub(super) overrides: BTreeMap<String, Vec<u8>>,
}

impl PackPlan {
    /// Lê a cópia do pack (os arquivos do índice mais `pack.toml` e `index.toml`).
    pub(super) fn read(files: &BTreeMap<String, Vec<u8>>, require_version: bool) -> Result<Self> {
        let manifest = PackManifest::parse(utf8(files.get(PACK_FILE), PACK_FILE)?)
            .map_err(|e| Error::internal(e.to_string()))?
            .value;
        if require_version && manifest.version.trim().is_empty() {
            return Err(Error::new(
                ExportErrorCode::PrismVersionRequired,
                "o pack.toml não tem version",
            ));
        }
        if manifest
            .options
            .as_ref()
            .is_some_and(|o| o.contains_key("meta-folder") || o.contains_key("meta-folder-base"))
        {
            return Err(Error::new(
                ExportErrorCode::PrismUnsupportedPack,
                "o pack usa meta-folder, que a instância pronta não reproduz",
            ));
        }
        let game = game_requirement(&manifest)
            .map_err(|e| Error::new(ExportErrorCode::PrismUnsupportedPack, e.to_string()))?;
        if let Some(loader) = &game.loader
            && mrpack_loader_key(&loader.loader).is_none()
        {
            return Err(Error::new(
                ExportErrorCode::PrismUnsupportedPack,
                format!("o loader {} não existe no formato .mrpack", loader.loader),
            ));
        }
        let mut refs = Vec::new();
        let mut overrides = BTreeMap::new();
        for (path, bytes) in files {
            if matches!(path.as_str(), PACK_FILE | "index.toml") {
                continue;
            }
            if path.ends_with(".pw.toml") {
                refs.push(read_ref(path, bytes)?);
            } else if !path.starts_with("server-overrides/") {
                overrides.insert(path.clone(), bytes.clone());
            }
        }
        Ok(Self {
            manifest,
            game,
            refs,
            overrides,
        })
    }

    /// Arquivos de terceiros em `overrides/` que pedem confirmação de licença.
    pub(super) fn third_party_locals(&self) -> Vec<(&str, u64)> {
        self.overrides
            .iter()
            .filter(|(path, _)| THIRD_PARTY_DIRS.iter().any(|dir| path.starts_with(dir)))
            .map(|(path, bytes)| (path.as_str(), bytes.len() as u64))
            .collect()
    }
}

/// Chave do loader em `dependencies` do `.mrpack`.
pub(super) fn mrpack_loader_key(loader: &str) -> Option<&'static str> {
    match loader {
        "fabric" => Some("fabric-loader"),
        "quilt" => Some("quilt-loader"),
        "forge" => Some("forge"),
        "neoforge" => Some("neoforge"),
        _ => None,
    }
}

fn utf8<'a>(bytes: Option<&'a Vec<u8>>, name: &str) -> Result<&'a str> {
    let bytes = bytes.ok_or_else(|| Error::internal(format!("{name} ausente")))?;
    std::str::from_utf8(bytes).map_err(|_| Error::internal(format!("{name} não é UTF-8")))
}

fn read_ref(path: &str, bytes: &[u8]) -> Result<RefItem> {
    let bad = |what: &str| {
        Error::new(
            ExportErrorCode::PrismUnsupportedPack,
            format!("{path}: {what}"),
        )
    };
    let text = std::str::from_utf8(bytes).map_err(|_| bad("não é UTF-8"))?;
    let meta = Metafile::parse(text)
        .map_err(|e| bad(&e.to_string()))?
        .value;
    let format = meta.hash_format().map_err(|e| bad(&e.to_string()))?;
    let hash = meta.download.hash.trim();
    if hash.is_empty() {
        return Err(bad("sem hash do download"));
    }
    let filename = meta
        .filename
        .rsplit('/')
        .next()
        .unwrap_or_default()
        .to_owned();
    if filename.is_empty() {
        return Err(bad("sem nome de arquivo"));
    }
    let folder = path.rsplit_once('/').map_or("", |(dir, _)| dir);
    let dest = if folder.is_empty() {
        filename.clone()
    } else {
        format!("{folder}/{filename}")
    };
    check_relative_path(&dest).map_err(|e| bad(&e.to_string()))?;
    let source = if meta.is_curseforge_mode() {
        let (project, file) = meta
            .curseforge_ids()
            .ok_or_else(|| bad("CurseForge sem [update.curseforge]"))?;
        RefSource::Curseforge { project, file }
    } else {
        let url = meta.download.url.trim().to_owned();
        let parsed = Url::parse(&url).map_err(|_| bad("endereço do download inválido"))?;
        if parsed.scheme() != "https" {
            return Err(bad("o Prism só baixa por https"));
        }
        if parsed.host_str() == Some(MODRINTH_CDN_HOST) {
            RefSource::Modrinth { url }
        } else {
            RefSource::Link { url }
        }
    };
    Ok(RefItem {
        meta_path: path.to_owned(),
        name: if meta.name.is_empty() {
            filename.clone()
        } else {
            meta.name.clone()
        },
        dest,
        filename,
        side: meta.side.clone(),
        optional: meta.option.as_ref().is_some_and(|o| o.optional),
        source,
        expected: ExpectedHash::new(format, hash),
    })
}

/// Endereço da CDN da CurseForge de um arquivo: `files/<id ÷ 1000>/<id mod 1000>/<nome>`.
/// É o único lugar que monta esse endereço (ADR-0050); nada vem da resposta da API.
pub(super) fn curseforge_cdn_url(base: &Url, file_id: u32, filename: &str) -> Result<Url> {
    let mut url = base.clone();
    url.path_segments_mut()
        .map_err(|()| Error::internal("endereço base da CDN inválido"))?
        .pop_if_empty()
        .push("files")
        .push(&(file_id / 1000).to_string())
        .push(&(file_id % 1000).to_string())
        .push(filename);
    Ok(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_da_cdn_usa_id_dividido_e_resto() {
        let base = Url::parse("https://edge.forgecdn.net").unwrap();
        assert_eq!(
            curseforge_cdn_url(&base, 4_712_345, "Mod Bom+1.jar")
                .unwrap()
                .as_str(),
            "https://edge.forgecdn.net/files/4712/345/Mod%20Bom+1.jar"
        );
        assert_eq!(
            curseforge_cdn_url(&base, 5000, "a.jar").unwrap().path(),
            "/files/5/0/a.jar"
        );
    }

    #[test]
    fn loader_sem_chave_no_mrpack() {
        assert_eq!(mrpack_loader_key("fabric"), Some("fabric-loader"));
        assert_eq!(mrpack_loader_key("liteloader"), None);
    }
}
