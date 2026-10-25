//! "O que mudou durante o teste": a captura depois que o jogo fecha (ARCHITECTURE §8.3 itens 2
//! a 6; SPEC T15; C-03).
//!
//! 1. [`capture_after_exit`] varre a instância de novo, filtra por tamanho e data e confirma
//!    pelo SHA-256: cada arquivo é novo, alterado ou removido em relação à
//!    [linha de base](crate::baseline).
//! 2. Arquivos de formato conhecido (`warden-configs`) são comparados pelos valores: se só
//!    mudou formatação, ordem ou comentário, o arquivo é "reformatado" e não aparece
//!    (CA-T15-02).
//! 3. Cada mudança cai num grupo ([`CaptureGroup`]): configs do pack, `options.txt` por chave
//!    (sem as voláteis, [`VOLATILE_OPTIONS`]; só quando não é pessoal, ADR-0067), arquivos criados pelos mods, configs de mundo,
//!    mods colocados à mão, resource packs e shaders, datapacks.
//! 4. Uma config do pack que o jogo só reformatou (ou um `options.txt` do pack em que só
//!    mudaram chaves voláteis) volta à versão do pack na hora: a próxima cópia do pack não
//!    para pedindo revisão por causa disso.
//! 5. O resultado fica em `state/baseline/capture.json` até a pessoa decidir:
//!    [`plan_apply`] monta o que gravar no pack (só o que foi escolhido; `options.txt` chave a
//!    chave), [`discard`] devolve à versão do pack as configs do pack que mudaram e
//!    [`finish_apply`] marca a captura como resolvida.
//!
//! Nada é gravado no pack por esta crate: a gravação é da `PackTransaction` da
//! `warden-project`, no comando do app.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};
use warden_configs::{ConfigFormat, SemanticChange, compare};
use warden_core::{atomic_write, resolve_inside};

use crate::baseline::{
    BASELINE_DIR, Baseline, BlobStore, CaptureIgnore, FileState, MAX_TEXT_BYTES, PackLayout,
    hash_file, instance_path, keeps_text, now_ms, pack_layout, read_pack_file, scan, sha256_hex,
    state_path, world_config_rest,
};
use crate::error::Error;
use crate::manifest::{EntryOrigin, Manifest};
use crate::materialize::InstanceDirs;
use crate::three_way;

/// Arquivo da captura dentro de `state/baseline/`.
pub const CAPTURE_FILE: &str = "capture.json";

/// Versão do formato da captura.
pub(crate) const CAPTURE_SCHEMA: u32 = 1;

/// Chaves do `options.txt` que o jogo muda sozinho e nunca aparecem (ARCHITECTURE §8.3 item 5).
pub const VOLATILE_OPTIONS: [&str; 12] = [
    "version",
    "lastServer",
    "fullscreen",
    "fullscreenResolution",
    "overrideWidth",
    "overrideHeight",
    "tutorialStep",
    "joinedFirstServer",
    "onboardAccessibility",
    "skipMultiplayerWarning",
    "incompatibleResourcePacks",
    "soundDevice",
];

/// O `options.txt` da raiz.
pub(crate) const OPTIONS_FILE: &str = "options.txt";

/// Grupo de uma mudança (SPEC T15, "Grupos").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum CaptureGroup {
    /// Config (ou script) que veio do pack e mudou no jogo. Marcado.
    PackConfig,
    /// O `options.txt`, chave a chave.
    Options,
    /// Config de mundo (`saves/<mundo>/serverconfig/`), com destino em `defaultconfigs/`.
    WorldConfig,
    /// Arquivo criado (ou mudado) pelos mods que não está no pack. Desmarcado.
    Generated,
    /// Mod colocado ou removido à mão em `mods/`: nunca copiado.
    Mod,
    /// Resource pack ou shader novo: nunca copiado.
    Resource,
    /// Datapack num mundo: só informativo.
    Datapack,
}

/// O que aconteceu com o arquivo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum CaptureChange {
    /// Não existia no início do teste.
    Added,
    /// Mudou.
    Modified,
    /// Sumiu.
    Removed,
}

/// Uma chave que mudou de valor (para mostrar "chave: antes → depois").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CaptureValue {
    /// A chave, como aparece no arquivo (`general.maxStack`).
    pub key: String,
    /// Valor antes (`None`: a chave não existia).
    pub before: Option<String>,
    /// Valor depois (`None`: a chave sumiu).
    pub after: Option<String>,
}

/// Uma chave do `options.txt` que mudou.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CaptureOption {
    /// A chave (`renderDistance`, `key_key.jump`).
    pub key: String,
    /// Valor no início do teste, como está no arquivo (`None`: a chave não existia).
    pub before: Option<String>,
    /// Valor agora.
    pub after: String,
    /// Pré-seleção: marcada se a chave já existia.
    pub selected: bool,
}

