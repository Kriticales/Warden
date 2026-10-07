//! Materialização do pack na instância de teste (ARCHITECTURE §8.2; ADR-0011).
//!
//! A semântica é a do packwiz-installer 0.5.14 (`UpdateManager.kt` e `DownloadTask.kt`), que o
//! teste de conformidade (`tests/conformance_installer.rs`) compara com o instalador real:
//!
//! 1. O pack é lido pela `warden-packwiz` (P1-01). Caminhos do índice são relativos à pasta do
//!    índice; o destino de um metafile é a pasta dele mais o `filename`; `alias` troca o
//!    destino.
//! 2. `side = "server"` não entra no cliente (e vice-versa); opcional desligado não entra
//!    ([`OptionalChoices`]). Os dois, se já estavam na instância, são removidos.
//! 3. `preserve = true` com o destino já existente: não toca.
//! 4. Destino com o hash esperado: não toca. Para não reler tudo a cada teste, o manifesto
//!    guarda tamanho e data de cada arquivo; iguais aos do disco e item igual no índice, o
//!    arquivo nem é lido. A segunda materialização sem mudanças não baixa nem grava nada.
//! 5. Downloads vão para o cache por hash ([`DownloadCache`], P1-03 por baixo), no máximo
//!    [`MaterializeOptions::max_parallel_downloads`] ao mesmo tempo; a CurseForge é consultada
//!    na hora, em lote, só para o que não está no cache, e o endereço nunca é gravado. Os
//!    cabeçalhos do download são os da `warden-curseforge` (P1-04): `x-api-key` também na CDN.
//! 6. A cópia para a instância é atômica (`.warden-tmp` + renomeação) e confere o hash de novo.
//! 7. Some da instância o que estava no manifesto e saiu do pack (só o que o Warden instalou).
//! 8. Antes de sobrescrever ou apagar um arquivo gerenciado que mudou na instância (ou um
//!    arquivo desconhecido no lugar de um item do pack), com [`OnModified::Pause`] nada é
//!    feito e o resultado é [`Outcome::NeedsReview`] (T15).
//! 9. Mods da CurseForge bloqueados para apps de terceiros não interrompem: vão para
//!    [`Report::blocked`] (T20).
//!
//! Diferenças deliberadas em relação ao packwiz-installer, todas para a instância de teste:
//! - sem janela (`-g`) o instalador liga **todos** os opcionais; o Warden usa as escolhas da
//!   instância e, sem escolha, o `default` do item (SPEC T11);
//! - o instalador confia no próprio manifesto e mantém um arquivo que o jogo alterou enquanto o
//!   item não muda no pack; o Warden confere o disco e devolve a instância ao estado do pack,
//!   depois da revisão (item 8);
//! - um arquivo do pack cujo hash no índice está desatualizado é copiado como está no pack
//!   (o instalador recusaria) e aparece em [`Report::stale_index`].
//!
//! Cancelar ([`CancellationToken`]) para os downloads e as cópias entre pedaços; o manifesto é
//! gravado com o que já foi feito, e nenhum arquivo incompleto fica com o nome final (só os
//! `.part` no cache, retomados ou limpos depois).

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use warden_core::{
    AppPaths, CancellationToken, DomainError as _, NoProgress, PackId, Progress, ProgressSink,
    TEMP_SUFFIX, resolve_inside,
};
use warden_curseforge::{CurseforgeClient, Distribution, FileRef};
use warden_http::{DownloadRequest, ExpectedHash, HttpClient, Url};
use warden_packwiz::hash::hash_matches;
use warden_packwiz::{
    HashFormat, IndexEntry, Loader, Metafile, PackManifest, Side, clean_path, read_pack,
};

use crate::blocked::BlockedFile;
use crate::downloads::{
    DownloadCache, Fetched, FileOrigin, STALE_PART_AGE, copy_hashing, hash_file, remove_if_exists,
    rename_with_retry,
};
use crate::error::{Error, FailedItem, FailureReason, Result};
use crate::manifest::{EntryOrigin, Manifest, ManifestEntry, ModrinthIds, modified_ns};
use crate::optional_choices::OptionalChoices;
use crate::player_tools::{PlayerToolsMode, excluded_paths};

/// Identificador da etapa no progresso.
pub const STAGE: &str = "syncPack";
/// Chave do texto da etapa no catálogo da interface.
pub const STAGE_LABEL: &str = "test.stage.syncPack";

/// Pastas de uma instância.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstanceDirs {
    /// `gameDir` (`instances/<pack-id>/minecraft`).
    pub game_dir: PathBuf,
    /// Estado do Warden, fora do `gameDir` (`instances/<pack-id>/state`).
    pub state_dir: PathBuf,
}

impl InstanceDirs {
    /// Pastas da instância de teste de um pack (ARCHITECTURE §8.1).
    #[must_use]
    pub fn for_pack(paths: &AppPaths, pack: PackId) -> Self {
        let root = paths.instance_dir(pack);
        Self {
            game_dir: root.join("minecraft"),
            state_dir: root.join("state"),
        }
    }
}

/// Lado instalado.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum InstallSide {
    /// Cliente (a instância de teste).
    #[default]
    Client,
    /// Servidor (servidor local, T27).
    Server,
}

impl InstallSide {
    /// Texto gravado no manifesto.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Client => "client",
            Self::Server => "server",
        }
    }

    fn wants(self, side: &Side) -> Option<bool> {
        match side {
            Side::Unset | Side::Both => Some(true),
            Side::Client => Some(self == Self::Client),
            Side::Server => Some(self == Self::Server),
            Side::Other(_) => None,
        }
    }
}

/// O que fazer quando um arquivo gerenciado mudou na instância e precisaria ser sobrescrito
/// ou apagado.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum OnModified {
    /// Não mexer em nada e devolver a lista ([`Outcome::NeedsReview`]): há mudanças do teste
    /// ainda não revisadas (T15).
    #[default]
    Pause,
    /// Sobrescrever: a revisão já foi feita (ou a pessoa escolheu "Descartar e testar").
    Overwrite,
}

