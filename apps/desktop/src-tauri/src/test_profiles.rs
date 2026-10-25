//! Perfis do teste (SPEC T11 "Perfis do teste"; ARCHITECTURE §7.8; ROADMAP L-08).
//!
//! Um perfil é um conjunto com nome de: memória, Java, argumentos da JVM, mods opcionais
//! ligados, janela do jogo e "Ao abrir o jogo" (menu principal ou entrar direto no mundo de
//! teste, pelo Quick Play). Ficam **neste computador**, por pack, em `packs.json` (dado de
//! máquina, ADR-0025): nunca no pack, não contam como alteração não salva e não guardam nada
//! pessoal (só números, o id do Java, os argumentos digitados e nomes de mundos de teste).
//!
//! ## Onde fica cada coisa em `packs.json` (`PackRecord.extra`)
//!
//! - `testSettings` (P1-08): memória, Java e argumentos do perfil **Padrão**. Continua sendo a
//!   fonte do Padrão, para quem já tinha ajustes gravados antes dos perfis.
//! - `testProfiles` (L-08): `{ active, default: { window, onLaunch }, profiles: [...] }`. O
//!   `default` guarda o que o Padrão ganhou com os perfis; `profiles` guarda os outros perfis
//!   completos. Campos desconhecidos (de uma versão mais nova) são preservados.
//!
//! O Padrão tem o id fixo [`DEFAULT_PROFILE`] (o mesmo que a sessão grava, L-04), não pode ser
//! apagado nem renomeado, e usa as escolhas de opcionais da instância
//! (`state/optional-choices.json`). Os outros perfis guardam as escolhas próprias
//! (`optionals`); sem escolhas (`None`), seguem as da instância.
//!
//! ## API para outras tarefas (P1-19 "Importar modpack", L-09, D-12)
//!
//! - [`create_profile`] cria um perfil a partir de [`ProfileFields`] e devolve o perfil criado
//!   (a P1-19 converte o `instance.cfg` do Prism/MultiMC em `ProfileFields` e chama esta função;
//!   o nome é conferido por [`check_name`]).
//! - [`profiles_of`] lê os perfis de um pack; [`select_profile`] escolhe o ativo.
//! - [`resolve`] devolve o perfil de um teste (o pedido ou o ativo), já com o Padrão completo.
//! - [`perf_flags`] e [`memory_above_ram`] dão os avisos `W_PERF_FLAGS` e de memória acima de
//!   75% da RAM; [`TestWindow::game_args`] e [`quick_play_world`] montam a janela e o mundo.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Mutex, PoisonError};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use warden_core::PackId;
use warden_java::RuntimeId;
use warden_project::ProjectErrorCode;

use warden_launcher::{LaunchCommand, LaunchOptions, QuickPlay};

use crate::commands::pack_meta::{
    MAX_JVM_ARGS, TestSettings, read_test_settings, write_test_settings,
};
use crate::error::{AppError, AppErrorCode};
use crate::settings::TestMemory;
use crate::state::AppState;

/// Campo de `PackRecord.extra` com os perfis do pack.
pub(crate) const TEST_PROFILES_KEY: &str = "testProfiles";

/// Id do perfil Padrão (fixo; é o que a sessão grava quando não há outro perfil).
pub(crate) const DEFAULT_PROFILE: &str = "default";

/// Maior nome de perfil, em caracteres.
pub(crate) const MAX_NAME: usize = 40;

/// Quantos perfis um pack pode ter, contando o Padrão.
pub(crate) const MAX_PROFILES: usize = 20;

/// Faixa aceita para a memória fixa (a mesma dos Ajustes do teste, P1-08).
const MEMORY_RANGE_MB: std::ops::RangeInclusive<u32> = 512..=65_536;

/// Faixa aceita para a largura e a altura da janela.
const WINDOW_RANGE: std::ops::RangeInclusive<u32> = 320..=7680;

/// Acima desta fração da memória do computador, a memória do teste gera aviso (SPEC T11).
const MEMORY_WARN_PERCENT: u64 = 75;

/// Argumentos que impedem o Warden de ler a memória da JVM (ADR-0038; `W_PERF_FLAGS`).
pub(crate) const PERF_FLAGS: [&str; 2] = ["-XX:+PerfDisableSharedMem", "-XX:-UsePerfData"];

/// Uma gravação de perfis por vez no app (ler, mudar e gravar `testProfiles` sem perder a
/// mudança de outro comando no meio).
static WRITE: Mutex<()> = Mutex::new(());

/// Tamanho da janela do jogo.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(tag = "mode", rename_all = "camelCase")]
pub(crate) enum TestWindow {
    /// O tamanho que o jogo escolhe (854 × 480).
    #[default]
    Default,
    /// Largura e altura fixas (`--width`/`--height`).
    Size {
        /// Largura, em pixels.
        width: u32,
        /// Altura, em pixels.
        height: u32,
    },
    /// Tela cheia (`--fullscreen`).
    Fullscreen,
}