/// Um arquivo que mudou durante o teste.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CapturedFile {
    /// Caminho na instância (relativo à pasta do jogo, com `/`). Para datapacks e resource
    /// packs em pasta, a pasta (`saves/Mundo/datapacks/meu`).
    pub path: String,
    /// Grupo.
    pub group: CaptureGroup,
    /// O que aconteceu.
    pub change: CaptureChange,
    /// Pré-seleção (SPEC T15).
    pub selected: bool,
    /// Para onde vai no pack, se for trazido (`None`: nunca é copiado).
    pub pack_path: Option<String>,
    /// Diferença por valor (formatos conhecidos).
    pub values: Vec<CaptureValue>,
    /// Chaves do `options.txt` (só no grupo `options`).
    pub options: Vec<CaptureOption>,
    /// Quantas chaves voláteis do `options.txt` mudaram e ficam escondidas.
    pub hidden_volatile: u32,
    /// Se há texto dos dois lados para "Ver diferença de texto".
    pub text: bool,
    /// SHA-256 no início do teste.
    pub before_sha256: Option<String>,
    /// SHA-256 agora.
    pub after_sha256: Option<String>,
    /// Tamanho agora (0 se sumiu).
    #[specta(type = specta_typescript::Number)]
    pub size: u64,
    /// Se o arquivo veio do pack (está no manifesto da instância).
    pub managed: bool,
}

/// Situação da captura.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum CaptureStatus {
    /// Esperando a pessoa decidir.
    Pending,
    /// "Trazer selecionados para o pack" feito.
    Applied {
        /// Quantas mudanças foram para o pack.
        count: u32,
    },
    /// "Descartar" feito.
    Discarded,
}

/// A captura de um teste (`state/baseline/capture.json`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Capture {
    /// Versão do formato.
    pub schema_version: u32,
    /// A sessão do teste.
    pub session_id: String,
    /// Quando o jogo fechou (milissegundos desde 1970).
    pub finished_at_ms: u64,
    /// As mudanças, em ordem de grupo e caminho.
    pub files: Vec<CapturedFile>,
    /// Quantos arquivos o jogo só reformatou (não aparecem).
    pub reformatted: u32,
    /// Situação.
    pub status: CaptureStatus,
}

impl Capture {
    /// Se há mudanças que a próxima cópia do pack desfaria: configs do pack e `options.txt`
    /// esperando decisão. É o que faz o Testar perguntar antes e o cabeçalho avisar.
    #[must_use]
    pub fn needs_review(&self) -> bool {
        self.status == CaptureStatus::Pending
            && self
                .files
                .iter()
                .any(|file| matches!(file.group, CaptureGroup::PackConfig | CaptureGroup::Options))
    }

    /// O arquivo pelo caminho na instância.
    #[must_use]
    pub fn file(&self, path: &str) -> Option<&CapturedFile> {
        self.files.iter().find(|file| file.path == path)
    }
}

/// Erro da captura.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum CaptureError {
    /// Não há captura (ou é de outro teste).
    #[error("não há mudanças deste teste para revisar")]
    NotFound,
    /// A captura já foi resolvida.
    #[error("as mudanças deste teste já foram revisadas")]
    AlreadyResolved,
    /// O arquivo mudou (no pack ou na instância) depois que a lista foi mostrada.
    #[error("o arquivo {path} mudou depois que a lista foi mostrada")]
    Stale {
        /// Caminho na instância (ou no pack) do arquivo.
        path: String,
    },
    /// O pedido cita um item que não está na captura ou não pode ser trazido.
    #[error("o item {path} não pode ser trazido para o pack")]
    InvalidItem {
        /// Caminho do item.
        path: String,
    },
    /// "Juntar as duas" não é possível para esse arquivo.
    #[error("as mudanças de {path} não podem ser juntadas automaticamente")]
    NotMergeable {
        /// Caminho do item.
        path: String,
    },
    /// Disco ou outro erro da instância.
    #[error(transparent)]
    Instance(#[from] Error),
}

impl From<warden_core::CoreError> for CaptureError {
    fn from(error: warden_core::CoreError) -> Self {
        Self::Instance(Error::Core(error))
    }
}

/// Lê a captura (`None` se não existe ou é de outra versão).
pub fn load_capture(state_dir: &Path) -> Result<Option<Capture>, Error> {
    let path = state_dir.join(BASELINE_DIR).join(CAPTURE_FILE);
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(Error::io("ler", state_path(CAPTURE_FILE), error)),
    };
    match serde_json::from_slice::<Capture>(&bytes) {
        Ok(capture) if capture.schema_version == CAPTURE_SCHEMA => Ok(Some(capture)),
        Ok(_) => Ok(None),
        Err(error) => {
            tracing::warn!(%error, "captura do teste ilegível; ignorada");
            Ok(None)
        }
    }
}

fn save_capture(state_dir: &Path, capture: &Capture) -> Result<(), Error> {
    let dir = state_dir.join(BASELINE_DIR);
    fs::create_dir_all(&dir).map_err(|e| Error::io("criar a pasta", state_path(""), e))?;
    let mut bytes = serde_json::to_vec_pretty(capture)
        .map_err(|error| Error::Internal(format!("captura em JSON: {error}")))?;
    bytes.push(b'\n');
    atomic_write(&dir.join(CAPTURE_FILE), &bytes)?;
    Ok(())
}

/// Apaga a captura (uma linha de base nova a substitui).
pub(crate) fn forget_capture(state_dir: &Path) -> Result<(), Error> {
    match fs::remove_file(state_dir.join(BASELINE_DIR).join(CAPTURE_FILE)) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(Error::io("apagar", state_path(CAPTURE_FILE), error)),
    }
}

/// O resultado de [`capture_after_exit`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureOutcome {
    /// A captura gravada.
    pub capture: Capture,
    /// Configs do pack que o jogo só reformatou e voltaram à versão do pack.
    pub restored: Vec<String>,
}

