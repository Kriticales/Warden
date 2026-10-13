//! Índice de IDs para o autocompletar do editor de scripts (ARCHITECTURE §10.2).
//!
//! Os IDs de cada jar vêm de [`warden_jarmeta::assets`] e ficam no cache
//! `cache/scriptids/<sha256 do jar>.json` ([`IdCache`]): os IDs só dependem dos bytes do jar,
//! então o hash basta como chave, e abrir o editor de novo não abre nenhum jar. A interface
//! recebe o índice uma vez por arquivo aberto e filtra na memória a cada tecla.

use std::fmt::Write as _;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use warden_jarmeta::Limits;
use warden_jarmeta::assets::{AssetIds, read_asset_ids_file};

/// Nome da pasta do cache dentro de `cache/` da pasta de dados.
pub const IDS_CACHE_DIR: &str = "scriptids";

/// Versão do formato gravado no cache. Outra versão é ignorada e refeita.
pub const IDS_FORMAT: u32 = 1;

/// Um jar a indexar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptJar {
    /// De onde os IDs vêm, como a interface mostra (o nome do mod, ou "Minecraft").
    pub source: String,
    /// Onde está o jar.
    pub path: PathBuf,
    /// SHA-256 do jar, se o chamador já sabe (o cache de downloads guarda por ele).
    pub sha256: Option<String>,
}

/// Os IDs de uma origem, em ordem.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct IdGroup {
    /// De onde os IDs vêm ("Minecraft", "Create"…).
    pub source: String,
    /// Itens.
    pub items: Vec<String>,
    /// Blocos.
    pub blocks: Vec<String>,
    /// Tags de item, sem o `#`.
    pub item_tags: Vec<String>,
    /// Tags de bloco, sem o `#`.
    pub block_tags: Vec<String>,
    /// Tags de fluido, sem o `#`.
    pub fluid_tags: Vec<String>,
    /// Receitas.
    pub recipes: Vec<String>,
}

impl IdGroup {
    /// O grupo de uma origem.
    #[must_use]
    pub fn new(source: impl Into<String>, ids: AssetIds) -> Self {
        Self {
            source: source.into(),
            items: ids.items.into_iter().collect(),
            blocks: ids.blocks.into_iter().collect(),
            item_tags: ids.item_tags.into_iter().collect(),
            block_tags: ids.block_tags.into_iter().collect(),
            fluid_tags: ids.fluid_tags.into_iter().collect(),
            recipes: ids.recipes.into_iter().collect(),
        }
    }

    fn is_empty(&self) -> bool {
        self.items.is_empty()
            && self.blocks.is_empty()
            && self.item_tags.is_empty()
            && self.block_tags.is_empty()
            && self.fluid_tags.is_empty()
            && self.recipes.is_empty()
    }
}

/// O índice de IDs do pack.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ScriptIds {
    /// Uma entrada por origem com IDs (origens sem nenhum ID ficam de fora).
    pub groups: Vec<IdGroup>,
    /// Jars que não deu para ler (a lista fica sem eles).
    pub unread: Vec<String>,
}

/// Números da montagem.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct IdsStats {
    /// Jars pedidos.
    pub jars: usize,
    /// Jars cujos IDs vieram do cache.
    pub cache_hits: usize,
    /// Jars abertos.
    pub jars_opened: usize,
}

/// O resultado de [`build_ids`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdsBuild {
    /// O índice.
    pub ids: ScriptIds,
    /// Os números.
    pub stats: IdsStats,
}

#[derive(Serialize, Deserialize)]
struct Cached {
    format: u32,
    sha256: String,
    ids: AssetIds,
}

/// Cache dos IDs por hash do jar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdCache {
    dir: PathBuf,
}

impl IdCache {
    /// Cache em `<cache>/scriptids`.
    #[must_use]
    pub fn in_cache_dir(cache_dir: &Path) -> Self {
        Self {
            dir: cache_dir.join(IDS_CACHE_DIR),
        }
    }

    fn path_for(&self, sha256: &str) -> Option<PathBuf> {
        is_sha256(sha256).then(|| self.dir.join(format!("{sha256}.json")))
    }