impl TestWindow {
    /// Os argumentos do jogo (aceitos do 1.7.10 à versão mais nova).
    pub(crate) fn game_args(self) -> Vec<String> {
        match self {
            Self::Default => Vec::new(),
            Self::Size { width, height } => vec![
                "--width".into(),
                width.to_string(),
                "--height".into(),
                height.to_string(),
            ],
            Self::Fullscreen => vec!["--fullscreen".into()],
        }
    }

    /// Como a assinatura do perfil (gancho 1.2) grava a janela.
    pub(crate) fn signature(self) -> Option<String> {
        match self {
            Self::Default => None,
            Self::Size { width, height } => Some(format!("{width}x{height}")),
            Self::Fullscreen => Some("fullscreen".into()),
        }
    }
}

/// "Ao abrir o jogo".
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(tag = "mode", rename_all = "camelCase")]
pub(crate) enum OnLaunch {
    /// Ir para o menu principal.
    #[default]
    Menu,
    /// Entrar direto no mundo de teste (Quick Play; Minecraft 1.20 ou mais novo).
    World {
        /// Pasta do mundo em `saves/` (`None` = o último jogado).
        world: Option<String>,
    },
}

impl OnLaunch {
    /// Como a assinatura do perfil (gancho 1.2) grava "Ao abrir o jogo".
    pub(crate) fn signature(&self) -> &'static str {
        match self {
            Self::Menu => "menu",
            Self::World { .. } => "world",
        }
    }
}

/// Os campos de um perfil, sem id nem nome.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProfileFields {
    /// Memória do teste: Automático ou valor fixo.
    pub(crate) memory: TestMemory,
    /// Java escolhido (`null` = Automático).
    pub(crate) java: Option<RuntimeId>,
    /// Argumentos extras da JVM, como o usuário digitou.
    pub(crate) jvm_args: String,
    /// Mods opcionais ligados (caminho do `.pw.toml` → ligado). `null` = as escolhas da
    /// instância (as do Padrão).
    pub(crate) optionals: Option<BTreeMap<String, bool>>,
    /// Janela do jogo.
    pub(crate) window: TestWindow,
    /// Ao abrir o jogo.
    pub(crate) on_launch: OnLaunch,
}

/// Um perfil do teste.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TestProfile {
    /// Id estável (`default` para o Padrão).
    pub(crate) id: String,
    /// Nome dado pelo usuário (vazio no Padrão: a interface mostra "Padrão").
    pub(crate) name: String,
    /// Os ajustes.
    #[serde(flatten)]
    pub(crate) fields: ProfileFields,
}

impl TestProfile {
    /// Se é o Padrão.
    pub(crate) fn is_default(&self) -> bool {
        self.id == DEFAULT_PROFILE
    }
}

/// O que o Padrão guarda em `testProfiles.default` (o resto está em `testSettings`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct DefaultExtras {
    window: TestWindow,
    on_launch: OnLaunch,
    #[serde(flatten)]
    unknown: Map<String, Value>,
}

/// Um perfil gravado, com os campos desconhecidos preservados.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredProfile {
    #[serde(flatten)]
    profile: TestProfile,
    #[serde(flatten)]
    unknown: Map<String, Value>,
}

/// O valor de `testProfiles`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct Stored {
    active: Option<String>,
    /// O maior número de id já dado (`perfil-N`), para um id apagado nunca voltar (as sessões
    /// gravam o id do perfil).
    last_id: u32,
    default: DefaultExtras,
    profiles: Vec<StoredProfile>,
    #[serde(flatten)]
    unknown: Map<String, Value>,
}

/// Os perfis de um pack: o ativo e a lista, com o Padrão primeiro.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Profiles {
    /// Id do perfil ativo (o que o Testar usa).
    pub(crate) active: String,
    /// Os perfis, com o Padrão primeiro.
    pub(crate) profiles: Vec<TestProfile>,
}

impl Profiles {
    /// O perfil com o id.
    pub(crate) fn get(&self, id: &str) -> Option<&TestProfile> {
        self.profiles.iter().find(|profile| profile.id == id)
    }
}

fn domain(error: &warden_project::Error) -> AppError {
    AppError::from_domain(error)
}

fn invalid(field: &str) -> AppError {
    AppError::new(ProjectErrorCode::InvalidInput).with_param("field", field)
}

fn not_found(id: &str) -> AppError {
    AppError::new(AppErrorCode::TestProfileNotFound).with_param("profileId", id)
}

/// Os campos de [`TestProfile`] em `packs.json`. O `flatten` duplo (perfil e campos) não tira
/// do mapa de desconhecidos o que o perfil já leu; sem esta limpeza, o valor antigo voltaria
/// por cima do novo ao gravar.
const PROFILE_KEYS: [&str; 8] = [
    "id",
    "name",
    "memory",
    "java",
    "jvmArgs",
    "optionals",
    "window",
    "onLaunch",
];