/// Um arquivo como está agora.
struct Now {
    size: u64,
    sha256: String,
    /// Conteúdo guardado no depósito.
    stored: bool,
}

/// O que mudou: caminho → estado no início (`None`: novo) e agora (`None`: sumiu).
type Changed<'a> = BTreeMap<String, (Option<&'a FileState>, Option<Now>)>;

/// Os arquivos que mudaram desde a linha de base: tamanho e data diferentes, confirmados pelo
/// SHA-256; mais os que sumiram.
fn changed_files<'a>(
    dirs: &InstanceDirs,
    baseline: &'a Baseline,
    blobs: &BlobStore,
) -> Result<Changed<'a>, Error> {
    let seen = scan(&dirs.game_dir, CaptureIgnore::builtin())?;
    let mut changed = Changed::new();
    for (path, now) in &seen {
        let before = baseline.files.get(path);
        if let Some(before) = before
            && before.size == now.size
            && before.modified_ns == now.modified_ns
        {
            continue;
        }
        let Some(current) = read_now(dirs, blobs, path, now.size)? else {
            continue;
        };
        if before.is_some_and(|before| before.sha256 == current.sha256) {
            continue;
        }
        changed.insert(path.clone(), (before, Some(current)));
    }
    for (path, before) in &baseline.files {
        if !seen.contains_key(path) {
            changed.insert(path.clone(), (Some(before), None));
        }
    }
    Ok(changed)
}

/// Monta a lista da captura, arquivo por arquivo.
struct Collector<'a> {
    baseline: &'a Baseline,
    manifest: Manifest,
    layout: PackLayout,
    blobs: BlobStore,
    files: Vec<CapturedFile>,
    restored: Vec<String>,
    reformatted: u32,
    aggregated: BTreeSet<String>,
    /// Configs pessoais (ADR-0067): nunca aparecem como candidatas ao pack.
    personal: &'a BTreeSet<String>,
}

impl Collector<'_> {
    /// Classifica um arquivo que mudou (SPEC T15, "Grupos").
    fn add(
        &mut self,
        path: &str,
        before: Option<&FileState>,
        now: Option<&Now>,
    ) -> Result<(), Error> {
        if self.personal.contains(path) {
            // Preferência de quem testa: fica só neste computador (`PersonalConfigs`).
            return Ok(());
        }
        let managed = self.manifest.files.get(path);
        let from_pack = managed.is_some_and(|entry| entry.origin == EntryOrigin::Pack);
        let change = match (before, now) {
            (None, _) => CaptureChange::Added,
            (Some(_), Some(_)) => CaptureChange::Modified,
            (Some(_), None) => CaptureChange::Removed,
        };
        let mut file = CapturedFile {
            path: path.to_owned(),
            group: CaptureGroup::Generated,
            change,
            selected: false,
            pack_path: Some(path.to_owned()),
            values: Vec::new(),
            options: Vec::new(),
            hidden_volatile: 0,
            text: false,
            before_sha256: before.map(|state| state.sha256.clone()),
            after_sha256: now.map(|now| now.sha256.clone()),
            size: now.map_or(0, |now| now.size),
            managed: managed.is_some(),
        };

        if let Some(item) = folder_item(path) {
            // Datapacks e resource packs em pasta: um item por pasta, só se ela é nova.
            let prefix = format!("{}/", item.path);
            let existed = self
                .baseline
                .files
                .keys()
                .any(|known| known.starts_with(&prefix));
            if change == CaptureChange::Added
                && !existed
                && self.aggregated.insert(item.path.clone())
            {
                file.path = item.path;
                file.group = item.group;
                file.pack_path = None;
                file.size = 0;
                file.after_sha256 = None;
                self.files.push(file);
            }
            return Ok(());
        }
        if let Some(group) = item_group(path) {
            // Mod, resource pack ou datapack (arquivo único): nunca copiado.
            let interesting = match group {
                CaptureGroup::Mod => change != CaptureChange::Modified || managed.is_none(),
                _ => change == CaptureChange::Added,
            };
            if interesting {
                file.group = group;
                file.pack_path = None;
                self.files.push(file);
            }
            return Ok(());
        }
        if let Some(rest) = world_config_rest(path) {
            if change == CaptureChange::Removed {
                return Ok(());
            }
            file.group = CaptureGroup::WorldConfig;
            file.pack_path = Some(format!("defaultconfigs/{rest}"));
        } else if path == OPTIONS_FILE && change == CaptureChange::Modified {
            file.group = CaptureGroup::Options;
        } else if from_pack && change != CaptureChange::Added {
            file.group = CaptureGroup::PackConfig;
            file.selected = change == CaptureChange::Modified;
            if let Some(start) = self.baseline.pack.get(path) {
                file.pack_path = Some(start.pack_path.clone());
            }
        } else if change == CaptureChange::Removed {
            // Arquivo que não veio do pack e sumiu: nada a trazer.
            return Ok(());
        }
        self.add_contents(file, before, now, from_pack)
    }

    /// Compara o conteúdo (por valor, ou por chave no `options.txt`) e guarda o arquivo, a não
    /// ser que só a formatação ou chaves voláteis tenham mudado.
    fn add_contents(
        &mut self,
        mut file: CapturedFile,
        before: Option<&FileState>,
        now: Option<&Now>,
        from_pack: bool,
    ) -> Result<(), Error> {
        let before_bytes = match before {
            Some(state) if state.stored => self.blobs.get(&state.sha256)?,
            _ => None,
        };
        let after_bytes = match now {
            Some(now) if now.stored => self.blobs.get(&now.sha256)?,
            _ => None,
        };
        file.text =
            text_of(before_bytes.as_deref()).is_some() || text_of(after_bytes.as_deref()).is_some();
        let both = before_bytes.as_deref().zip(after_bytes.as_deref());

        if file.group == CaptureGroup::Options {
            let Some((old, new)) = both else {
                // Sem o texto dos dois lados, o arquivo inteiro aparece como alterado.
                file.group = if from_pack {
                    CaptureGroup::PackConfig
                } else {
                    CaptureGroup::Generated
                };
                file.selected = false;
                self.files.push(file);
                return Ok(());
            };
            let (options, hidden) = options_diff(old, new);
            if options.is_empty() {
                // Só chaves voláteis: não aparece.
                self.skip_reformatted(&file.path, from_pack);
                return Ok(());
            }
            file.selected = options.iter().any(|option| option.selected);
            file.options = options;
            file.hidden_volatile = hidden;
            file.pack_path = Some(match self.baseline.pack.get(&file.path) {
                Some(start) if from_pack => start.pack_path.clone(),
                _ => format!("{}{OPTIONS_FILE}", self.layout.index_dir),
            });
            self.files.push(file);
            return Ok(());
        }

        if file.change == CaptureChange::Modified
            && let Some((old, new)) = both
            && let Some(format) = ConfigFormat::detect(Path::new(&file.path), new)
            && let Ok(diff) = compare(format, old, new)
        {
            if diff.is_same_values() {
                self.skip_reformatted(&file.path, from_pack);
                return Ok(());
            }
            file.values = diff.changes.iter().map(value_change).collect();
        }
        self.files.push(file);
        Ok(())
    }

    /// Só a formatação mudou: não aparece, e a config do pack volta à versão do pack.
    fn skip_reformatted(&mut self, path: &str, from_pack: bool) {
        self.reformatted += 1;
        if from_pack && !self.layout.preserved.contains(path) {
            self.restored.push(path.to_owned());
        }
    }
}