/// De onde vêm as escolhas dos opcionais.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum OptionalSelection {
    /// As gravadas na instância (`state/optional-choices.json`), com o padrão de cada item.
    #[default]
    Saved,
    /// As dadas.
    Explicit(OptionalChoices),
}

/// Opções da materialização.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterializeOptions {
    /// Lado.
    pub side: InstallSide,
    /// Arquivos alterados na instância.
    pub on_modified: OnModified,
    /// Opcionais.
    pub optionals: OptionalSelection,
    /// Quais ferramentas do jogador (spark, Crash Assistant) entram (D16; [`crate::player_tools`]).
    pub player_tools: PlayerToolsMode,
    /// Downloads simultâneos.
    pub max_parallel_downloads: usize,
}

impl Default for MaterializeOptions {
    fn default() -> Self {
        Self {
            side: InstallSide::Client,
            on_modified: OnModified::Pause,
            optionals: OptionalSelection::Saved,
            player_tools: PlayerToolsMode::Normal,
            max_parallel_downloads: 6,
        }
    }
}

/// O que a materialização usa de fora: cliente HTTP, CurseForge (com a chave, quando houver)
/// e cache de downloads. Barato de clonar.
#[derive(Debug, Clone)]
pub struct Sources {
    /// Cliente HTTP da `warden-http`.
    pub http: HttpClient,
    /// Cliente da CurseForge; `None` quando não há chave.
    pub curseforge: Option<CurseforgeClient>,
    /// Cache de downloads.
    pub cache: Arc<DownloadCache>,
}

/// O que aconteceria com um arquivo alterado.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum ModifiedAction {
    /// Seria substituído pela versão do pack.
    Overwrite,
    /// Seria apagado (saiu do pack, ficou desligado ou é do outro lado).
    Remove,
}

/// Arquivo da instância que mudou desde que o Warden o colocou lá (ou que o Warden não
/// conhece) e seria sobrescrito ou apagado.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ModifiedFile {
    /// Caminho relativo à pasta do jogo, com `/`.
    pub path: String,
    /// O que aconteceria.
    pub action: ModifiedAction,
    /// Se o arquivo estava no manifesto (`false`: arquivo desconhecido no lugar de um item).
    pub managed: bool,
}

/// Resultado de uma materialização completa (com ou sem itens bloqueados).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    /// Arquivos gravados na instância.
    pub written: u32,
    /// Arquivos baixados da internet (o resto veio do cache ou do pack).
    pub downloaded: u32,
    /// Arquivos apagados da instância.
    pub removed: u32,
    /// Itens que já estavam certos.
    pub unchanged: u32,
    /// Itens do outro lado (`side`).
    pub skipped_side: u32,
    /// Opcionais desligados.
    pub skipped_disabled: u32,
    /// Ferramentas do jogador deixadas de fora deste tipo de teste (D16).
    pub skipped_player_tools: u32,
    /// Itens com `preserve` que já existiam.
    pub preserved: u32,
    /// Mods da CurseForge que precisam de download manual (T20).
    pub blocked: Vec<BlockedFile>,
    /// Arquivos do pack cujo hash no índice não confere com o arquivo (índice desatualizado).
    pub stale_index: Vec<String>,
    /// Itens que não chegaram à instância (rede, CurseForge sem chave, metafile inválido…).
    /// O resto foi instalado e o manifesto, gravado.
    pub failed: Vec<FailedItem>,
    /// Se o manifesto foi regravado.
    pub manifest_written: bool,
}

impl Report {
    /// Se tudo chegou à instância (nenhuma falha e nenhum item bloqueado esperando a T20).
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.failed.is_empty() && self.blocked.is_empty()
    }

    /// O erro que lista os itens que faltaram ([`Error::Incomplete`]), se algum falhou. Os
    /// bloqueados não entram: eles vão para a T20.
    #[must_use]
    pub fn incomplete_error(&self) -> Option<Error> {
        (!self.failed.is_empty()).then(|| Error::Incomplete {
            items: self.failed.clone(),
        })
    }
}

/// Resultado da materialização.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Feita, talvez com itens bloqueados ([`Report::blocked`]) ou que falharam
    /// ([`Report::failed`]).
    Done(Report),
    /// Nada foi feito: há arquivos alterados na instância que seriam sobrescritos ou apagados.
    NeedsReview(Vec<ModifiedFile>),
}

/// Versão do jogo e do loader que a instância precisa, lidas do `pack.toml`. A versão do
/// loader é sempre a exata do pack (D7; S-R5-3 §13): é com ela que o motor do launcher instala
/// o loader, sem consultar o meta do loader na rede.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct GameRequirement {
    /// Versão do Minecraft.
    pub minecraft: String,
    /// Loader com a versão exata; `None` para vanilla.
    pub loader: Option<LoaderRequirement>,
}

/// Loader e versão exata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LoaderRequirement {
    /// Chave do loader em `[versions]` (`fabric`, `forge`, `neoforge`, `quilt`, `liteloader`).
    pub loader: String,
    /// Versão exata.
    pub version: String,
}

/// Versão do jogo e do loader do pack. Erros: sem versão do Minecraft, loader com versão não
/// exata (vazia, `latest`, `recommended`, faixa…) ou mais de um loader.
pub fn game_requirement(pack: &PackManifest) -> Result<GameRequirement> {
    let minecraft = pack
        .minecraft_version()
        .map(str::trim)
        .filter(|version| !version.is_empty())
        .ok_or(Error::MinecraftVersionMissing)?;
    let loaders = pack.loaders();
    if loaders.len() > 1 {
        return Err(Error::LoaderAmbiguous {
            loaders: loaders
                .iter()
                .map(|(loader, _)| loader.key().to_owned())
                .collect(),
        });
    }
    let loader = match loaders.first() {
        None => None,
        Some((loader, version)) => {
            if !is_exact_version(version) {
                return Err(Error::LoaderVersionNotExact {
                    loader: loader.key().to_owned(),
                    version: (*version).to_owned(),
                });
            }
            Some(LoaderRequirement {
                loader: Loader::key(*loader).to_owned(),
                version: version.trim().to_owned(),
            })
        }
    };
    Ok(GameRequirement {
        minecraft: minecraft.to_owned(),
        loader,
    })
}