fn read_stored(state: &AppState, pack_id: PackId) -> Result<Stored, AppError> {
    let record = state.packs.get(pack_id).map_err(|error| domain(&error))?;
    let mut stored: Stored = record
        .extra
        .get(TEST_PROFILES_KEY)
        .and_then(|value| {
            serde_json::from_value(value.clone())
                .inspect_err(|error| {
                    tracing::warn!(%error, "perfis do teste ilegíveis em packs.json; usando só o Padrão");
                })
                .ok()
        })
        .unwrap_or_default();
    for item in &mut stored.profiles {
        item.unknown
            .retain(|key, _| !PROFILE_KEYS.contains(&key.as_str()));
    }
    Ok(stored)
}

fn write_stored(state: &AppState, pack_id: PackId, stored: &Stored) -> Result<(), AppError> {
    let value = serde_json::to_value(stored)
        .map_err(|error| AppError::internal(format!("perfis do teste: {error}")))?;
    state
        .packs
        .set_extra(pack_id, TEST_PROFILES_KEY, Some(value))
        .map_err(|error| domain(&error))
}

fn default_profile(settings: TestSettings, extras: &DefaultExtras) -> TestProfile {
    TestProfile {
        id: DEFAULT_PROFILE.into(),
        name: String::new(),
        fields: ProfileFields {
            memory: settings.memory,
            java: settings.java,
            jvm_args: settings.jvm_args,
            optionals: None,
            window: extras.window,
            on_launch: extras.on_launch.clone(),
        },
    }
}

fn assemble(settings: TestSettings, stored: &Stored) -> Profiles {
    let mut profiles = vec![default_profile(settings, &stored.default)];
    profiles.extend(
        stored
            .profiles
            .iter()
            .filter(|item| item.profile.id != DEFAULT_PROFILE)
            .map(|item| item.profile.clone()),
    );
    let active = stored
        .active
        .clone()
        .filter(|id| profiles.iter().any(|profile| &profile.id == id))
        .unwrap_or_else(|| DEFAULT_PROFILE.into());
    Profiles { active, profiles }
}

/// Os perfis do pack (o Padrão sempre existe).
pub(crate) fn profiles_of(state: &AppState, pack_id: PackId) -> Result<Profiles, AppError> {
    let settings = read_test_settings(state, pack_id)?;
    let stored = read_stored(state, pack_id)?;
    Ok(assemble(settings, &stored))
}

/// Confere um nome de perfil: sem espaços nas pontas, de 1 a [`MAX_NAME`] caracteres, sem
/// repetir outro perfil (sem diferença de maiúsculas) nem o "Padrão". `except` é o perfil que
/// está sendo renomeado. Devolve o nome limpo.
pub(crate) fn check_name(
    profiles: &Profiles,
    name: &str,
    except: Option<&str>,
) -> Result<String, AppError> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > MAX_NAME || name.chars().any(char::is_control) {
        return Err(invalid("name").with_param("max", MAX_NAME.to_string()));
    }
    let lower = name.to_lowercase();
    let taken = lower == "padrão"
        || lower == "padrao"
        || profiles.profiles.iter().any(|profile| {
            !profile.is_default()
                && Some(profile.id.as_str()) != except
                && profile.name.to_lowercase() == lower
        });
    if taken {
        return Err(AppError::new(AppErrorCode::TestProfileNameTaken).with_param("name", name));
    }
    Ok(name.to_owned())
}

/// Confere e limpa os campos de um perfil (as mesmas regras dos Ajustes do teste).
fn check_fields(mut fields: ProfileFields) -> Result<ProfileFields, AppError> {
    if let TestMemory::Fixed { mb } = fields.memory
        && !MEMORY_RANGE_MB.contains(&mb)
    {
        return Err(invalid("memory"));
    }
    if fields.jvm_args.chars().count() > MAX_JVM_ARGS || fields.jvm_args.contains('\0') {
        return Err(invalid("jvmArgs"));
    }
    fields.jvm_args = fields.jvm_args.trim().to_owned();
    if let TestWindow::Size { width, height } = fields.window
        && !(WINDOW_RANGE.contains(&width) && WINDOW_RANGE.contains(&height))
    {
        return Err(invalid("window"));
    }
    if let OnLaunch::World { world: Some(world) } = &fields.on_launch
        && !is_world_name(world)
    {
        return Err(invalid("world"));
    }
    if let Some(optionals) = &fields.optionals
        && optionals
            .keys()
            .any(|path| path.is_empty() || path.contains(['\0', '\\']))
    {
        return Err(invalid("optionals"));
    }
    Ok(fields)
}

/// Uma pasta de mundo em `saves/` (um nome só, sem caminho).
fn is_world_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 255
        && name != "."
        && name != ".."
        && !name.contains(['/', '\\', '\0', ':'])
}

/// O próximo id livre (`perfil-1`, `perfil-2`…), nunca o de um perfil apagado; anota o
/// número dado em `last_id`.
fn next_id(stored: &mut Stored) -> String {
    let highest = stored
        .profiles
        .iter()
        .filter_map(|item| item.profile.id.strip_prefix("perfil-")?.parse::<u32>().ok())
        .max()
        .unwrap_or(0)
        .max(stored.last_id);
    stored.last_id = highest.saturating_add(1);
    format!("perfil-{}", stored.last_id)
}