/// Compara a instância com a linha de base depois que o jogo fechou, grava a captura e devolve
/// à versão do pack as configs do pack que só foram reformatadas. Sem linha de base (o jogo não
/// chegou a abrir), `None`.
///
/// Erros: disco.
pub fn capture_after_exit(
    dirs: &InstanceDirs,
    pack_root: &Path,
    session_id: &str,
) -> Result<Option<CaptureOutcome>, Error> {
    capture_after_exit_with(dirs, pack_root, session_id, &BTreeSet::new())
}

/// [`capture_after_exit`] sem as configs pessoais (ADR-0067; [`crate::PersonalConfigs`]): o que
/// mudou nesses caminhos não aparece como candidato ao pack.
pub fn capture_after_exit_with(
    dirs: &InstanceDirs,
    pack_root: &Path,
    session_id: &str,
    personal: &BTreeSet<String>,
) -> Result<Option<CaptureOutcome>, Error> {
    let Some(baseline) = Baseline::load(&dirs.state_dir)? else {
        return Ok(None);
    };
    let blobs = BlobStore::of(&dirs.state_dir);
    let changed = changed_files(dirs, &baseline, &blobs)?;
    let mut collector = Collector {
        baseline: &baseline,
        manifest: Manifest::load(&dirs.state_dir)?,
        layout: pack_layout(pack_root),
        blobs,
        files: Vec::new(),
        restored: Vec::new(),
        reformatted: 0,
        aggregated: BTreeSet::new(),
        personal,
    };
    for (path, (before, now)) in &changed {
        collector.add(path, *before, now.as_ref())?;
    }
    let Collector {
        mut files,
        restored,
        reformatted,
        ..
    } = collector;
    files.sort_by(|a, b| {
        group_rank(a.group)
            .cmp(&group_rank(b.group))
            .then_with(|| a.path.cmp(&b.path))
    });
    restore_instance(dirs, &restored)?;
    let capture = Capture {
        schema_version: CAPTURE_SCHEMA,
        session_id: session_id.to_owned(),
        finished_at_ms: now_ms(),
        files,
        reformatted,
        status: CaptureStatus::Pending,
    };
    save_capture(&dirs.state_dir, &capture)?;
    Ok(Some(CaptureOutcome { capture, restored }))
}

