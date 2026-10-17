//! IDs de itens, blocos, tags e receitas declarados por um jar (C-07; ARCHITECTURE §10.2).
//!
//! O editor de scripts (KubeJS e CraftTweaker) completa `minecraft:copper_ingot` e
//! `#forge:ingots/iron` sem abrir o jogo. Os IDs saem só dos **nomes** das entradas do zip,
//! então nada é descompactado:
//!
//! - item: `assets/<ns>/models/item/<caminho>.json`;
//! - bloco: `assets/<ns>/blockstates/<caminho>.json`;
//! - tag de item, bloco ou fluido: `data/<ns>/tags/{items,item}/…`, `{blocks,block}/…` e
//!   `{fluids,fluid}/…` (o plural até a 1.20.6; o singular a partir da 1.21);
//! - receita: `data/<ns>/recipes/…` (até a 1.20.6) e `data/<ns>/recipe/…` (1.21+).
//!
//! Limitação documentada: itens e blocos registrados só por código, sem modelo nem estado de
//! bloco no jar, ficam de fora (com o jogo aberto e o KubeJS 7, o servidor web do KubeJS completa
//! a lista). Jars embutidos não são abertos: costumam ser bibliotecas sem conteúdo de jogo.

use std::collections::BTreeSet;
use std::fs::File;
use std::io::{BufReader, Cursor, Read, Seek};
use std::path::Path;

use serde::{Deserialize, Serialize};
use zip::ZipArchive;

use crate::error::{Error, LimitKind};
use crate::limits::Limits;

/// Os IDs de um jar, cada lista em ordem e sem repetição.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetIds {
    /// Itens (`minecraft:copper_ingot`).
    pub items: BTreeSet<String>,
    /// Blocos (`minecraft:copper_block`).
    pub blocks: BTreeSet<String>,
    /// Tags de item, sem o `#` (`forge:ingots/iron`).
    pub item_tags: BTreeSet<String>,
    /// Tags de bloco, sem o `#`.
    pub block_tags: BTreeSet<String>,
    /// Tags de fluido, sem o `#`.
    pub fluid_tags: BTreeSet<String>,
    /// Receitas (`minecraft:copper_block`).
    pub recipes: BTreeSet<String>,
}

impl AssetIds {
    /// Nenhum ID.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
            && self.blocks.is_empty()
            && self.item_tags.is_empty()
            && self.block_tags.is_empty()
            && self.fluid_tags.is_empty()
            && self.recipes.is_empty()
    }

    /// Junta os IDs de outro jar.
    pub fn extend(&mut self, other: Self) {
        self.items.extend(other.items);
        self.blocks.extend(other.blocks);
        self.item_tags.extend(other.item_tags);
        self.block_tags.extend(other.block_tags);
        self.fluid_tags.extend(other.fluid_tags);
        self.recipes.extend(other.recipes);
    }

    /// Classifica o nome de uma entrada do zip e guarda o ID, se for um.
    pub fn add_entry(&mut self, name: &str) {
        if let Some((kind, id)) = classify(name) {
            let set = match kind {
                Kind::Item => &mut self.items,
                Kind::Block => &mut self.blocks,
                Kind::ItemTag => &mut self.item_tags,
                Kind::BlockTag => &mut self.block_tags,
                Kind::FluidTag => &mut self.fluid_tags,
                Kind::Recipe => &mut self.recipes,
            };
            set.insert(id);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Item,
    Block,
    ItemTag,
    BlockTag,
    FluidTag,
    Recipe,
}

/// O tipo e o ID de uma entrada, ou `None` se ela não declara um ID.
fn classify(name: &str) -> Option<(Kind, String)> {
    let stem = name.strip_suffix(".json")?;
    let mut parts = stem.splitn(3, '/');
    let root = parts.next()?;
    let namespace = parts.next()?;
    let rest = parts.next()?;
    if !valid_namespace(namespace) {
        return None;
    }
    let (kind, path) = match root {
        "assets" => {
            if let Some(path) = rest.strip_prefix("models/item/") {
                (Kind::Item, path)
            } else {
                (Kind::Block, rest.strip_prefix("blockstates/")?)
            }
        }
        "data" => {
            if let Some(tag) = rest.strip_prefix("tags/") {
                let (folder, path) = tag.split_once('/')?;
                let kind = match folder {
                    "items" | "item" => Kind::ItemTag,
                    "blocks" | "block" => Kind::BlockTag,
                    "fluids" | "fluid" => Kind::FluidTag,
                    _ => return None,
                };
                (kind, path)
            } else {
                let path = rest
                    .strip_prefix("recipes/")
                    .or_else(|| rest.strip_prefix("recipe/"))?;
                (Kind::Recipe, path)
            }
        }
        _ => return None,
    };
    valid_path(path).then(|| (kind, format!("{namespace}:{path}")))
}

/// Espaço de nomes de um `ResourceLocation`: `[a-z0-9_.-]+`.
fn valid_namespace(text: &str) -> bool {
    !text.is_empty()
        && text != "."
        && text != ".."
        && text
            .bytes()
            .all(|b| matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'_' | b'.' | b'-'))
}