/// Versão exata: não vazia, só letras, números, `.`, `-`, `+` e `_`, e não um apelido.
fn is_exact_version(version: &str) -> bool {
    let version = version.trim();
    !version.is_empty()
        && version
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '+' | '_'))
        && !["latest", "recommended", "stable", "newest"]
            .iter()
            .any(|alias| version.eq_ignore_ascii_case(alias))
        && version.chars().any(|c| c.is_ascii_digit())
}

/// Origem do conteúdo de um item.
#[derive(Debug, Clone)]
enum ItemSource {
    /// Arquivo do próprio pack.
    Pack { file: PathBuf },
    /// Download por link.
    Url {
        url: String,
        modrinth: Option<ModrinthIds>,
    },
    /// CurseForge, resolvido na hora.
    Curseforge { project: u32, file: u32 },
}

/// Um item do pack que deve estar na instância.
#[derive(Debug, Clone)]
struct Item {
    /// Caminho no índice (relativo à pasta do índice).
    index_path: String,
    /// Hash no índice.
    index_hash: String,
    /// Nome para mostrar.
    name: String,
    /// Nome do arquivo (para a T20).
    file_name: String,
    /// Destino relativo à pasta do jogo.
    dest: String,
    /// `preserve`.
    preserve: bool,
    /// Hash esperado do conteúdo; `None` num arquivo do pack sem hash no índice.
    expected: Option<ExpectedHash>,
    /// Origem.
    source: ItemSource,
}

impl Item {
    fn expected_text(&self) -> String {
        self.expected
            .as_ref()
            .map_or_else(String::new, ToString::to_string)
    }

    fn origin(&self) -> EntryOrigin {
        match &self.source {
            ItemSource::Pack { .. } => EntryOrigin::Pack,
            ItemSource::Url { url, modrinth } => EntryOrigin::Url {
                url: url.clone(),
                modrinth: modrinth.clone(),
            },
            ItemSource::Curseforge { project, file } => EntryOrigin::Curseforge {
                project: u64::from(*project),
                file: u64::from(*file),
            },
        }
    }

    fn failed(&self, reason: FailureReason) -> FailedItem {
        FailedItem {
            path: self.index_path.clone(),
            name: self.name.clone(),
            reason,
        }
    }
}

/// O pack lido e filtrado.
#[derive(Debug, Default)]
struct Plan {
    items: Vec<Item>,
    failed: Vec<FailedItem>,
    skipped_side: u32,
    skipped_disabled: u32,
    skipped_player_tools: u32,
}

/// Lê o pack e monta a lista do que deve estar na instância.
fn plan(
    pack_root: &Path,
    side: InstallSide,
    choices: &OptionalChoices,
    player_tools: &PlayerToolsMode,
) -> Result<Plan> {
    let warden_packwiz::PackRead {
        pack,
        index,
        metafiles,
        ..
    } = read_pack(pack_root).map_err(Error::Pack)?;
    let index = index.map_err(Error::Pack)?.value;
    let index_file = clean_path(&pack.value.index.file);
    let index_dir_rel = match index_file.rsplit_once('/') {
        Some((dir, _)) => format!("{dir}/"),
        None => String::new(),
    };
    let index_dir = pack_root.join(&index_dir_rel);
    let index_format = HashFormat::parse(&index.hash_format).ok();
    let metafiles: BTreeMap<&str, &warden_packwiz::MetafileRead> = metafiles
        .iter()
        .map(|read| (read.path.as_str(), read))
        .collect();

    let context = PlanContext {
        index_dir_rel,
        index_dir,
        index_format,
        side,
        choices,
    };
    let mut plan = Plan::default();
    let mut by_dest: BTreeMap<String, Item> = BTreeMap::new();
    // Ferramentas do jogador que este tipo de teste não leva (D16): saem da lista e, se já
    // estavam na instância, são removidas como qualquer item que saiu do pack.
    let left_out = excluded_paths(pack_root, player_tools);
    for entry in index.normalized_entries() {
        if left_out.contains(&clean_path(&format!(
            "{}{}",
            context.index_dir_rel, entry.file
        ))) {
            plan.skipped_player_tools += 1;
            continue;
        }
        match plan_entry(&context, &entry, &metafiles) {
            EntryPlan::Item(item) => {
                by_dest.insert(item.dest.clone(), *item);
            }
            EntryPlan::SkippedSide => plan.skipped_side += 1,
            EntryPlan::SkippedDisabled => plan.skipped_disabled += 1,
            EntryPlan::Failed(failed) => plan.failed.push(failed),
        }
    }
    plan.items = by_dest.into_values().collect();
    Ok(plan)
}

/// O que não muda entre as entradas do índice.
struct PlanContext<'a> {
    /// Pasta do índice relativa ao pack, com `/` no fim (ou vazia).
    index_dir_rel: String,
    /// Pasta do índice no disco.
    index_dir: PathBuf,
    /// Formato dos hashes do índice.
    index_format: Option<HashFormat>,
    side: InstallSide,
    choices: &'a OptionalChoices,
}

/// Destino de uma entrada do índice.
enum EntryPlan {
    Item(Box<Item>),
    SkippedSide,
    SkippedDisabled,
    Failed(FailedItem),
}