/// Lê o arquivo agora: SHA-256 e, se for texto de config, o conteúdo no depósito. `None` se
/// sumiu no meio.
fn read_now(
    dirs: &InstanceDirs,
    blobs: &BlobStore,
    path: &str,
    size: u64,
) -> Result<Option<Now>, Error> {
    let full = dirs.game_dir.join(path);
    if keeps_text(path) && size <= MAX_TEXT_BYTES {
        let bytes = match fs::read(&full) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(Error::io("ler", instance_path(path), error)),
        };
        let sha256 = blobs.put(&bytes)?;
        return Ok(Some(Now {
            size: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
            sha256,
            stored: true,
        }));
    }
    match hash_file(&full, path) {
        Ok(sha256) => Ok(Some(Now {
            size,
            sha256,
            stored: false,
        })),
        Err(Error::Io { source, .. }) if source.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

/// Ordem dos grupos na tela.
fn group_rank(group: CaptureGroup) -> u8 {
    match group {
        CaptureGroup::PackConfig => 0,
        CaptureGroup::Options => 1,
        CaptureGroup::WorldConfig => 2,
        CaptureGroup::Mod => 3,
        CaptureGroup::Resource => 4,
        CaptureGroup::Datapack => 5,
        CaptureGroup::Generated => 6,
    }
}

/// Um item que é uma pasta (datapack ou resource pack descompactado).
struct FolderItem {
    path: String,
    group: CaptureGroup,
}

fn folder_item(path: &str) -> Option<FolderItem> {
    let parts: Vec<&str> = path.split('/').collect();
    match parts.as_slice() {
        ["saves", world, "datapacks", name, _, ..] if !world.is_empty() => Some(FolderItem {
            path: format!("saves/{world}/datapacks/{name}"),
            group: CaptureGroup::Datapack,
        }),
        [dir @ ("resourcepacks" | "shaderpacks"), name, _, ..] => Some(FolderItem {
            path: format!("{dir}/{name}"),
            group: CaptureGroup::Resource,
        }),
        _ => None,
    }
}

/// Grupo de um item que nunca é copiado: jar em `mods/`, arquivo em `resourcepacks/` ou
/// `shaderpacks/`, datapack em arquivo único.
fn item_group(path: &str) -> Option<CaptureGroup> {
    let parts: Vec<&str> = path.split('/').collect();
    match parts.as_slice() {
        ["mods", name] if name.to_ascii_lowercase().ends_with(".jar") => Some(CaptureGroup::Mod),
        ["resourcepacks" | "shaderpacks", _] => Some(CaptureGroup::Resource),
        ["saves", world, "datapacks", _] if !world.is_empty() => Some(CaptureGroup::Datapack),
        _ => None,
    }
}

/// O texto, se o conteúdo é UTF-8.
pub(crate) fn text_of(bytes: Option<&[u8]>) -> Option<&str> {
    let bytes = bytes?;
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    std::str::from_utf8(bytes).ok()
}

pub(crate) fn value_change(change: &SemanticChange) -> CaptureValue {
    match change {
        SemanticChange::Added { path, value } => CaptureValue {
            key: path.to_string(),
            before: None,
            after: Some(value.to_string()),
        },
        SemanticChange::Removed { path, value } => CaptureValue {
            key: path.to_string(),
            before: Some(value.to_string()),
            after: None,
        },
        SemanticChange::Changed {
            path,
            before,
            after,
        } => CaptureValue {
            key: path.to_string(),
            before: Some(before.to_string()),
            after: Some(after.to_string()),
        },
    }
}

/// As chaves de um `options.txt`, com o valor como está no arquivo (a última ocorrência vale).
pub(crate) fn option_values(bytes: &[u8]) -> BTreeMap<String, String> {
    let text = text_of(Some(bytes)).unwrap_or_default();
    let mut values = BTreeMap::new();
    for line in text.split('\n') {
        let line = line.strip_suffix('\r').unwrap_or(line);
        if let Some((key, value)) = line.split_once(':') {
            values.insert(key.to_owned(), value.to_owned());
        }
    }
    values
}

/// Chaves do `options.txt` que mudaram, sem as voláteis, na ordem do arquivo novo; e quantas
/// voláteis mudaram.
fn options_diff(before: &[u8], after: &[u8]) -> (Vec<CaptureOption>, u32) {
    let old = option_values(before);
    let mut changes = Vec::new();
    let mut hidden = 0_u32;
    let mut seen = BTreeSet::new();
    let text = text_of(Some(after)).unwrap_or_default();
    let new = option_values(after);
    for line in text.split('\n') {
        let line = line.strip_suffix('\r').unwrap_or(line);
        let Some((key, _)) = line.split_once(':') else {
            continue;
        };
        if !seen.insert(key) {
            continue;
        }
        let Some(value) = new.get(key) else {
            continue;
        };
        let previous = old.get(key);
        if previous == Some(value) {
            continue;
        }
        if VOLATILE_OPTIONS.contains(&key) {
            hidden += 1;
            continue;
        }
        changes.push(CaptureOption {
            key: key.to_owned(),
            before: previous.cloned(),
            after: value.clone(),
            selected: previous.is_some(),
        });
    }
    (changes, hidden)
}

/// Grava as chaves escolhidas no `options.txt` do pack: troca só o valor da linha de cada
/// chave (a última, se repetida) e acrescenta no fim as que o pack não tem. Sem arquivo no
/// pack, cria um só com as chaves escolhidas. O fim de linha do arquivo é mantido.
#[must_use]
pub fn set_option_lines(pack: Option<&[u8]>, values: &[(String, String)], ending: &str) -> Vec<u8> {
    let original = pack.unwrap_or_default();
    let (bom, body) = match original.strip_prefix(b"\xEF\xBB\xBF") {
        Some(rest) => (&b"\xEF\xBB\xBF"[..], rest),
        None => (&b""[..], original),
    };
    let text = String::from_utf8_lossy(body).into_owned();
    let ending = if text.contains("\r\n") {
        "\r\n"
    } else if text.contains('\n') {
        "\n"
    } else {
        ending
    };
    let mut trailing = text.is_empty() || text.ends_with('\n');
    let mut lines: Vec<String> = text.split('\n').map(ToOwned::to_owned).collect();
    // `split` deixa um pedaço vazio depois do último fim de linha (ou no arquivo vazio).
    if trailing {
        lines.pop();
    }
    let carriage = if ending == "\r\n" { "\r" } else { "" };
    for (key, value) in values {
        let found = lines.iter().rposition(|line| {
            line.strip_suffix('\r')
                .unwrap_or(line)
                .split_once(':')
                .is_some_and(|(line_key, _)| line_key == key)
        });
        if let Some(index) = found {
            let kept = if lines[index].ends_with('\r') {
                "\r"
            } else {
                ""
            };
            lines[index] = format!("{key}:{value}{kept}");
            continue;
        }
        if !trailing {
            // A última linha não tinha fim de linha: ganha um antes da chave nova.
            if let Some(last) = lines.last_mut() {
                last.push_str(carriage);
            }
            trailing = true;
        }
        lines.push(format!("{key}:{value}{carriage}"));
    }
    let mut out = bom.to_vec();
    out.extend_from_slice(lines.join("\n").as_bytes());
    if trailing && !lines.is_empty() {
        out.push(b'\n');
    }
    out
}

/// Devolve à versão do pack os arquivos da instância: apaga a cópia, e a próxima cópia do
/// pack os grava de novo.
pub(crate) fn restore_instance(dirs: &InstanceDirs, paths: &[String]) -> Result<(), Error> {
    for path in paths {
        let full = resolve_inside(&dirs.game_dir, path)?;
        match fs::remove_file(&full) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(Error::io("apagar", instance_path(path), error)),
        }
    }
    Ok(())
}