    /// Os IDs guardados, se existirem, forem legíveis, do formato atual e do mesmo hash.
    #[must_use]
    pub fn load(&self, sha256: &str) -> Option<AssetIds> {
        let bytes = std::fs::read(self.path_for(sha256)?).ok()?;
        let cached: Cached = serde_json::from_slice(&bytes).ok()?;
        (cached.format == IDS_FORMAT && cached.sha256 == sha256).then_some(cached.ids)
    }

    /// Grava os IDs (atomicamente). Falhar não impede o editor: o próximo pedido relê o jar.
    pub fn store(&self, sha256: &str, ids: &AssetIds) {
        let Some(path) = self.path_for(sha256) else {
            return;
        };
        let cached = Cached {
            format: IDS_FORMAT,
            sha256: sha256.to_owned(),
            ids: ids.clone(),
        };
        let result = std::fs::create_dir_all(&self.dir)
            .map_err(|e| e.to_string())
            .and_then(|()| serde_json::to_vec(&cached).map_err(|e| e.to_string()))
            .and_then(|json| warden_core::atomic_write(&path, &json).map_err(|e| e.to_string()));
        if let Err(error) = result {
            tracing::debug!(%error, "não deu para gravar o cache de IDs dos scripts");
        }
    }
}

/// 64 dígitos hexadecimais minúsculos.
fn is_sha256(text: &str) -> bool {
    text.len() == 64 && text.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// SHA-256 de um arquivo, lido em blocos.
///
/// # Errors
///
/// Falha de leitura do disco.
pub fn sha256_file(path: &Path) -> std::io::Result<String> {
    let mut reader = BufReader::new(File::open(path)?);
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; 64 * 1024];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher
        .finalize()
        .iter()
        .fold(String::with_capacity(64), |mut out, byte| {
            let _ = write!(out, "{byte:02x}");
            out
        }))
}