/// Caminho de um `ResourceLocation`: `[a-z0-9_./-]+`, sem partes vazias.
fn valid_path(text: &str) -> bool {
    !text.is_empty()
        && !text.starts_with('/')
        && !text.ends_with('/')
        && !text
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
        && text
            .bytes()
            .all(|b| matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'_' | b'.' | b'-' | b'/'))
}

/// Lê os IDs de um jar no disco.
///
/// # Errors
///
/// [`Error::Read`] se o arquivo não abre, [`Error::LimitExceeded`] se passa dos limites e
/// [`Error::InvalidArchive`] se não é um zip.
pub fn read_asset_ids_file(path: &Path, limits: &Limits) -> Result<AssetIds, Error> {
    let read_error = |source| Error::Read {
        path: path.to_owned(),
        source,
    };
    let file = File::open(path).map_err(read_error)?;
    let size = file.metadata().map_err(read_error)?.len();
    check_size(size, limits)?;
    read_archive(BufReader::new(file), limits)
}

/// Lê os IDs de um jar já na memória.
///
/// # Errors
///
/// Os mesmos de [`read_asset_ids_file`], exceto a leitura do disco.
pub fn read_asset_ids_bytes(bytes: &[u8], limits: &Limits) -> Result<AssetIds, Error> {
    check_size(u64::try_from(bytes.len()).unwrap_or(u64::MAX), limits)?;
    read_archive(Cursor::new(bytes), limits)
}

fn check_size(size: u64, limits: &Limits) -> Result<(), Error> {
    if size > limits.max_file_bytes {
        return Err(Error::LimitExceeded {
            kind: LimitKind::FileSize,
            limit: limits.max_file_bytes,
            actual: size,
        });
    }
    Ok(())
}