/// A captura pendente desse teste, ou o erro certo.
fn pending_capture(dirs: &InstanceDirs, session_id: &str) -> Result<Capture, CaptureError> {
    let capture = load_capture(&dirs.state_dir)?
        .filter(|capture| capture.session_id == session_id)
        .ok_or(CaptureError::NotFound)?;
    if capture.status != CaptureStatus::Pending {
        return Err(CaptureError::AlreadyResolved);
    }
    Ok(capture)
}

/// Configs do pack (que voltam com a próxima cópia) que mudaram na instância: com "Descartar"
/// ou depois de "Trazer", voltam à versão do pack se ainda forem diferentes dela.
fn managed_changes(capture: &Capture, layout: &PackLayout) -> Vec<String> {
    capture
        .files
        .iter()
        .filter(|file| file.managed && file.change != CaptureChange::Added)
        .filter(|file| matches!(file.group, CaptureGroup::PackConfig | CaptureGroup::Options))
        .filter(|file| !layout.preserved.contains(&file.path))
        .map(|file| file.path.clone())
        .collect()
}

/// "Descartar": as configs do pack que mudaram na instância voltam à versão do pack no próximo
/// teste; o resto (arquivos criados pelos mods, mundos) fica como está. Devolve os arquivos
/// devolvidos ao pack.
///
/// Erros: [`CaptureError::NotFound`], [`CaptureError::AlreadyResolved`], disco.
pub fn discard(
    dirs: &InstanceDirs,
    pack_root: &Path,
    session_id: &str,
) -> Result<Vec<String>, CaptureError> {
    let mut capture = pending_capture(dirs, session_id)?;
    let restored = managed_changes(&capture, &pack_layout(pack_root));
    restore_instance(dirs, &restored)?;
    capture.status = CaptureStatus::Discarded;
    save_capture(&dirs.state_dir, &capture)?;
    Ok(restored)
}

/// O que fazer com um arquivo escolhido.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum CaptureResolution {
    /// Trazer a versão do jogo ("Usar a do jogo", no conflito).
    Take,
    /// Conflito: manter a do pack (a mudança do jogo é descartada).
    KeepPack,
    /// Conflito: juntar as duas (as chaves mudadas são diferentes).
    Merge,
    /// Conflito: o texto combinado à mão no editor.
    Edited {
        /// O texto final.
        text: String,
    },
}

/// A decisão sobre um arquivo da captura.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CaptureDecision {
    /// Caminho na instância (o `path` do [`CapturedFile`]).
    pub path: String,
    /// O que fazer.
    pub resolution: CaptureResolution,
    /// SHA-256 do arquivo do pack quando a lista foi mostrada (`None`: não existia). Se o
    /// pack mudou depois disso, nada é gravado ([`CaptureError::Stale`]).
    pub expected_pack_sha256: Option<String>,
}

/// "Trazer selecionados para o pack".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CaptureApplyRequest {
    /// A sessão do teste (a captura tem de ser dela).
    pub session_id: String,
    /// Os arquivos escolhidos (o `options.txt` vai em `optionKeys`).
    pub files: Vec<CaptureDecision>,
    /// As chaves escolhidas do `options.txt`.
    pub option_keys: Vec<String>,
    /// SHA-256 do `options.txt` do pack quando a lista foi mostrada.
    pub expected_options_sha256: Option<String>,
}

/// O que gravar no pack.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ApplyPlan {
    /// Caminho no pack → conteúdo novo.
    pub writes: BTreeMap<String, Vec<u8>>,
    /// Caminhos do pack a apagar (config do pack que o jogo apagou).
    pub deletes: Vec<String>,
    /// Quantas mudanças vão para o pack (cada arquivo conta um; cada chave do `options.txt`
    /// conta uma).
    pub count: u32,
}