fn plan_entry(
    context: &PlanContext<'_>,
    entry: &IndexEntry,
    metafiles: &BTreeMap<&str, &warden_packwiz::MetafileRead>,
) -> EntryPlan {
    let index_dir_rel = &context.index_dir_rel;
    let alias = (!entry.alias.is_empty()).then(|| clean_path(&entry.alias));
    let failed = |name: &str, reason: FailureReason| {
        EntryPlan::Failed(FailedItem {
            path: entry.file.clone(),
            name: name.to_owned(),
            reason,
        })
    };
    let item = if entry.metafile {
        let metafile = match metafiles.get(entry.file.as_str()).map(|read| &read.result) {
            Some(Ok(parsed)) => &parsed.value,
            Some(Err(error)) => {
                return failed(
                    file_name_of(&entry.file),
                    FailureReason::InvalidMetafile {
                        message: error.to_string(),
                    },
                );
            }
            None => {
                return failed(
                    file_name_of(&entry.file),
                    FailureReason::InvalidMetafile {
                        message: "o metafile não foi lido".to_owned(),
                    },
                );
            }
        };
        match context.side.wants(&metafile.side) {
            Some(true) => {}
            Some(false) => return EntryPlan::SkippedSide,
            None => {
                return failed(
                    &metafile.name,
                    FailureReason::UnknownSide {
                        side: metafile.side.as_str().to_owned(),
                    },
                );
            }
        }
        if !context
            .choices
            .is_enabled(&entry.file, metafile.option.as_ref())
        {
            return EntryPlan::SkippedDisabled;
        }
        let mut item = match metafile_item(&entry.file, &entry.hash, metafile, entry.preserve) {
            Ok(item) => item,
            Err(reason) => return failed(&metafile.name, reason),
        };
        item.dest = match (&alias, entry.file.rsplit_once('/')) {
            (Some(alias), _) => clean_path(&format!("{index_dir_rel}{alias}")),
            (None, Some((dir, _))) => {
                clean_path(&format!("{index_dir_rel}{dir}/{}", metafile.filename))
            }
            (None, None) => clean_path(&format!("{index_dir_rel}{}", metafile.filename)),
        };
        item
    } else {
        let format = if entry.hash_format.is_empty() {
            context.index_format
        } else {
            HashFormat::parse(&entry.hash_format).ok()
        };
        let expected = match (format, entry.hash.trim()) {
            (Some(format), hash) if !hash.is_empty() => Some(ExpectedHash::new(format, hash)),
            _ => None,
        };
        let dest = clean_path(&format!(
            "{index_dir_rel}{}",
            alias.as_deref().unwrap_or(&entry.file)
        ));
        Item {
            index_path: entry.file.clone(),
            index_hash: entry.hash.clone(),
            name: file_name_of(&entry.file).to_owned(),
            file_name: file_name_of(&entry.file).to_owned(),
            dest,
            preserve: entry.preserve,
            expected,
            source: ItemSource::Pack {
                file: context.index_dir.join(&entry.file),
            },
        }
    };
    if let Err(error) = warden_packwiz::check_relative_path(&item.dest) {
        return EntryPlan::Failed(item.failed(FailureReason::UnsafePath {
            message: error.to_string(),
        }));
    }
    EntryPlan::Item(Box::new(item))
}

fn metafile_item(
    path: &str,
    index_hash: &str,
    metafile: &Metafile,
    preserve: bool,
) -> std::result::Result<Item, FailureReason> {
    let format = metafile
        .hash_format()
        .map_err(|error| FailureReason::InvalidMetafile {
            message: error.to_string(),
        })?;
    let hash = metafile.download.hash.trim();
    if hash.is_empty() {
        return Err(FailureReason::InvalidMetafile {
            message: "o metafile não tem o hash do download".to_owned(),
        });
    }
    let source = if metafile.is_curseforge_mode() {
        let Some((project, file)) = metafile.curseforge_ids() else {
            return Err(FailureReason::InvalidMetafile {
                message: "metafile da CurseForge sem [update.curseforge]".to_owned(),
            });
        };
        ItemSource::Curseforge { project, file }
    } else {
        let url = metafile.download.url.trim();
        if url.is_empty() {
            return Err(FailureReason::InvalidMetafile {
                message: "o metafile não tem o endereço do download".to_owned(),
            });
        }
        ItemSource::Url {
            url: url.to_owned(),
            modrinth: metafile
                .modrinth_ids()
                .map(|(project, version)| ModrinthIds {
                    project: project.to_owned(),
                    version: version.to_owned(),
                }),
        }
    };
    Ok(Item {
        index_path: path.to_owned(),
        index_hash: index_hash.to_owned(),
        name: if metafile.name.is_empty() {
            metafile.filename.clone()
        } else {
            metafile.name.clone()
        },
        file_name: file_name_of(&metafile.filename).to_owned(),
        dest: String::new(),
        preserve,
        expected: Some(ExpectedHash::new(format, hash)),
        source,
    })
}

fn file_name_of(path: &str) -> &str {
    path.rsplit_once('/').map_or(path, |(_, name)| name)
}

/// Situação de um arquivo do disco em relação ao manifesto.
#[derive(Debug, Clone)]
struct DiskState {
    exists: bool,
    /// Hashes, quando foi preciso ler o arquivo.
    sha256: Option<String>,
    /// Confere com o esperado do item.
    matches_expected: bool,
}

/// O que fazer com cada item.
#[derive(Debug, Clone)]
enum Action {
    /// Já está certo; a entrada do manifesto (nova ou a mesma).
    Keep(Option<ManifestEntry>),
    /// `preserve` com o destino existente.
    Preserve,
    /// Precisa gravar.
    Write,
}

fn manifest_entry_for(item: &Item, sha256: String, metadata: &fs::Metadata) -> ManifestEntry {
    ManifestEntry {
        sha256,
        size: metadata.len(),
        modified_ns: modified_ns(metadata),
        index_path: item.index_path.clone(),
        index_hash: item.index_hash.clone(),
        expected: item.expected_text(),
        origin: item.origin(),
    }
}

/// Se o arquivo no disco ainda é o que o manifesto registrou, sem ler o conteúdo.
fn unchanged_since_recorded(entry: &ManifestEntry, metadata: &fs::Metadata) -> bool {
    metadata.is_file()
        && metadata.len() == entry.size
        && entry.modified_ns != 0
        && modified_ns(metadata) == entry.modified_ns
}