/// Monta o índice dos jars, com o cache quando houver. Os grupos saem na ordem dos jars, com
/// as origens repetidas juntadas (dois jars do mesmo mod viram um grupo).
#[must_use]
pub fn build_ids(jars: &[ScriptJar], cache: Option<&IdCache>, limits: &Limits) -> IdsBuild {
    let mut stats = IdsStats {
        jars: jars.len(),
        ..IdsStats::default()
    };
    let mut groups: Vec<(String, AssetIds)> = Vec::new();
    let mut unread = Vec::new();
    for jar in jars {
        let sha256 = match &jar.sha256 {
            Some(sha) => Some(sha.to_ascii_lowercase()),
            None => sha256_file(&jar.path).ok(),
        };
        let cached = sha256
            .as_deref()
            .and_then(|sha| cache.and_then(|cache| cache.load(sha)));
        let ids = if let Some(ids) = cached {
            stats.cache_hits += 1;
            ids
        } else {
            stats.jars_opened += 1;
            match read_asset_ids_file(&jar.path, limits) {
                Ok(ids) => {
                    if let (Some(cache), Some(sha)) = (cache, sha256.as_deref()) {
                        cache.store(sha, &ids);
                    }
                    ids
                }
                Err(error) => {
                    tracing::debug!(%error, origem = %jar.source, "jar sem IDs para os scripts");
                    unread.push(jar.source.clone());
                    continue;
                }
            }
        };
        match groups.iter_mut().find(|(source, _)| *source == jar.source) {
            Some((_, existing)) => existing.extend(ids),
            None => groups.push((jar.source.clone(), ids)),
        }
    }
    let groups = groups
        .into_iter()
        .map(|(source, ids)| IdGroup::new(source, ids))
        .filter(|group| !group.is_empty())
        .collect();
    unread.sort();
    unread.dedup();
    IdsBuild {
        ids: ScriptIds { groups, unread },
        stats,
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Write};

    use zip::write::SimpleFileOptions;

    use super::*;

    fn write_jar(path: &Path, entries: &[&str]) {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for name in entries {
            writer
                .start_file(*name, SimpleFileOptions::default())
                .unwrap();
            writer.write_all(b"{}").unwrap();
        }
        std::fs::write(path, writer.finish().unwrap().into_inner()).unwrap();
    }

    fn jar(source: &str, path: PathBuf) -> ScriptJar {
        ScriptJar {
            source: source.into(),
            path,
            sha256: None,
        }
    }

    #[test]
    fn monta_com_cache_por_hash() {
        let dir = tempfile::tempdir().unwrap();
        let vanilla = dir.path().join("1.20.1.jar");
        write_jar(
            &vanilla,
            &[
                "assets/minecraft/models/item/copper_ingot.json",
                "assets/minecraft/blockstates/copper_block.json",
            ],
        );
        let create = dir.path().join("create.jar");
        write_jar(&create, &["data/forge/tags/items/ingots/zinc.json"]);
        let broken = dir.path().join("quebrado.jar");
        std::fs::write(&broken, b"nada").unwrap();
        let jars = vec![
            jar("Minecraft", vanilla.clone()),
            jar("Create", create),
            jar("Quebrado", broken),
            jar("Sumido", dir.path().join("sumido.jar")),
        ];
        let cache = IdCache::in_cache_dir(&dir.path().join("cache"));

        let first = build_ids(&jars, Some(&cache), &Limits::default());
        assert_eq!(
            first.stats,
            IdsStats {
                jars: 4,
                cache_hits: 0,
                jars_opened: 4
            }
        );
        assert_eq!(first.ids.groups.len(), 2);
        assert_eq!(first.ids.groups[0].source, "Minecraft");
        assert_eq!(first.ids.groups[0].items, ["minecraft:copper_ingot"]);
        assert_eq!(first.ids.groups[0].blocks, ["minecraft:copper_block"]);
        assert_eq!(first.ids.groups[1].item_tags, ["forge:ingots/zinc"]);
        assert_eq!(first.ids.unread, ["Quebrado", "Sumido"]);

        // A segunda montagem não abre os jars que deram certo.
        let second = build_ids(&jars, Some(&cache), &Limits::default());
        assert_eq!(second.stats.cache_hits, 2);
        assert_eq!(second.stats.jars_opened, 2);
        assert_eq!(second.ids, first.ids);

        // O jar mudou: hash novo, relido.
        write_jar(&vanilla, &["assets/minecraft/models/item/iron_ingot.json"]);
        let third = build_ids(&jars[..1], Some(&cache), &Limits::default());
        assert_eq!(third.stats.jars_opened, 1);
        assert_eq!(third.ids.groups[0].items, ["minecraft:iron_ingot"]);
    }

    #[test]
    fn hash_conhecido_e_origens_repetidas() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.jar");
        write_jar(&a, &["assets/a/models/item/x.json"]);
        let b = dir.path().join("b.jar");
        write_jar(&b, &["assets/a/models/item/y.json"]);
        let cache = IdCache::in_cache_dir(dir.path());
        let sha = sha256_file(&a).unwrap();
        let jars = vec![
            ScriptJar {
                source: "A".into(),
                path: a,
                sha256: Some(sha.to_uppercase()),
            },
            jar("A", b),
        ];
        let build = build_ids(&jars, Some(&cache), &Limits::default());
        assert_eq!(build.ids.groups.len(), 1);
        assert_eq!(build.ids.groups[0].items, ["a:x", "a:y"]);
        assert!(cache.load(&sha).is_some());
        // Hash inválido nunca vira caminho.
        assert!(cache.load("../x").is_none());
        cache.store("../x", &AssetIds::default());
    }

    #[test]
    fn cache_de_outro_formato_e_ignorado() {
        let dir = tempfile::tempdir().unwrap();
        let cache = IdCache::in_cache_dir(dir.path());
        let sha = "a".repeat(64);
        cache.store(&sha, &AssetIds::default());
        assert!(cache.load(&sha).is_some());
        let path = cache.path_for(&sha).unwrap();
        std::fs::write(
            &path,
            format!(r#"{{"format":99,"sha256":"{sha}","ids":{{"items":[],"blocks":[],"itemTags":[],"blockTags":[],"fluidTags":[],"recipes":[]}}}}"#),
        )
        .unwrap();
        assert!(cache.load(&sha).is_none());
        std::fs::write(&path, "{").unwrap();
        assert!(cache.load(&sha).is_none());
    }
}