/// Cria um perfil com `name` e `fields` (não fica ativo). Erros: nome inválido ou repetido,
/// campos inválidos, mais de [`MAX_PROFILES`] perfis.
pub(crate) fn create_profile(
    state: &AppState,
    pack_id: PackId,
    name: &str,
    fields: ProfileFields,
) -> Result<TestProfile, AppError> {
    let _write = WRITE.lock().unwrap_or_else(PoisonError::into_inner);
    let current = profiles_of(state, pack_id)?;
    let name = check_name(&current, name, None)?;
    if current.profiles.len() >= MAX_PROFILES {
        return Err(AppError::new(AppErrorCode::TestProfileLimit)
            .with_param("max", MAX_PROFILES.to_string()));
    }
    let fields = check_fields(fields)?;
    let mut stored = read_stored(state, pack_id)?;
    let profile = TestProfile {
        id: next_id(&mut stored),
        name,
        fields,
    };
    stored.profiles.push(StoredProfile {
        profile: profile.clone(),
        unknown: Map::new(),
    });
    write_stored(state, pack_id, &stored)?;
    Ok(profile)
}

/// Renomeia um perfil (o Padrão não tem nome próprio).
pub(crate) fn rename_profile(
    state: &AppState,
    pack_id: PackId,
    id: &str,
    name: &str,
) -> Result<(), AppError> {
    let _write = WRITE.lock().unwrap_or_else(PoisonError::into_inner);
    if id == DEFAULT_PROFILE {
        return Err(AppError::new(AppErrorCode::TestProfileDefaultLocked));
    }
    let current = profiles_of(state, pack_id)?;
    let name = check_name(&current, name, Some(id))?;
    let mut stored = read_stored(state, pack_id)?;
    let item = stored
        .profiles
        .iter_mut()
        .find(|item| item.profile.id == id)
        .ok_or_else(|| not_found(id))?;
    item.profile.name = name;
    write_stored(state, pack_id, &stored)
}

/// Apaga um perfil (o Padrão não pode). Se era o ativo, o Padrão passa a ser.
pub(crate) fn delete_profile(state: &AppState, pack_id: PackId, id: &str) -> Result<(), AppError> {
    let _write = WRITE.lock().unwrap_or_else(PoisonError::into_inner);
    if id == DEFAULT_PROFILE {
        return Err(AppError::new(AppErrorCode::TestProfileDefaultLocked));
    }
    let mut stored = read_stored(state, pack_id)?;
    let before = stored.profiles.len();
    stored.profiles.retain(|item| item.profile.id != id);
    if stored.profiles.len() == before {
        return Err(not_found(id));
    }
    if stored.active.as_deref() == Some(id) {
        stored.active = None;
    }
    write_stored(state, pack_id, &stored)
}

/// Escolhe o perfil ativo (o que o Testar usa).
pub(crate) fn select_profile(state: &AppState, pack_id: PackId, id: &str) -> Result<(), AppError> {
    let _write = WRITE.lock().unwrap_or_else(PoisonError::into_inner);
    let current = profiles_of(state, pack_id)?;
    if current.get(id).is_none() {
        return Err(not_found(id));
    }
    let mut stored = read_stored(state, pack_id)?;
    stored.active = (id != DEFAULT_PROFILE).then(|| id.to_owned());
    write_stored(state, pack_id, &stored)
}

/// Grava os campos de perfis existentes (o diálogo Ajustes do teste salva todos os que
/// mudaram de uma vez). O nome de cada um também é gravado (conferido). O Padrão vai para
/// `testSettings` (memória, Java, argumentos) e `testProfiles.default` (janela, "Ao abrir o
/// jogo"); as escolhas de opcionais do Padrão são as da instância e não passam por aqui.
pub(crate) fn save_profiles(
    state: &AppState,
    pack_id: PackId,
    changed: Vec<TestProfile>,
) -> Result<(), AppError> {
    let _write = WRITE.lock().unwrap_or_else(PoisonError::into_inner);
    let current = profiles_of(state, pack_id)?;
    let mut stored = read_stored(state, pack_id)?;
    let mut settings: Option<TestSettings> = None;
    for profile in changed {
        let fields = check_fields(profile.fields)?;
        if profile.id == DEFAULT_PROFILE {
            let mut base = read_test_settings(state, pack_id)?;
            base.memory = fields.memory;
            base.java = fields.java;
            base.jvm_args = fields.jvm_args;
            settings = Some(base);
            stored.default.window = fields.window;
            stored.default.on_launch = fields.on_launch;
            continue;
        }
        let name = check_name(&current, &profile.name, Some(&profile.id))?;
        let item = stored
            .profiles
            .iter_mut()
            .find(|item| item.profile.id == profile.id)
            .ok_or_else(|| not_found(&profile.id))?;
        item.profile.name = name;
        item.profile.fields = fields;
    }
    // Cada nome foi conferido contra os gravados; dois do mesmo lote também não podem repetir.
    let mut names: Vec<String> = Vec::new();
    for item in &stored.profiles {
        let lower = item.profile.name.to_lowercase();
        if names.contains(&lower) {
            return Err(AppError::new(AppErrorCode::TestProfileNameTaken)
                .with_param("name", &item.profile.name));
        }
        names.push(lower);
    }
    if let Some(settings) = settings {
        write_test_settings(state, pack_id, settings)?;
    }
    write_stored(state, pack_id, &stored)
}