fn read_metadata(path: &Path) -> Result<Option<fs::Metadata>> {
    match fs::metadata(path) {
        Ok(metadata) => Ok(Some(metadata)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(Error::io("ler", path, error)),
    }
}

/// Lê o destino e compara com o esperado.
fn disk_state(path: &Path, expected: Option<&ExpectedHash>) -> Result<DiskState> {
    let Some(metadata) = read_metadata(path)? else {
        return Ok(DiskState {
            exists: false,
            sha256: None,
            matches_expected: false,
        });
    };
    if !metadata.is_file() {
        return Ok(DiskState {
            exists: true,
            sha256: None,
            matches_expected: false,
        });
    }
    let (_, hashes) = hash_file(path).map_err(|e| Error::io("ler", path, e))?;
    let matches_expected = expected.is_some_and(|expected| {
        hash_matches(
            expected.format,
            &expected.value,
            &hashes.get(expected.format).unwrap_or_default(),
        )
    });
    Ok(DiskState {
        exists: true,
        sha256: Some(hashes.sha256),
        matches_expected,
    })
}

/// Materializa o pack de `pack_root` na instância `dirs`. Veja o módulo.
pub async fn materialize(
    sources: &Sources,
    pack_root: &Path,
    dirs: &InstanceDirs,
    options: &MaterializeOptions,
    progress: &dyn ProgressSink,
    cancel: &CancellationToken,
) -> Result<Outcome> {
    progress.stage(STAGE, STAGE_LABEL);
    let choices = match &options.optionals {
        OptionalSelection::Saved => OptionalChoices::load(&dirs.state_dir)?,
        OptionalSelection::Explicit(choices) => choices.clone(),
    };
    let side = options.side;
    let root = pack_root.to_path_buf();
    let player_tools = options.player_tools.clone();
    let plan = blocking(move || plan(&root, side, &choices, &player_tools)).await?;
    check_cancel(cancel)?;

    fs::create_dir_all(&dirs.game_dir)
        .map_err(|e| Error::io("criar a pasta", &dirs.game_dir, e))?;
    let old = Manifest::load(&dirs.state_dir)?;

    // 1. Situação de cada item e do que saiu do pack, sem mexer em nada.
    let game_dir = dirs.game_dir.clone();
    let items = plan.items.clone();
    let old_for_check = old.clone();
    let checked = blocking(move || check_disk(&game_dir, &items, &old_for_check)).await?;
    check_cancel(cancel)?;
    let Checked {
        actions,
        removals,
        modified,
    } = checked;
    if options.on_modified == OnModified::Pause && !modified.is_empty() {
        return Ok(Outcome::NeedsReview(modified));
    }

    let report = Report {
        skipped_side: plan.skipped_side,
        skipped_disabled: plan.skipped_disabled,
        skipped_player_tools: plan.skipped_player_tools,
        ..Report::default()
    };
    let failed = plan.failed.clone();
    let mut manifest = old;
    side.as_str().clone_into(&mut manifest.side);
    if let Err(error) = sources.cache.clean_stale_parts(STALE_PART_AGE) {
        tracing::warn!(%error, "não foi possível limpar downloads abandonados");
    }

    let mut run = Run {
        sources,
        dirs,
        items: &plan.items,
        progress,
        cancel,
        manifest,
        report,
        failed,
    };
    let result = run.execute(options, &actions, &removals).await;
    let Run {
        manifest,
        mut report,
        failed,
        ..
    } = run;
    // O manifesto reflete o que foi feito, mesmo cancelado ou com falhas.
    report.manifest_written = manifest.save_if_changed(&dirs.state_dir)?;
    result?;
    report.failed = failed;
    Ok(Outcome::Done(report))
}

/// Resultado da conferência do disco.
#[derive(Debug)]
struct Checked {
    /// Ação por item, na ordem de `plan.items`.
    actions: Vec<Action>,
    /// Entradas do manifesto antigo a remover (caminho), com o arquivo ainda no disco ou não.
    removals: Vec<String>,
    /// Arquivos alterados que seriam sobrescritos ou apagados.
    modified: Vec<ModifiedFile>,
}

fn check_disk(game_dir: &Path, items: &[Item], old: &Manifest) -> Result<Checked> {
    let mut actions = Vec::with_capacity(items.len());
    let mut modified = Vec::new();
    for item in items {
        let path = resolve_inside(game_dir, &item.dest)?;
        let recorded = old.files.get(&item.dest);
        let metadata = read_metadata(&path)?;
        // Atalho: item igual no índice e arquivo igual ao registrado.
        if let (Some(entry), Some(metadata)) = (recorded, &metadata)
            && entry.index_path == item.index_path
            && entry.index_hash == item.index_hash
            && entry.expected == item.expected_text()
            && unchanged_since_recorded(entry, metadata)
        {
            actions.push(Action::Keep(Some(entry.clone())));
            continue;
        }
        if item.preserve && metadata.is_some() {
            actions.push(Action::Preserve);
            continue;
        }
        let state = disk_state(&path, item.expected.as_ref())?;
        if state.matches_expected
            && let (Some(sha256), Some(metadata)) = (state.sha256.clone(), &metadata)
        {
            actions.push(Action::Keep(Some(manifest_entry_for(
                item, sha256, metadata,
            ))));
            continue;
        }
        if state.exists {
            let changed = match (recorded, &state.sha256) {
                (Some(entry), Some(sha256)) => !entry.sha256.eq_ignore_ascii_case(sha256),
                _ => true,
            };
            if changed {
                modified.push(ModifiedFile {
                    path: item.dest.clone(),
                    action: ModifiedAction::Overwrite,
                    managed: recorded.is_some(),
                });
            }
        }
        actions.push(Action::Write);
    }
    let wanted: BTreeSet<&str> = items.iter().map(|item| item.dest.as_str()).collect();
    let mut removals = Vec::new();
    for (dest, entry) in &old.files {
        if wanted.contains(dest.as_str()) {
            continue;
        }
        removals.push(dest.clone());
        let Ok(path) = resolve_inside(game_dir, dest) else {
            continue;
        };
        let Some(metadata) = read_metadata(&path)? else {
            continue;
        };
        let changed = if unchanged_since_recorded(entry, &metadata) {
            false
        } else if metadata.is_file() {
            let (_, hashes) = hash_file(&path).map_err(|e| Error::io("ler", &path, e))?;
            !entry.sha256.eq_ignore_ascii_case(&hashes.sha256)
        } else {
            true
        };
        if changed {
            modified.push(ModifiedFile {
                path: dest.clone(),
                action: ModifiedAction::Remove,
                managed: true,
            });
        }
    }
    Ok(Checked {
        actions,
        removals,
        modified,
    })
}

/// Um item que precisa de download.
#[derive(Debug, Clone)]
struct PendingDownload {
    item: usize,
    request: DownloadRequest,
    origin: FileOrigin,
    curseforge: bool,
}

/// Uma materialização em andamento: o que já foi feito fica em `manifest`, `report` e
/// `failed`, para o manifesto ser gravado mesmo se a operação parar no meio.
struct Run<'a> {
    sources: &'a Sources,
    dirs: &'a InstanceDirs,
    items: &'a [Item],
    progress: &'a dyn ProgressSink,
    cancel: &'a CancellationToken,
    manifest: Manifest,
    report: Report,
    failed: Vec<FailedItem>,
}