fn read_archive<R: Read + Seek>(reader: R, limits: &Limits) -> Result<AssetIds, Error> {
    let zip = ZipArchive::new(reader).map_err(|error| Error::InvalidArchive {
        detail: error.to_string(),
    })?;
    if zip.len() > limits.max_entries {
        return Err(Error::LimitExceeded {
            kind: LimitKind::EntryCount,
            limit: u64::try_from(limits.max_entries).unwrap_or(u64::MAX),
            actual: u64::try_from(zip.len()).unwrap_or(u64::MAX),
        });
    }
    let mut ids = AssetIds::default();
    // Um nome que não decodifica (nem UTF-8 nem CP437) fica de fora, como entrada ilegível.
    for name in zip.file_names().flatten() {
        ids.add_entry(&name);
    }
    Ok(ids)
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use zip::write::SimpleFileOptions;

    use super::*;

    fn jar(entries: &[&str]) -> Vec<u8> {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for name in entries {
            writer
                .start_file(*name, SimpleFileOptions::default())
                .unwrap();
            writer.write_all(b"{}").unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    fn set(items: &[&str]) -> BTreeSet<String> {
        items.iter().map(|&item| item.to_owned()).collect()
    }

    #[test]
    fn classifica_itens_blocos_tags_e_receitas() {
        let bytes = jar(&[
            "assets/minecraft/models/item/copper_ingot.json",
            "assets/minecraft/models/block/copper_block.json",
            "assets/minecraft/blockstates/copper_block.json",
            "assets/create/models/item/crushed_ore/copper.json",
            "data/forge/tags/items/ingots/iron.json",
            "data/c/tags/item/ingots/copper.json",
            "data/minecraft/tags/blocks/mineable/pickaxe.json",
            "data/minecraft/tags/fluid/water.json",
            "data/minecraft/tags/entity_types/raiders.json",
            "data/minecraft/recipes/copper_block.json",
            "data/minecraft/recipe/lightning_rod.json",
            "data/minecraft/advancements/story/root.json",
            "assets/minecraft/textures/item/copper_ingot.png",
            "META-INF/MANIFEST.MF",
        ]);
        let ids = read_asset_ids_bytes(&bytes, &Limits::default()).unwrap();
        assert_eq!(
            ids.items,
            set(&["create:crushed_ore/copper", "minecraft:copper_ingot"])
        );
        assert_eq!(ids.blocks, set(&["minecraft:copper_block"]));
        assert_eq!(
            ids.item_tags,
            set(&["c:ingots/copper", "forge:ingots/iron"])
        );
        assert_eq!(ids.block_tags, set(&["minecraft:mineable/pickaxe"]));
        assert_eq!(ids.fluid_tags, set(&["minecraft:water"]));
        assert_eq!(
            ids.recipes,
            set(&["minecraft:copper_block", "minecraft:lightning_rod"])
        );
    }

    #[test]
    fn ignora_nomes_que_nao_sao_ids() {
        let bytes = jar(&[
            "assets/Minecraft/models/item/x.json",
            "assets/minecraft/models/item/Copper.json",
            "assets/minecraft/models/item/.json",
            "assets/minecraft/models/item/a//b.json",
            "data/minecraft/tags/items/",
            "data/minecraft/tags/items.json",
            "assets/../models/item/x.json",
            "assets/a/models/item/../x.json",
            "assets/minecraft/models/item/x.json.bak",
        ]);
        let ids = read_asset_ids_bytes(&bytes, &Limits::default()).unwrap();
        assert!(ids.is_empty(), "{ids:?}");
    }

    #[test]
    fn junta_e_respeita_limites() {
        let mut a =
            read_asset_ids_bytes(&jar(&["assets/a/models/item/x.json"]), &Limits::default())
                .unwrap();
        let b = read_asset_ids_bytes(
            &jar(&["assets/b/models/item/y.json", "assets/b/models/item/z.json"]),
            &Limits::default(),
        )
        .unwrap();
        a.extend(b);
        assert_eq!(a.items, set(&["a:x", "b:y", "b:z"]));

        let limits = Limits {
            max_entries: 1,
            ..Limits::default()
        };
        let error = read_asset_ids_bytes(
            &jar(&["assets/b/models/item/y.json", "assets/b/models/item/z.json"]),
            &limits,
        )
        .unwrap_err();
        assert!(matches!(
            error,
            Error::LimitExceeded {
                kind: LimitKind::EntryCount,
                ..
            }
        ));
        let small = Limits {
            max_file_bytes: 3,
            ..Limits::default()
        };
        assert!(matches!(
            read_asset_ids_bytes(b"abcd", &small),
            Err(Error::LimitExceeded {
                kind: LimitKind::FileSize,
                ..
            })
        ));
        assert!(matches!(
            read_asset_ids_bytes(b"nada", &Limits::default()),
            Err(Error::InvalidArchive { .. })
        ));
    }

    #[test]
    fn le_do_disco() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.jar");
        std::fs::write(&path, jar(&["assets/a/models/item/x.json"])).unwrap();
        let ids = read_asset_ids_file(&path, &Limits::default()).unwrap();
        assert_eq!(ids.items, set(&["a:x"]));
        assert!(matches!(
            read_asset_ids_file(&dir.path().join("falta.jar"), &Limits::default()),
            Err(Error::Read { .. })
        ));
    }
}