/// O perfil de um teste: o pedido (`requested`) ou, sem pedido, o ativo.
pub(crate) fn resolve(
    state: &AppState,
    pack_id: PackId,
    requested: Option<&str>,
) -> Result<TestProfile, AppError> {
    let profiles = profiles_of(state, pack_id)?;
    let id = requested.unwrap_or(&profiles.active);
    profiles.get(id).cloned().ok_or_else(|| not_found(id))
}

/// Os argumentos de `args` que impedem a leitura de memória da JVM (`W_PERF_FLAGS`).
pub(crate) fn perf_flags(args: &[String]) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    for arg in args {
        if PERF_FLAGS.contains(&arg.as_str()) && !found.contains(arg) {
            found.push(arg.clone());
        }
    }
    found
}

/// Se `memory_mb` passa de 75% da memória do computador (`total_mb`, quando conhecida).
pub(crate) fn memory_above_ram(memory_mb: u32, total_mb: Option<u64>) -> bool {
    total_mb.is_some_and(|total| u64::from(memory_mb) * 100 > total * MEMORY_WARN_PERCENT)
}

/// O mundo do Quick Play em `saves/`: o pedido, se existir; sem pedido (ou se ele foi
/// apagado), o jogado por último (o `level.dat` mais novo). `None` se não houver mundo: o jogo
/// abre no menu, porque com um mundo inexistente ele fica parado depois do menu (S-R5-3 §4.1).
pub(crate) fn quick_play_world(game_dir: &Path, wanted: Option<&str>) -> Option<String> {
    let saves = game_dir.join("saves");
    if let Some(name) = wanted
        && is_world_name(name)
        && saves.join(name).join("level.dat").is_file()
    {
        return Some(name.to_owned());
    }
    std::fs::read_dir(&saves)
        .ok()?
        .flatten()
        .filter_map(|entry| {
            let modified = entry
                .path()
                .join("level.dat")
                .metadata()
                .ok()?
                .modified()
                .ok()?;
            Some((modified, entry.file_name().to_string_lossy().into_owned()))
        })
        .filter(|(_, name)| is_world_name(name))
        .max()
        .map(|(_, name)| name)
}

/// Põe o mundo do Quick Play nas opções da abertura (`None` = menu principal). O arquivo do
/// `--quickPlayPath` não é pedido: ele cairia na captura "O que mudou durante o teste".
pub(crate) fn apply_quick_play(options: &mut LaunchOptions, world: Option<&str>) {
    options.quick_play = world.map(|world| QuickPlay::Singleplayer {
        world: world.to_owned(),
    });
}