/// Itens prontos para copiar (índice do item → arquivo de origem).
type Ready = BTreeMap<usize, PathBuf>;

impl Run<'_> {
    async fn execute(
        &mut self,
        options: &MaterializeOptions,
        actions: &[Action],
        removals: &[String],
    ) -> Result<()> {
        // 2. Restos de uma cópia interrompida por queda do app, e remoções (como o
        //    packwiz-installer, antes dos downloads).
        self.remove_temp_leftovers();
        self.remove(removals)?;
        // 3. Itens certos e preservados; o resto precisa ser gravado.
        let to_write = self.sort(actions);
        self.progress
            .progress(Progress::items(0, Some(to_write.len() as u64)));
        // 4. Origem de cada item: o pack, o cache ou um download.
        let (mut ready, mut downloads, curseforge) = self.sources_of(&to_write)?;
        check_cancel(self.cancel)?;
        if !curseforge.is_empty() {
            resolve_curseforge(
                self.sources,
                self.items,
                &curseforge,
                &mut downloads,
                &mut self.report,
                &mut self.failed,
                self.cancel,
            )
            .await?;
        }
        // 5. Downloads em paralelo, para o cache.
        let downloaded = run_downloads(
            self.sources,
            options,
            self.items,
            downloads,
            &mut self.failed,
            self.cancel,
        )
        .await?;
        let from_network = downloaded
            .iter()
            .filter(|(_, fetched)| fetched.from_network)
            .count();
        self.report.downloaded = u32::try_from(from_network).unwrap_or(u32::MAX);
        ready.extend(
            downloaded
                .into_iter()
                .map(|(index, fetched)| (index, fetched.file.path)),
        );
        // 6. Cópias para a instância, na ordem dos itens.
        self.copy(&to_write, &ready).await
    }

    fn remove_temp_leftovers(&self) {
        for item in self.items {
            let Ok(path) = resolve_inside(&self.dirs.game_dir, &item.dest) else {
                continue;
            };
            let mut temp = path.into_os_string();
            temp.push(TEMP_SUFFIX);
            if let Err(error) = remove_if_exists(Path::new(&temp)) {
                tracing::warn!(%error, "não foi possível apagar um temporário da instância");
            }
        }
    }

    fn remove(&mut self, removals: &[String]) -> Result<()> {
        for dest in removals {
            check_cancel(self.cancel)?;
            match resolve_inside(&self.dirs.game_dir, dest) {
                Ok(path) => match remove_if_exists(&path) {
                    Ok(()) => {
                        if self.manifest.files.remove(dest).is_some() {
                            self.report.removed += 1;
                        }
                    }
                    Err(error) => {
                        tracing::warn!(%error, caminho = dest, "não foi possível apagar da instância");
                    }
                },
                Err(_) => {
                    self.manifest.files.remove(dest);
                }
            }
        }
        Ok(())
    }

    fn sort(&mut self, actions: &[Action]) -> Vec<usize> {
        let mut to_write = Vec::new();
        for (index, (item, action)) in self.items.iter().zip(actions).enumerate() {
            match action {
                Action::Keep(entry) => {
                    self.report.unchanged += 1;
                    if let Some(entry) = entry {
                        self.manifest.files.insert(item.dest.clone(), entry.clone());
                    }
                }
                Action::Preserve => self.report.preserved += 1,
                Action::Write => to_write.push(index),
            }
        }
        to_write
    }

    /// Separa os itens a gravar: prontos (pack ou cache), downloads por link e itens da
    /// CurseForge que precisam do endereço.
    fn sources_of(
        &mut self,
        to_write: &[usize],
    ) -> Result<(Ready, Vec<PendingDownload>, Vec<usize>)> {
        let mut ready = Ready::new();
        let mut downloads = Vec::new();
        let mut curseforge = Vec::new();
        for &index in to_write {
            let item = &self.items[index];
            if let ItemSource::Pack { file } = &item.source {
                ready.insert(index, file.clone());
                continue;
            }
            let Some(expected) = item.expected.clone() else {
                continue;
            };
            if let Some(cached) = self.sources.cache.find(&expected)? {
                ready.insert(index, cached.path);
                continue;
            }
            let ItemSource::Url { url, modrinth } = &item.source else {
                curseforge.push(index);
                continue;
            };
            let parsed = match Url::parse(url) {
                Ok(parsed) => parsed,
                Err(error) => {
                    self.failed
                        .push(item.failed(FailureReason::InvalidMetafile {
                            message: format!("endereço inválido: {error}"),
                        }));
                    continue;
                }
            };
            let mut request = DownloadRequest::new(parsed.clone(), PathBuf::new());
            request.expected_hashes.push(expected);
            if let Some(client) = &self.sources.curseforge {
                request.headers = client.download_headers(&parsed).unwrap_or_default();
            }
            let origin = match modrinth {
                Some(ids) => FileOrigin::modrinth(&ids.project, &ids.version, url),
                None => FileOrigin::url(url),
            };
            downloads.push(PendingDownload {
                item: index,
                request,
                origin,
                curseforge: false,
            });
        }
        Ok((ready, downloads, curseforge))
    }

    async fn copy(&mut self, to_write: &[usize], ready: &Ready) -> Result<()> {
        let total = to_write.len() as u64;
        let mut done = 0_u64;
        for &index in to_write {
            check_cancel(self.cancel)?;
            let item = &self.items[index];
            let Some(source) = ready.get(&index).cloned() else {
                continue;
            };
            let game_dir = self.dirs.game_dir.clone();
            let item_for_copy = item.clone();
            let token = self.cancel.clone();
            let result =
                blocking(move || install_file(&game_dir, &item_for_copy, &source, &token)).await;
            match result {
                Ok(installed) => {
                    if installed.stale_index {
                        self.report.stale_index.push(item.index_path.clone());
                    }
                    self.manifest
                        .files
                        .insert(item.dest.clone(), installed.entry);
                    self.report.written += 1;
                }
                Err(Error::Cancelled) => return Err(Error::Cancelled),
                Err(error) => self.failed.push(item.failed(io_reason(&error))),
            }
            done += 1;
            self.progress.progress(Progress::items(done, Some(total)));
        }
        Ok(())
    }
}