/// Monta o que gravar no pack para o pedido. Nada é gravado aqui.
///
/// Erros: [`CaptureError::NotFound`], [`CaptureError::AlreadyResolved`],
/// [`CaptureError::Stale`] (o pack ou a instância mudaram depois que a lista foi mostrada),
/// [`CaptureError::InvalidItem`], [`CaptureError::NotMergeable`], disco.
pub fn plan_apply(
    dirs: &InstanceDirs,
    pack_root: &Path,
    request: &CaptureApplyRequest,
) -> Result<ApplyPlan, CaptureError> {
    let capture = pending_capture(dirs, &request.session_id)?;
    let blobs = BlobStore::of(&dirs.state_dir);
    let mut plan = ApplyPlan::default();

    for decision in &request.files {
        let file = capture
            .file(&decision.path)
            .filter(|file| file.group != CaptureGroup::Options)
            .ok_or_else(|| CaptureError::InvalidItem {
                path: decision.path.clone(),
            })?;
        let pack_target = file
            .pack_path
            .clone()
            .ok_or_else(|| CaptureError::InvalidItem {
                path: decision.path.clone(),
            })?;
        let pack_now = read_pack_file(pack_root, &pack_target)?;
        let pack_sha = pack_now.as_deref().map(sha256_hex);
        if pack_sha != decision.expected_pack_sha256 {
            return Err(CaptureError::Stale { path: pack_target });
        }
        match &decision.resolution {
            CaptureResolution::KeepPack => continue,
            CaptureResolution::Take => {
                if file.change == CaptureChange::Removed {
                    if pack_now.is_some() {
                        plan.deletes.push(pack_target);
                    }
                } else {
                    let bytes = instance_bytes(dirs, &blobs, file)?;
                    plan.writes.insert(pack_target, bytes);
                }
            }
            CaptureResolution::Merge => {
                let merged =
                    three_way::merge_file(&blobs, file, pack_now.as_deref())?.ok_or_else(|| {
                        CaptureError::NotMergeable {
                            path: decision.path.clone(),
                        }
                    })?;
                plan.writes.insert(pack_target, merged);
            }
            CaptureResolution::Edited { text } => {
                let mut bytes = Vec::new();
                if pack_now
                    .as_deref()
                    .is_some_and(|pack| pack.starts_with(b"\xEF\xBB\xBF"))
                    && !text.starts_with('\u{feff}')
                {
                    bytes.extend_from_slice(b"\xEF\xBB\xBF");
                }
                bytes.extend_from_slice(text.as_bytes());
                plan.writes.insert(pack_target, bytes);
            }
        }
        plan.count += 1;
    }

    if !request.option_keys.is_empty() {
        let file = capture
            .files
            .iter()
            .find(|file| file.group == CaptureGroup::Options)
            .ok_or_else(|| CaptureError::InvalidItem {
                path: OPTIONS_FILE.to_owned(),
            })?;
        let target = file
            .pack_path
            .clone()
            .unwrap_or_else(|| OPTIONS_FILE.to_owned());
        let pack_now = read_pack_file(pack_root, &target)?;
        if pack_now.as_deref().map(sha256_hex) != request.expected_options_sha256 {
            return Err(CaptureError::Stale { path: target });
        }
        let mut values = Vec::new();
        for key in &request.option_keys {
            let option = file
                .options
                .iter()
                .find(|option| &option.key == key)
                .ok_or_else(|| CaptureError::InvalidItem {
                    path: format!("{OPTIONS_FILE}:{key}"),
                })?;
            values.push((option.key.clone(), option.after.clone()));
        }
        let ending = blobs
            .get(file.after_sha256.as_deref().unwrap_or_default())?
            .filter(|bytes| bytes.windows(2).any(|pair| pair == b"\r\n"))
            .map_or("\n", |_| "\r\n");
        let bytes = set_option_lines(pack_now.as_deref(), &values, ending);
        plan.count += u32::try_from(values.len()).unwrap_or(u32::MAX);
        plan.writes.insert(target, bytes);
    }
    Ok(plan)
}

/// O conteúdo do jogo para gravar no pack: o guardado na captura ou, sem ele, o arquivo da
/// instância, desde que não tenha mudado depois da captura.
fn instance_bytes(
    dirs: &InstanceDirs,
    blobs: &BlobStore,
    file: &CapturedFile,
) -> Result<Vec<u8>, CaptureError> {
    let expected = file.after_sha256.as_deref().unwrap_or_default();
    if let Some(bytes) = blobs.get(expected)? {
        return Ok(bytes);
    }
    let full = resolve_inside(&dirs.game_dir, &file.path)?;
    let bytes = match fs::read(&full) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Err(CaptureError::Stale {
                path: file.path.clone(),
            });
        }
        Err(error) => return Err(Error::io("ler", instance_path(&file.path), error).into()),
    };
    if sha256_hex(&bytes) != expected {
        return Err(CaptureError::Stale {
            path: file.path.clone(),
        });
    }
    Ok(bytes)
}