/// Acrescenta os argumentos da janela no fim da linha do jogo. Com `@argfile`, eles vêm
/// depois do arquivo, que já termina com a classe principal e os argumentos do jogo: o Java
/// os entrega ao jogo do mesmo jeito.
pub(crate) fn append_window(command: &mut LaunchCommand, window: TestWindow) {
    command.args.extend(window.game_args());
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    use warden_launcher::command::{TargetOs, build};
    use warden_launcher::{
        GameSpec, InstalledGame, JavaRuntime, LoaderSpec, LoggingConfig, OfflineProfile,
    };
    use warden_project::registry::PackRecord;

    use super::*;
    use crate::error::ErrorCode;
    use crate::state::tests::test_state;
    use crate::test_session::{choose_memory, launch_options};

    fn register(state: &AppState) -> PackId {
        let record = PackRecord {
            id: PackId::new(),
            name: "Vale Sereno".into(),
            path: PathBuf::from("pack"),
            last_test: None,
            extra: BTreeMap::new(),
        };
        let id = record.id;
        state.packs.insert(record).unwrap();
        id
    }

    fn fixed(gb: u32) -> ProfileFields {
        ProfileFields {
            memory: TestMemory::Fixed { mb: gb * 1024 },
            ..ProfileFields::default()
        }
    }

    fn code(error: &AppError) -> ErrorCode {
        error.code
    }

    #[test]
    fn so_o_padrao_quando_nada_foi_gravado() {
        let dir = tempfile::tempdir().unwrap();
        let state = test_state(dir.path());
        let pack = register(&state);
        let profiles = profiles_of(&state, pack).unwrap();
        assert_eq!(profiles.active, DEFAULT_PROFILE);
        assert_eq!(profiles.profiles.len(), 1);
        assert!(profiles.profiles[0].is_default());
        assert_eq!(profiles.profiles[0].fields, ProfileFields::default());
        let resolved = resolve(&state, pack, None).unwrap();
        assert!(resolved.is_default());
    }

    #[test]
    fn criar_escolher_renomear_e_apagar() {
        let dir = tempfile::tempdir().unwrap();
        let state = test_state(dir.path());
        let pack = register(&state);

        let weak = create_profile(&state, pack, "  PC fraco (4 GB) ", fixed(4)).unwrap();
        assert_eq!(weak.id, "perfil-1");
        assert_eq!(weak.name, "PC fraco (4 GB)");
        let shaders = create_profile(&state, pack, "Shaders", fixed(8)).unwrap();
        assert_eq!(shaders.id, "perfil-2");
        // Criar não muda o ativo.
        assert_eq!(profiles_of(&state, pack).unwrap().active, DEFAULT_PROFILE);

        select_profile(&state, pack, "perfil-1").unwrap();
        let active = resolve(&state, pack, None).unwrap();
        assert_eq!(active.id, "perfil-1");
        assert_eq!(active.fields.memory, TestMemory::Fixed { mb: 4096 });
        // Pedido explícito vence o ativo.
        assert_eq!(
            resolve(&state, pack, Some("perfil-2")).unwrap().id,
            "perfil-2"
        );
        assert_eq!(
            code(&resolve(&state, pack, Some("sumiu")).unwrap_err()),
            ErrorCode::App(AppErrorCode::TestProfileNotFound)
        );

        rename_profile(&state, pack, "perfil-1", "PC fraco").unwrap();
        assert_eq!(
            profiles_of(&state, pack).unwrap().profiles[1].name,
            "PC fraco"
        );
        // Renomear para o próprio nome (com outra caixa) pode; para o de outro, não.
        rename_profile(&state, pack, "perfil-1", "pc FRACO").unwrap();
        assert_eq!(
            code(&rename_profile(&state, pack, "perfil-2", "Pc Fraco").unwrap_err()),
            ErrorCode::App(AppErrorCode::TestProfileNameTaken)
        );

        // Apagar o ativo volta para o Padrão; o id apagado não é reaproveitado por engano.
        delete_profile(&state, pack, "perfil-1").unwrap();
        let profiles = profiles_of(&state, pack).unwrap();
        assert_eq!(profiles.active, DEFAULT_PROFILE);
        assert_eq!(profiles.profiles.len(), 2);
        let third = create_profile(&state, pack, "Outro", fixed(6)).unwrap();
        assert_eq!(third.id, "perfil-3");
        // Nem o do último perfil criado: as sessões antigas guardam o id.
        delete_profile(&state, pack, "perfil-3").unwrap();
        let fourth = create_profile(&state, pack, "Mais um", fixed(6)).unwrap();
        assert_eq!(fourth.id, "perfil-4");
    }

    #[test]
    fn padrao_nao_pode_ser_renomeado_nem_apagado_e_nomes_sao_conferidos() {
        let dir = tempfile::tempdir().unwrap();
        let state = test_state(dir.path());
        let pack = register(&state);
        let locked = ErrorCode::App(AppErrorCode::TestProfileDefaultLocked);
        assert_eq!(
            code(&delete_profile(&state, pack, DEFAULT_PROFILE).unwrap_err()),
            locked
        );
        assert_eq!(
            code(&rename_profile(&state, pack, DEFAULT_PROFILE, "Outro").unwrap_err()),
            locked
        );
        let invalid = ErrorCode::Project(ProjectErrorCode::InvalidInput);
        for name in ["", "   ", &"x".repeat(MAX_NAME + 1), "linha\nquebrada"] {
            assert_eq!(
                code(&create_profile(&state, pack, name, ProfileFields::default()).unwrap_err()),
                invalid,
                "{name:?}"
            );
        }
        for name in ["Padrão", "padrao"] {
            assert_eq!(
                code(&create_profile(&state, pack, name, ProfileFields::default()).unwrap_err()),
                ErrorCode::App(AppErrorCode::TestProfileNameTaken)
            );
        }
        let bad_memory = ProfileFields {
            memory: TestMemory::Fixed { mb: 100 },
            ..ProfileFields::default()
        };
        assert_eq!(
            code(&create_profile(&state, pack, "Pouca", bad_memory).unwrap_err()),
            invalid
        );
        let bad_world = ProfileFields {
            on_launch: OnLaunch::World {
                world: Some("../fora".into()),
            },
            ..ProfileFields::default()
        };
        assert_eq!(
            code(&create_profile(&state, pack, "Fora", bad_world).unwrap_err()),
            invalid
        );
        let bad_window = ProfileFields {
            window: TestWindow::Size {
                width: 10,
                height: 10,
            },
            ..ProfileFields::default()
        };
        assert_eq!(
            code(&create_profile(&state, pack, "Janela", bad_window).unwrap_err()),
            invalid
        );
        for index in 1..MAX_PROFILES {
            create_profile(
                &state,
                pack,
                &format!("Perfil {index}"),
                ProfileFields::default(),
            )
            .unwrap();
        }
        assert_eq!(
            code(&create_profile(&state, pack, "Demais", ProfileFields::default()).unwrap_err()),
            ErrorCode::App(AppErrorCode::TestProfileLimit)
        );
    }

    #[test]
    fn salvar_grava_o_padrao_nos_ajustes_e_os_outros_nos_perfis() {
        let dir = tempfile::tempdir().unwrap();
        let state = test_state(dir.path());
        let pack = register(&state);
        let weak = create_profile(&state, pack, "PC fraco", fixed(4)).unwrap();

        let mut profiles = profiles_of(&state, pack).unwrap().profiles;
        profiles[0].fields = ProfileFields {
            memory: TestMemory::Fixed { mb: 6144 },
            jvm_args: "  -XX:+UseZGC ".into(),
            window: TestWindow::Fullscreen,
            on_launch: OnLaunch::World { world: None },
            // O Padrão usa as escolhas da instância: o que vier aqui é ignorado.
            optionals: Some(BTreeMap::from([("mods/a.pw.toml".into(), true)])),
            ..ProfileFields::default()
        };
        profiles[1].fields.optionals = Some(BTreeMap::from([("mods/a.pw.toml".into(), false)]));
        profiles[1].name = "PC fraquinho".into();
        save_profiles(&state, pack, profiles).unwrap();

        // O Padrão continua em `testSettings` (o que a P1-08 e o Prism leem).
        let settings = read_test_settings(&state, pack).unwrap();
        assert_eq!(settings.memory, TestMemory::Fixed { mb: 6144 });
        assert_eq!(settings.jvm_args, "-XX:+UseZGC");
        let saved = profiles_of(&state, pack).unwrap();
        assert_eq!(saved.profiles[0].fields.window, TestWindow::Fullscreen);
        assert_eq!(
            saved.profiles[0].fields.on_launch,
            OnLaunch::World { world: None }
        );
        assert_eq!(saved.profiles[0].fields.optionals, None);
        assert_eq!(saved.profiles[1].id, weak.id);
        assert_eq!(saved.profiles[1].name, "PC fraquinho");
        assert_eq!(
            saved.profiles[1].fields.optionals,
            Some(BTreeMap::from([("mods/a.pw.toml".into(), false)]))
        );

        // Dois perfis com o mesmo nome no mesmo lote: recusado, nada é gravado.
        create_profile(&state, pack, "Shaders", fixed(8)).unwrap();
        let mut profiles = profiles_of(&state, pack).unwrap().profiles;
        profiles[1].name = "Igual".into();
        profiles[2].name = "igual".into();
        assert_eq!(
            code(&save_profiles(&state, pack, profiles).unwrap_err()),
            ErrorCode::App(AppErrorCode::TestProfileNameTaken)
        );
        assert_eq!(
            profiles_of(&state, pack).unwrap().profiles[1].name,
            "PC fraquinho"
        );
    }

    #[test]
    fn campos_desconhecidos_sao_preservados_e_valor_ilegivel_vale_o_padrao() {
        let dir = tempfile::tempdir().unwrap();
        let state = test_state(dir.path());
        let pack = register(&state);
        state
            .packs
            .set_extra(
                pack,
                TEST_PROFILES_KEY,
                Some(serde_json::json!({
                    "active": "perfil-7",
                    "futuro": 1,
                    "default": { "window": { "mode": "fullscreen" }, "novo": true },
                    "profiles": [{
                        "id": "perfil-7", "name": "Antigo", "memory": { "mode": "auto" },
                        "java": null, "jvmArgs": "", "optionals": null,
                        "window": { "mode": "default" }, "onLaunch": { "mode": "menu" },
                        "cor": "verde"
                    }]
                })),
            )
            .unwrap();
        assert_eq!(resolve(&state, pack, None).unwrap().id, "perfil-7");
        rename_profile(&state, pack, "perfil-7", "Renomeado").unwrap();
        let value = state.packs.get(pack).unwrap().extra[TEST_PROFILES_KEY].clone();
        assert_eq!(value["futuro"], 1);
        assert_eq!(value["default"]["novo"], true);
        assert_eq!(value["profiles"][0]["cor"], "verde");
        assert_eq!(value["profiles"][0]["name"], "Renomeado");

        state
            .packs
            .set_extra(pack, TEST_PROFILES_KEY, Some(serde_json::json!("lixo")))
            .unwrap();
        let profiles = profiles_of(&state, pack).unwrap();
        assert_eq!(profiles.active, DEFAULT_PROFILE);
        assert_eq!(profiles.profiles.len(), 1);
    }

    #[test]
    fn avisos_de_memoria_e_de_argumentos() {
        assert!(memory_above_ram(13_000, Some(16_384)));
        assert!(!memory_above_ram(12_288, Some(16_384)));
        assert!(!memory_above_ram(65_536, None));
        let args: Vec<String> = [
            "-XX:+UseG1GC",
            "-XX:+PerfDisableSharedMem",
            "-XX:-UsePerfData",
            "-XX:+PerfDisableSharedMem",
        ]
        .map(String::from)
        .to_vec();
        assert_eq!(
            perf_flags(&args),
            ["-XX:+PerfDisableSharedMem", "-XX:-UsePerfData"]
        );
        assert!(perf_flags(&["-XX:+UseG1GC".into()]).is_empty());
    }

    #[test]
    fn mundo_do_quick_play() {
        let dir = tempfile::tempdir().unwrap();
        let game = dir.path();
        assert_eq!(quick_play_world(game, None), None);
        let saves = game.join("saves");
        for name in ["Antigo", "Novo"] {
            std::fs::create_dir_all(saves.join(name)).unwrap();
            std::fs::write(saves.join(name).join("level.dat"), b"x").unwrap();
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        // Pasta sem level.dat não é mundo.
        std::fs::create_dir_all(saves.join("Vazio")).unwrap();
        assert_eq!(quick_play_world(game, None).as_deref(), Some("Novo"));
        assert_eq!(
            quick_play_world(game, Some("Antigo")).as_deref(),
            Some("Antigo")
        );
        // O pedido sumiu (ou é inválido): o jogado por último.
        assert_eq!(
            quick_play_world(game, Some("Apagado")).as_deref(),
            Some("Novo")
        );
        assert_eq!(
            quick_play_world(game, Some("Vazio")).as_deref(),
            Some("Novo")
        );
        assert_eq!(
            quick_play_world(game, Some("../x")).as_deref(),
            Some("Novo")
        );
    }

    /// Um jogo instalado vanilla da versão (só o que a linha de comando usa).
    fn installed(minecraft: &str, release_time: &str) -> InstalledGame {
        InstalledGame {
            spec: GameSpec::new(minecraft, LoaderSpec::Vanilla).unwrap(),
            version_id: minecraft.into(),
            hierarchy: vec![minecraft.into()],
            release_time: release_time.into(),
            main_class: "net.minecraft.client.main.Main".into(),
            jvm_args: vec!["-cp".into(), "<cp>".into()],
            game_args: vec!["--gameDir".into(), "<game>".into()],
            game_dir_placeholder: "<game>".into(),
            logging: LoggingConfig::None,
        }
    }

    /// A linha de comando de um teste com o perfil, como o Testar monta (golden).
    fn command_for(
        profile: &ProfileFields,
        game: &InstalledGame,
        world: Option<&str>,
    ) -> Vec<String> {
        let memory = choose_memory(profile.memory, TestMemory::Auto, 10, Some(32_768));
        let mut options = launch_options(crate::test_session::PlanInput {
            game_dir: PathBuf::from("/i/minecraft"),
            state_dir: PathBuf::from("/i/state"),
            java: JavaRuntime {
                java: PathBuf::from("/j/bin/java"),
                launcher: PathBuf::from("/j/bin/java"),
                major: 17,
            },
            memory_mb: memory.mb,
            user_jvm_args: profile.jvm_args.clone(),
            player: OfflineProfile::from_name("Jogador").unwrap(),
        });
        apply_quick_play(&mut options, world);
        let mut command = build(game, &options, TargetOs::Linux).unwrap();
        append_window(&mut command, profile.window);
        command.args
    }

    /// CA-T11-04 (linha de comando): o perfil "PC fraco (4 GB)" abre com 4 GB.
    #[test]
    fn ca_t11_04_perfil_pc_fraco_abre_com_4_gb() {
        let profile = ProfileFields {
            window: TestWindow::Size {
                width: 1280,
                height: 720,
            },
            ..fixed(4)
        };
        let args = command_for(
            &profile,
            &installed("1.20.1", "2023-06-12T13:25:51+00:00"),
            None,
        );
        assert_eq!(args[0], "-Xmx4096M", "{args:?}");
        assert!(!args.iter().any(|arg| arg.starts_with("--quickPlay")));
        assert_eq!(
            args[args.len() - 4..],
            ["--width", "1280", "--height", "720"]
        );
    }

    /// CA-T11-05 (golden por faixa): na 1.20.1 o "Entrar direto no mundo de teste" vira
    /// `--quickPlaySingleplayer <pasta do mundo>`; na 1.19.2 a opção não existe (a interface a
    /// desabilita e o Testar abre no menu, sem argumento de Quick Play).
    #[test]
    fn ca_t11_05_quick_play_por_faixa() {
        let profile = ProfileFields {
            on_launch: OnLaunch::World { world: None },
            window: TestWindow::Fullscreen,
            ..ProfileFields::default()
        };
        let modern = installed("1.20.1", "2023-06-12T13:25:51+00:00");
        let args = command_for(&profile, &modern, Some("Mundo de teste"));
        let at = args
            .iter()
            .position(|arg| arg == "--quickPlaySingleplayer")
            .expect("--quickPlaySingleplayer");
        assert_eq!(args[at + 1], "Mundo de teste");
        assert!(!args.iter().any(|arg| arg == "--quickPlayPath"));
        assert_eq!(args.last().map(String::as_str), Some("--fullscreen"));

        assert!(!warden_launcher::quick_play::quick_play_support("1.19.2").is_supported());
        let old = installed("1.19.2", "2022-08-05T11:57:05+00:00");
        let args = command_for(&profile, &old, None);
        assert!(
            !args
                .iter()
                .any(|arg| arg.starts_with("--quickPlay") || arg == "--server")
        );
    }
}