/// Resolve os itens da CurseForge que não estão no cache: em lote, sem gravar o endereço.
async fn resolve_curseforge(
    sources: &Sources,
    items: &[Item],
    wanted: &[usize],
    downloads: &mut Vec<PendingDownload>,
    report: &mut Report,
    failed: &mut Vec<FailedItem>,
    cancel: &CancellationToken,
) -> Result<()> {
    let ids = |index: usize| match items[index].source {
        ItemSource::Curseforge { project, file } => (u64::from(project), u64::from(file)),
        _ => (0, 0),
    };
    let Some(client) = sources
        .curseforge
        .as_ref()
        .filter(|client| client.has_key())
    else {
        for &index in wanted {
            failed.push(items[index].failed(FailureReason::CurseforgeKeyMissing));
        }
        return Ok(());
    };
    let refs: Vec<FileRef> = wanted
        .iter()
        .map(|&index| {
            let (mod_id, file_id) = ids(index);
            FileRef { mod_id, file_id }
        })
        .collect();
    let distributions = match client.distribution(&refs, Some(cancel)).await {
        Ok(distributions) => distributions,
        Err(warden_curseforge::Error::Http(warden_http::Error::Cancelled)) => {
            return Err(Error::Cancelled);
        }
        Err(error) => {
            let reason = curseforge_reason(&error);
            for &index in wanted {
                failed.push(items[index].failed(reason.clone()));
            }
            return Ok(());
        }
    };
    for (&index, distribution) in wanted.iter().zip(distributions) {
        let item = &items[index];
        let (project, file) = ids(index);
        match distribution.distribution {
            Distribution::Allowed { url } => {
                let Ok(parsed) = Url::parse(&url) else {
                    failed.push(item.failed(FailureReason::CurseforgeUnavailable {
                        message: "a CurseForge devolveu um endereço inválido".to_owned(),
                        retryable: false,
                    }));
                    continue;
                };
                let Some(expected) = item.expected.clone() else {
                    continue;
                };
                let mut request = DownloadRequest::new(parsed.clone(), PathBuf::new());
                request.expected_hashes.push(expected);
                request.headers = client.download_headers(&parsed).unwrap_or_default();
                downloads.push(PendingDownload {
                    item: index,
                    request,
                    origin: FileOrigin::curseforge(project, file),
                    curseforge: true,
                });
            }
            Distribution::Blocked { page_url } => {
                let expected = item.expected.clone();
                report.blocked.push(BlockedFile {
                    metafile: item.index_path.clone(),
                    name: item.name.clone(),
                    file_name: item.file_name.clone(),
                    project_id: project,
                    file_id: file,
                    page_url,
                    hash_format: expected
                        .as_ref()
                        .map(|hash| hash.format.to_string())
                        .unwrap_or_default(),
                    hash: expected.map(|hash| hash.value).unwrap_or_default(),
                });
            }
            Distribution::Missing => {
                failed.push(item.failed(FailureReason::CurseforgeFileMissing));
            }
        }
    }
    Ok(())
}

fn curseforge_reason(error: &warden_curseforge::Error) -> FailureReason {
    match error {
        warden_curseforge::Error::KeyMissing => FailureReason::CurseforgeKeyMissing,
        warden_curseforge::Error::KeyInvalid { .. } => FailureReason::CurseforgeKeyInvalid,
        warden_curseforge::Error::FileNotFound { .. }
        | warden_curseforge::Error::ModNotFound { .. } => FailureReason::CurseforgeFileMissing,
        other => FailureReason::CurseforgeUnavailable {
            message: other.to_string(),
            retryable: other.retryable(),
        },
    }
}

/// Roda os downloads com no máximo `max_parallel_downloads` ao mesmo tempo. Devolve os
/// arquivos baixados por item; as falhas vão para `failed`.
async fn run_downloads(
    sources: &Sources,
    options: &MaterializeOptions,
    items: &[Item],
    downloads: Vec<PendingDownload>,
    failed: &mut Vec<FailedItem>,
    cancel: &CancellationToken,
) -> Result<Vec<(usize, Fetched)>> {
    let semaphore = Arc::new(Semaphore::new(options.max_parallel_downloads.max(1)));
    let mut tasks = JoinSet::new();
    for pending in downloads {
        let semaphore = Arc::clone(&semaphore);
        let http = sources.http.clone();
        let cache = Arc::clone(&sources.cache);
        let token = cancel.clone();
        tasks.spawn(async move {
            let _permit = semaphore.acquire_owned().await;
            let url = pending.request.url.to_string();
            let result = cache
                .download(
                    &http,
                    pending.request,
                    &pending.origin,
                    &NoProgress,
                    Some(&token),
                )
                .await;
            (pending.item, pending.curseforge, url, result)
        });
    }
    let mut done = Vec::new();
    let mut cancelled = false;
    while let Some(joined) = tasks.join_next().await {
        let (index, curseforge, url, result) = match joined {
            Ok(value) => value,
            Err(error) => return Err(Error::Internal(format!("tarefa de download: {error}"))),
        };
        match result {
            Ok(fetched) => done.push((index, fetched)),
            Err(Error::Http(warden_http::Error::Cancelled) | Error::Cancelled) => cancelled = true,
            Err(error) => {
                let mut message = error.to_string();
                if curseforge {
                    // O endereço da CurseForge não vai para mensagens (termos da API).
                    message = message.replace(&url, "[endereço da CurseForge]");
                }
                failed.push(items[index].failed(FailureReason::DownloadFailed {
                    message,
                    retryable: error.retryable(),
                }));
            }
        }
        if cancel.is_cancelled() {
            cancelled = true;
            tasks.abort_all();
        }
    }
    if cancelled {
        return Err(Error::Cancelled);
    }
    done.sort_by_key(|(index, _)| *index);
    Ok(done)
}