/// Depois de gravar no pack: marca a captura como resolvida e devolve à versão do pack o que
/// ficou diferente dele na instância (configs do pack não trazidas e arquivos que passaram a
/// ser do pack com outro conteúdo), para a próxima cópia não parar pedindo revisão. Devolve os
/// arquivos devolvidos.
///
/// Erros: [`CaptureError::NotFound`], [`CaptureError::AlreadyResolved`], disco.
pub fn finish_apply(
    dirs: &InstanceDirs,
    pack_root: &Path,
    session_id: &str,
    plan: &ApplyPlan,
) -> Result<Vec<String>, CaptureError> {
    let mut capture = pending_capture(dirs, session_id)?;
    let layout = pack_layout(pack_root);
    let mut candidates: BTreeSet<String> = managed_changes(&capture, &layout).into_iter().collect();
    candidates.extend(
        plan.writes
            .keys()
            .filter(|path| !layout.preserved.contains(*path))
            .cloned(),
    );
    let mut restored = Vec::new();
    for path in candidates {
        let Ok(full) = resolve_inside(&dirs.game_dir, &path) else {
            continue;
        };
        let instance = match fs::read(&full) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(Error::io("ler", instance_path(&path), error).into()),
        };
        let pack = read_pack_file(pack_root, &path)?;
        if pack.as_deref() != Some(instance.as_slice()) {
            restored.push(path);
        }
    }
    restore_instance(dirs, &restored)?;
    capture.status = CaptureStatus::Applied { count: plan.count };
    save_capture(&dirs.state_dir, &capture)?;
    Ok(restored)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opcoes_por_chave_sem_as_volateis() {
        let before = b"version:3465\nrenderDistance:12\nguiScale:0\nlang:en_us\nfullscreen:false\n";
        let after = b"version:3700\nrenderDistance:16\nguiScale:0\nlang:en_us\nfullscreen:true\nkey_key.jump:key.keyboard.space\n";
        let (changes, hidden) = options_diff(before, after);
        assert_eq!(hidden, 2);
        assert_eq!(
            changes,
            [
                CaptureOption {
                    key: "renderDistance".into(),
                    before: Some("12".into()),
                    after: "16".into(),
                    selected: true,
                },
                CaptureOption {
                    key: "key_key.jump".into(),
                    before: None,
                    after: "key.keyboard.space".into(),
                    selected: false,
                },
            ]
        );
    }

    #[test]
    fn opcoes_com_crlf_e_valor_com_dois_pontos() {
        let before = b"lastServer:127.0.0.1:25565\r\nrenderDistance:12\r\n";
        let after = b"lastServer:mc.exemplo:25565\r\nrenderDistance:12\r\nlang:\"pt_br\"\r\n";
        let (changes, hidden) = options_diff(before, after);
        assert_eq!(hidden, 1);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].key, "lang");
        assert_eq!(changes[0].after, "\"pt_br\"");
    }

    #[test]
    fn grava_so_a_linha_da_chave() {
        let pack = b"version:3465\nrenderDistance:12\nguiScale:0\n";
        let out = set_option_lines(Some(pack), &[("renderDistance".into(), "16".into())], "\n");
        assert_eq!(out, b"version:3465\nrenderDistance:16\nguiScale:0\n");
        let out = set_option_lines(Some(pack), &[("lang".into(), "pt_br".into())], "\n");
        assert_eq!(
            out,
            b"version:3465\nrenderDistance:12\nguiScale:0\nlang:pt_br\n"
        );
    }

    #[test]
    fn grava_mantendo_crlf_bom_e_sem_fim_de_linha() {
        let pack = b"\xEF\xBB\xBFa:1\r\nrenderDistance:12\r\n";
        let out = set_option_lines(Some(pack), &[("renderDistance".into(), "16".into())], "\n");
        assert_eq!(out, b"\xEF\xBB\xBFa:1\r\nrenderDistance:16\r\n");
        let out = set_option_lines(Some(pack), &[("b".into(), "2".into())], "\n");
        assert_eq!(out, b"\xEF\xBB\xBFa:1\r\nrenderDistance:12\r\nb:2\r\n");
        let out = set_option_lines(Some(b"a:1"), &[("a".into(), "2".into())], "\n");
        assert_eq!(out, b"a:2");
        let out = set_option_lines(Some(b"a:1\r\nb:2"), &[("c".into(), "3".into())], "\n");
        assert_eq!(out, b"a:1\r\nb:2\r\nc:3\r\n");
        let out = set_option_lines(None, &[("renderDistance".into(), "16".into())], "\r\n");
        assert_eq!(out, b"renderDistance:16\r\n");
    }

    #[test]
    fn grupos_dos_itens_que_nunca_sao_copiados() {
        assert_eq!(item_group("mods/jei.jar"), Some(CaptureGroup::Mod));
        assert_eq!(item_group("mods/JEI.JAR"), Some(CaptureGroup::Mod));
        assert_eq!(item_group("mods/sub/x.jar"), None);
        assert_eq!(item_group("mods/notas.txt"), None);
        assert_eq!(
            item_group("resourcepacks/a.zip"),
            Some(CaptureGroup::Resource)
        );
        assert_eq!(
            item_group("shaderpacks/b.zip"),
            Some(CaptureGroup::Resource)
        );
        assert_eq!(
            item_group("saves/Mundo/datapacks/x.zip"),
            Some(CaptureGroup::Datapack)
        );
        let folder = folder_item("saves/Mundo de teste/datapacks/meu/data/x.json").unwrap();
        assert_eq!(folder.path, "saves/Mundo de teste/datapacks/meu");
        assert_eq!(folder.group, CaptureGroup::Datapack);
        let folder = folder_item("resourcepacks/Faithful/pack.mcmeta").unwrap();
        assert_eq!(folder.path, "resourcepacks/Faithful");
        assert!(folder_item("config/a.toml").is_none());
    }
}
