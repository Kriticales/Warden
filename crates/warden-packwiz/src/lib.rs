//! Crate `warden-packwiz`: o formato packwiz lido e escrito exatamente como o packwiz faz
//! (ARCHITECTURE §6.1 a §6.4; ADR-0006).
//!
//! - Modelos de `pack.toml` ([`PackManifest`]), `index.toml` ([`PackIndex`]) e metafiles
//!   `.pw.toml` ([`Metafile`]), com todos os campos do packwiz (R3 §1.5 a §1.7), inclusive
//!   `[option]`, `pin`, `preserve`, `[export.curseforge]` e `[options]`.
//! - Leitura tolerante da pasta do pack ([`read_pack`]): erro por arquivo.
//! - Escrita de arquivo novo byte a byte igual à do packwiz (`to_toml_string`), provada contra
//!   fixtures geradas pelo packwiz real (`cargo xtask fixtures-packwiz`).
//! - Edição mínima de arquivos existentes com `toml_edit` ([`edit`]).
//! - Nomes únicos de metafile ([`unique_metafile_stem`]) e lado a partir do Modrinth
//!   ([`side_from_modrinth`]).
//! - `.packwizignore` com a semântica exata do packwiz ([`PackwizIgnore`]).
//! - Hashes `sha1`, `sha256`, `sha512`, `md5` e `murmur2` da CurseForge ([`hash`]).
//! - Modelos de arquivos de controle e verificação de higiene ([`hygiene`]).
//!
//! Esta crate só lê e monta texto: quem grava no pack é a `PackTransaction` da
//! `warden-project` (ARCHITECTURE §6.5). Executar o packwiz é papel da `warden-packwiz-cli`.

mod decode;
pub mod edit;
mod encode;
mod error;
pub mod hash;
pub mod hygiene;
mod ignore;
mod index;
mod metafile;
mod naming;
mod pack;
mod read;
mod value;

pub use error::{Error, PackwizErrorCode, Result};
pub use hash::HashFormat;
pub use ignore::{PACKWIZ_DEFAULT_PATTERNS, PackwizIgnore};
pub use index::{IndexEntry, METAFILE_SUFFIX, PackIndex, check_relative_path, clean_path};
pub use metafile::{
    CurseForgeFile, Download, MODE_CURSEFORGE, Metafile, ModOption, ModrinthFile, Side, SideNote,
    filename_from_url, side_from_modrinth,
};
pub use naming::{MetafileSource, metafile_stem, slugify_name, unique_metafile_stem};
pub use pack::{
    CURRENT_PACK_FORMAT, DEFAULT_INDEX_FILE, IndexRef, Loader, Migration, PackManifest, Parsed,
};
pub use read::{MetafileRead, PACK_FILE, PACKWIZIGNORE_FILE, PackRead, read_metafile, read_pack};
pub use value::Value;