fn io_reason(error: &Error) -> FailureReason {
    match error {
        Error::Core(warden_core::CoreError::PathOutsideRoot { .. }) => FailureReason::UnsafePath {
            message: error.to_string(),
        },
        Error::Internal(message) => FailureReason::DownloadFailed {
            message: message.clone(),
            retryable: false,
        },
        other => FailureReason::Io {
            message: warden_core::error_chain(other),
        },
    }
}

/// Arquivo gravado na instância.
struct Installed {
    entry: ManifestEntry,
    stale_index: bool,
}

/// Copia `source` para o destino do item, atomicamente, conferindo o hash.
fn install_file(
    game_dir: &Path,
    item: &Item,
    source: &Path,
    cancel: &CancellationToken,
) -> Result<Installed> {
    let destination = resolve_inside(game_dir, &item.dest)?;
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(|e| Error::io("criar a pasta", parent, e))?;
    }
    let mut temp = destination.clone().into_os_string();
    temp.push(TEMP_SUFFIX);
    let temp = PathBuf::from(temp);
    let (_, hashes) = copy_hashing(source, &temp, Some(cancel))?;
    let matches = item.expected.as_ref().is_none_or(|expected| {
        hash_matches(
            expected.format,
            &expected.value,
            &hashes.get(expected.format).unwrap_or_default(),
        )
    });
    let stale_index = match (&item.source, matches) {
        (_, true) => false,
        (ItemSource::Pack { .. }, false) => true,
        (_, false) => {
            remove_if_exists(&temp)?;
            let expected = item
                .expected
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_default();
            return Err(Error::Internal(format!(
                "o arquivo do cache não confere com o hash esperado ({expected})"
            )));
        }
    };
    if let Err(error) = rename_with_retry(&temp, &destination) {
        let _ = remove_if_exists(&temp);
        return Err(Error::io("renomear para", destination, error));
    }
    let metadata = fs::metadata(&destination).map_err(|e| Error::io("ler", &destination, e))?;
    Ok(Installed {
        entry: manifest_entry_for(item, hashes.sha256, &metadata),
        stale_index,
    })
}

fn check_cancel(cancel: &CancellationToken) -> Result<()> {
    if cancel.is_cancelled() {
        Err(Error::Cancelled)
    } else {
        Ok(())
    }
}

/// Roda trabalho de disco fora do executor assíncrono.
async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T> + Send + 'static,
) -> Result<T> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|error| Error::Internal(format!("tarefa de disco: {error}")))?
}

#[cfg(test)]
mod tests {
    use std::fmt::Write as _;

    use super::*;

    fn pack(versions: &[(&str, &str)]) -> PackManifest {
        let mut text = String::from(
            "name = \"T\"\npack-format = \"packwiz:1.1.0\"\n\n[index]\nfile = \"index.toml\"\nhash-format = \"sha256\"\nhash = \"\"\n\n[versions]\n",
        );
        for (key, value) in versions {
            let _ = writeln!(text, "{key} = \"{value}\"");
        }
        PackManifest::parse(&text).unwrap().value
    }

    #[test]
    fn requisito_com_versao_exata_do_loader() {
        let requirement =
            game_requirement(&pack(&[("minecraft", "1.20.1"), ("fabric", "0.16.5")])).unwrap();
        assert_eq!(requirement.minecraft, "1.20.1");
        assert_eq!(
            requirement.loader,
            Some(LoaderRequirement {
                loader: "fabric".to_owned(),
                version: "0.16.5".to_owned()
            })
        );
        let forge =
            game_requirement(&pack(&[("minecraft", "1.7.10"), ("forge", "10.13.4.1614")])).unwrap();
        assert_eq!(forge.loader.unwrap().version, "10.13.4.1614");
        let vanilla = game_requirement(&pack(&[("minecraft", "26.1")])).unwrap();
        assert_eq!(vanilla.loader, None);
    }

    #[test]
    fn requisito_recusa_versao_nao_exata_e_dois_loaders() {
        for version in [
            "",
            " ",
            "latest",
            "Recommended",
            "[47,)",
            "47.*",
            "stable",
            "x",
        ] {
            let error = game_requirement(&pack(&[("minecraft", "1.20.1"), ("forge", version)]))
                .unwrap_err();
            assert!(
                matches!(error, Error::LoaderVersionNotExact { .. }),
                "{version:?}: {error:?}"
            );
        }
        let error = game_requirement(&pack(&[
            ("minecraft", "1.20.1"),
            ("forge", "47.3.0"),
            ("fabric", "0.16.5"),
        ]))
        .unwrap_err();
        assert!(
            matches!(error, Error::LoaderAmbiguous { loaders } if loaders == ["fabric", "forge"])
        );
        let error = game_requirement(&pack(&[("fabric", "0.16.5")])).unwrap_err();
        assert!(matches!(error, Error::MinecraftVersionMissing));
    }

    #[test]
    fn lado_do_item() {
        assert_eq!(InstallSide::Client.wants(&Side::Unset), Some(true));
        assert_eq!(InstallSide::Client.wants(&Side::Both), Some(true));
        assert_eq!(InstallSide::Client.wants(&Side::Client), Some(true));
        assert_eq!(InstallSide::Client.wants(&Side::Server), Some(false));
        assert_eq!(InstallSide::Server.wants(&Side::Client), Some(false));
        assert_eq!(InstallSide::Server.wants(&Side::Server), Some(true));
        assert_eq!(InstallSide::Client.wants(&Side::Other("x".into())), None);
    }
}
