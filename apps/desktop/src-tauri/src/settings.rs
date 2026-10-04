//! Configurações do app: `settings.json` na pasta de configuração (ARCHITECTURE §13; T21).
//!
//! - Tem `schemaVersion`. Ao abrir, um arquivo de versão anterior passa pelas migrações em
//!   ordem e é gravado de novo; um de versão mais nova (gravado por um Warden mais novo) é lido
//!   como dá, e esta versão não o altera (`SETTINGS_NEWER_VERSION`).
//! - Leitura tolerante: campo com tipo errado volta ao padrão sem derrubar os outros; campo
//!   desconhecido é preservado. JSON inválido vai para `settings.json.corrompido-<data>` e o app
//!   abre com os padrões.
//! - Gravação atômica. O arquivo só passa a existir quando algo é gravado: a primeira execução
//!   (T01) é "não existe `settings.json`".
//! - Nunca contém segredo: as chaves ficam no cofre ou no `.env` (ADR-0025); aqui fica só o
//!   modo (`secretsBackend`), que muda por `secrets_backend_set`.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use warden_core::{CoreError, atomic_write};
use warden_secrets::BackendKind;

use crate::error::{AppError, AppErrorCode};

/// Versão atual do formato.
pub(crate) const SCHEMA_VERSION: u32 = 1;

/// Nível de detalhe dos registros (Configurações → Privacidade e registros).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum LogLevel {
    /// `info` para tudo.
    #[default]
    Normal,
    /// `debug` para o código do Warden.
    Detailed,
}

/// Memória padrão do teste (Configurações → Teste).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(tag = "mode", rename_all = "camelCase")]
pub enum TestMemory {
    /// O Warden escolhe pela quantidade de mods e pela memória do computador.
    #[default]
    Auto,
    /// Valor fixo, em MB.
    Fixed {
        /// Memória máxima do Java, em MB.
        mb: u32,
    },
}

/// Configurações do app.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// Versão do formato.
    pub schema_version: u32,
    /// Onde ficam as chaves (muda só por `secrets_backend_set`).
    pub secrets_backend: BackendKind,
    /// Pasta padrão dos packs; `null` = a pasta padrão do sistema (Documentos\Warden).
    /// Muda por um comando que abre o diálogo nativo (ARCHITECTURE §4.1), não por
    /// `settings_update`.
    pub packs_dir: Option<PathBuf>,
    /// Nome do jogador no perfil offline do teste.
    pub player_name: String,
    /// Nível de detalhe dos registros.
    pub log_level: LogLevel,
    /// Mostrar diferenças antes de salvar uma config.
    pub config_diff_before_save: bool,
    /// Mostrar versões beta e alfa dos mods.
    pub show_prerelease_versions: bool,
    /// Intervalo da verificação automática de atualizações, em horas (0 = desligada).
    pub update_check_interval_hours: u32,
    /// Memória padrão do teste.
    pub test_memory: TestMemory,
    /// Quando a EULA do Minecraft foi aceita (RFC 3339), para o servidor local (P1, T27).
    pub eula_accepted_at: Option<String>,
    /// Campos que esta versão não conhece (de uma versão mais nova): preservados ao gravar.
    #[serde(flatten)]
    #[specta(skip)]
    pub unknown: Map<String, Value>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            secrets_backend: BackendKind::Keyring,
            packs_dir: None,
            player_name: "Jogador".to_owned(),
            log_level: LogLevel::Normal,
            config_diff_before_save: true,
            show_prerelease_versions: false,
            update_check_interval_hours: 24,
            test_memory: TestMemory::Auto,
            eula_accepted_at: None,
            unknown: Map::new(),
        }
    }
}

/// Mudanças pedidas pela interface (`settings_update`). Campo ausente = não muda.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SettingsPatch {
    /// Nome do jogador: 3 a 16 caracteres, letras sem acento, números e `_` (regra do
    /// Minecraft).
    #[specta(optional)]
    pub player_name: Option<String>,
    /// Nível dos registros.
    #[specta(optional)]
    pub log_level: Option<LogLevel>,
    /// Diferenças antes de salvar.
    #[specta(optional)]
    pub config_diff_before_save: Option<bool>,
    /// Versões beta e alfa.
    #[specta(optional)]
    pub show_prerelease_versions: Option<bool>,
    /// Intervalo da verificação de atualizações (0 a 720 horas).
    #[specta(optional)]
    pub update_check_interval_hours: Option<u32>,
    /// Memória do teste (512 a 65536 MB quando fixa).
    #[specta(optional)]
    pub test_memory: Option<TestMemory>,
}

impl SettingsPatch {
    fn validate(&self) -> Result<(), AppError> {
        let invalid = |field: &str| {
            Err(AppError::new(AppErrorCode::SettingsInvalid).with_param("field", field))
        };
        if let Some(name) = &self.player_name {
            let valid_chars = name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
            if !(3..=16).contains(&name.len()) || !valid_chars {
                return invalid("playerName");
            }
        }
        if self
            .update_check_interval_hours
            .is_some_and(|hours| hours > 720)
        {
            return invalid("updateCheckIntervalHours");
        }
        if let Some(TestMemory::Fixed { mb }) = self.test_memory
            && !(512..=65_536).contains(&mb)
        {
            return invalid("testMemory");
        }
        Ok(())
    }

    fn apply(self, settings: &mut Settings) {
        if let Some(value) = self.player_name {
            settings.player_name = value;
        }
        if let Some(value) = self.log_level {
            settings.log_level = value;
        }
        if let Some(value) = self.config_diff_before_save {
            settings.config_diff_before_save = value;
        }
        if let Some(value) = self.show_prerelease_versions {
            settings.show_prerelease_versions = value;
        }
        if let Some(value) = self.update_check_interval_hours {
            settings.update_check_interval_hours = value;
        }
        if let Some(value) = self.test_memory {
            settings.test_memory = value;
        }
    }
}

/// Como o arquivo foi encontrado ao abrir.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LoadOutcome {
    /// Não existe (primeira execução).
    Missing,
    /// Lido na versão atual.
    Current,
    /// Lido e migrado da versão `from`; já gravado na atual.
    Migrated { from: u32 },
    /// Gravado por um Warden mais novo: lido, mas esta versão não grava.
    Newer { version: u32 },
    /// Ilegível: guardado em `backup`, e o app usa os padrões.
    Corrupt { backup: PathBuf },
}

struct State {
    settings: Settings,
    exists: bool,
    read_only: bool,
}

/// `settings.json` em memória, com gravação atômica.
pub struct SettingsStore {
    path: PathBuf,
    state: Mutex<State>,
}

impl std::fmt::Debug for SettingsStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SettingsStore")
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

impl SettingsStore {
    /// Abre `path` (que pode não existir), migrando se preciso.
    pub(crate) fn open(path: PathBuf) -> Result<(Self, LoadOutcome), AppError> {
        let (settings, outcome) = load(&path)?;
        let exists = !matches!(outcome, LoadOutcome::Missing | LoadOutcome::Corrupt { .. });
        let read_only = matches!(outcome, LoadOutcome::Newer { .. });
        let store = Self {
            path,
            state: Mutex::new(State {
                settings,
                exists,
                read_only,
            }),
        };
        if matches!(outcome, LoadOutcome::Migrated { .. }) {
            store.save(&store.lock().settings)?;
        }
        Ok((store, outcome))
    }

    /// Configurações atuais.
    pub fn get(&self) -> Settings {
        self.lock().settings.clone()
    }

    /// Se o `settings.json` existe (fim da primeira execução, T01).
    pub fn exists(&self) -> bool {
        self.lock().exists
    }

    /// Aplica as mudanças da interface e grava.
    pub fn update(&self, patch: SettingsPatch) -> Result<Settings, AppError> {
        patch.validate()?;
        self.modify(|settings| patch.apply(settings))
    }

    /// Grava o modo das chaves (usado no meio da troca de modo).
    pub fn set_secrets_backend(&self, backend: BackendKind) -> Result<(), AppError> {
        self.modify(|settings| settings.secrets_backend = backend)
            .map(|_| ())
    }

    fn modify(&self, change: impl FnOnce(&mut Settings)) -> Result<Settings, AppError> {
        let mut state = self.lock();
        if state.read_only {
            return Err(AppError::new(AppErrorCode::SettingsNewerVersion)
                .with_param("version", state.settings.schema_version.to_string()));
        }
        let mut next = state.settings.clone();
        change(&mut next);
        self.save(&next)?;
        state.settings = next;
        state.exists = true;
        Ok(state.settings.clone())
    }

    fn save(&self, settings: &Settings) -> Result<(), AppError> {
        let mut text = serde_json::to_string_pretty(settings)
            .map_err(|error| AppError::internal(format!("settings.json: {error}")))?;
        text.push('\n');
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| CoreError::io("criar a pasta", parent, error))?;
        }
        atomic_write(&self.path, text.as_bytes())?;
        Ok(())
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Migrações: a de índice `n` leva da versão `n` para `n + 1`. Só acréscimo, no fim.
const MIGRATIONS: [fn(&mut Map<String, Value>); 1] = [migrate_v0_to_v1];

/// Versão 0: arquivo sem `schemaVersion` (protótipos). Os nomes dos campos são os mesmos; só
/// passa a declarar a versão.
fn migrate_v0_to_v1(_settings: &mut Map<String, Value>) {}

fn load(path: &Path) -> Result<(Settings, LoadOutcome), AppError> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok((Settings::default(), LoadOutcome::Missing));
        }
        Err(error) => return Err(CoreError::io("ler", path, error).into()),
    };
    let Ok(Value::Object(mut map)) = serde_json::from_slice::<Value>(&bytes) else {
        let backup = backup_corrupt(path)?;
        tracing::warn!(backup = %backup.display(), "settings.json ilegível; usando os padrões");
        return Ok((Settings::default(), LoadOutcome::Corrupt { backup }));
    };

    let version = map
        .get("schemaVersion")
        .and_then(Value::as_u64)
        .map_or(0, |version| u32::try_from(version).unwrap_or(u32::MAX));
    let outcome = match version.cmp(&SCHEMA_VERSION) {
        std::cmp::Ordering::Greater => {
            tracing::warn!(
                version,
                "settings.json de uma versão mais nova do Warden; só leitura"
            );
            LoadOutcome::Newer { version }
        }
        std::cmp::Ordering::Less => {
            for migration in MIGRATIONS.iter().skip(version as usize) {
                migration(&mut map);
            }
            map.insert("schemaVersion".to_owned(), Value::from(SCHEMA_VERSION));
            tracing::info!(from = version, to = SCHEMA_VERSION, "settings.json migrado");
            LoadOutcome::Migrated { from: version }
        }
        std::cmp::Ordering::Equal => LoadOutcome::Current,
    };
    Ok((tolerant_settings(map), outcome))
}

/// Monta as configurações campo a campo: um campo com tipo errado volta ao padrão sem
/// derrubar os outros.
fn tolerant_settings(map: Map<String, Value>) -> Settings {
    let whole = Value::Object(map.clone());
    if let Ok(settings) = serde_json::from_value::<Settings>(whole) {
        return settings;
    }
    let Ok(Value::Object(mut merged)) = serde_json::to_value(Settings::default()) else {
        return Settings::default();
    };
    for (key, value) in map {
        let previous = merged.insert(key.clone(), value);
        if serde_json::from_value::<Settings>(Value::Object(merged.clone())).is_err() {
            tracing::warn!(field = %key, "settings.json: valor inválido ignorado");
            match previous {
                Some(previous) => merged.insert(key, previous),
                None => merged.remove(&key),
            };
        }
    }
    serde_json::from_value(Value::Object(merged)).unwrap_or_default()
}

fn backup_corrupt(path: &Path) -> Result<PathBuf, AppError> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs());
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(format!(".corrompido-{stamp}"));
    let backup = path.with_file_name(name);
    std::fs::rename(path, &backup).map_err(|error| CoreError::io("renomear", path, error))?;
    Ok(backup)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ErrorCode;

    fn store_in(dir: &Path) -> (SettingsStore, LoadOutcome) {
        SettingsStore::open(dir.join("config").join("settings.json")).unwrap()
    }

    #[test]
    fn sem_arquivo_usa_padroes_e_nao_cria_o_arquivo() {
        let dir = tempfile::tempdir().unwrap();
        let (store, outcome) = store_in(dir.path());
        assert_eq!(outcome, LoadOutcome::Missing);
        assert!(!store.exists());
        assert_eq!(store.get(), Settings::default());
        assert!(!dir.path().join("config/settings.json").exists());
    }

    #[test]
    fn atualizar_grava_e_reabrir_le() {
        let dir = tempfile::tempdir().unwrap();
        let (store, _) = store_in(dir.path());
        let updated = store
            .update(SettingsPatch {
                player_name: Some("Steve_01".into()),
                log_level: Some(LogLevel::Detailed),
                test_memory: Some(TestMemory::Fixed { mb: 6144 }),
                update_check_interval_hours: Some(0),
                ..SettingsPatch::default()
            })
            .unwrap();
        assert_eq!(updated.player_name, "Steve_01");
        assert!(store.exists());

        let text = std::fs::read_to_string(dir.path().join("config/settings.json")).unwrap();
        assert!(text.contains("\"schemaVersion\": 1"), "{text}");
        assert!(text.ends_with('\n'));
        assert!(!text.contains('\r'));

        let (reopened, outcome) = store_in(dir.path());
        assert_eq!(outcome, LoadOutcome::Current);
        assert_eq!(reopened.get(), updated);
        assert_eq!(reopened.get().test_memory, TestMemory::Fixed { mb: 6144 });
    }

    #[test]
    fn valores_invalidos_sao_recusados_sem_gravar() {
        let dir = tempfile::tempdir().unwrap();
        let (store, _) = store_in(dir.path());
        for (patch, field) in [
            (
                SettingsPatch {
                    player_name: Some("ab".into()),
                    ..SettingsPatch::default()
                },
                "playerName",
            ),
            (
                SettingsPatch {
                    player_name: Some("João".into()),
                    ..SettingsPatch::default()
                },
                "playerName",
            ),
            (
                SettingsPatch {
                    player_name: Some("a".repeat(17)),
                    ..SettingsPatch::default()
                },
                "playerName",
            ),
            (
                SettingsPatch {
                    update_check_interval_hours: Some(721),
                    ..SettingsPatch::default()
                },
                "updateCheckIntervalHours",
            ),
            (
                SettingsPatch {
                    test_memory: Some(TestMemory::Fixed { mb: 256 }),
                    ..SettingsPatch::default()
                },
                "testMemory",
            ),
        ] {
            let error = store.update(patch).unwrap_err();
            assert_eq!(error.code, ErrorCode::App(AppErrorCode::SettingsInvalid));
            assert_eq!(error.params["field"], field);
        }
        assert!(!store.exists());
    }

    #[test]
    fn patch_recusa_campos_desconhecidos_e_o_modo_das_chaves() {
        assert!(serde_json::from_str::<SettingsPatch>(r#"{"secretsBackend":"envfile"}"#).is_err());
        assert!(serde_json::from_str::<SettingsPatch>(r#"{"packsDir":"C:/x"}"#).is_err());
        let patch: SettingsPatch = serde_json::from_str(r#"{"playerName":"Alex"}"#).unwrap();
        assert_eq!(patch.player_name.as_deref(), Some("Alex"));
    }

    #[test]
    fn migra_arquivo_sem_versao_e_grava() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config/settings.json");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, r#"{"playerName":"Antigo","logLevel":"detailed"}"#).unwrap();
        let (store, outcome) = SettingsStore::open(path.clone()).unwrap();
        assert_eq!(outcome, LoadOutcome::Migrated { from: 0 });
        assert_eq!(store.get().player_name, "Antigo");
        assert_eq!(store.get().log_level, LogLevel::Detailed);
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("\"schemaVersion\": 1"), "{text}");
    }

    #[test]
    fn campo_com_tipo_errado_volta_ao_padrao_e_desconhecido_e_preservado() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config/settings.json");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(
            &path,
            r#"{"schemaVersion":1,"playerName":42,"logLevel":"detailed","campoNovo":{"a":1}}"#,
        )
        .unwrap();
        let (store, _) = SettingsStore::open(path.clone()).unwrap();
        let settings = store.get();
        assert_eq!(settings.player_name, "Jogador");
        assert_eq!(settings.log_level, LogLevel::Detailed);
        store
            .update(SettingsPatch {
                show_prerelease_versions: Some(true),
                ..SettingsPatch::default()
            })
            .unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("\"campoNovo\""), "{text}");
    }

    #[test]
    fn arquivo_de_versao_mais_nova_nao_e_alterado() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config/settings.json");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let original = r#"{"schemaVersion":99,"playerName":"Futuro"}"#;
        std::fs::write(&path, original).unwrap();
        let (store, outcome) = SettingsStore::open(path.clone()).unwrap();
        assert_eq!(outcome, LoadOutcome::Newer { version: 99 });
        assert_eq!(store.get().player_name, "Futuro");
        let error = store.update(SettingsPatch::default()).unwrap_err();
        assert_eq!(
            error.code,
            ErrorCode::App(AppErrorCode::SettingsNewerVersion)
        );
        assert!(store.set_secrets_backend(BackendKind::Envfile).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
    }

    #[test]
    fn json_invalido_vira_copia_e_padroes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config/settings.json");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        for broken in ["{ quebrado", "[1, 2]", ""] {
            std::fs::write(&path, broken).unwrap();
            let (store, outcome) = SettingsStore::open(path.clone()).unwrap();
            let LoadOutcome::Corrupt { backup } = outcome else {
                panic!("esperava Corrupt para {broken:?}");
            };
            assert_eq!(std::fs::read_to_string(&backup).unwrap(), broken);
            assert!(!path.exists());
            assert!(!store.exists());
            assert_eq!(store.get(), Settings::default());
            std::fs::remove_file(backup).unwrap();
        }
    }

    #[test]
    fn modo_das_chaves_e_gravado() {
        let dir = tempfile::tempdir().unwrap();
        let (store, _) = store_in(dir.path());
        store.set_secrets_backend(BackendKind::Envfile).unwrap();
        let (reopened, _) = store_in(dir.path());
        assert_eq!(reopened.get().secrets_backend, BackendKind::Envfile);
    }

    #[test]
    fn falha_ao_gravar_nao_muda_o_estado_em_memoria() {
        let dir = tempfile::tempdir().unwrap();
        let (store, _) = store_in(dir.path());
        let _guard = warden_core::fault::arm("atomic_write.before_rename");
        let error = store
            .update(SettingsPatch {
                player_name: Some("Novo".into()),
                ..SettingsPatch::default()
            })
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::Core(warden_core::CoreErrorCode::Io));
        assert_eq!(store.get().player_name, "Jogador");
        assert!(!store.exists());
    }

    #[test]
    fn json_dos_padroes() {
        let json = serde_json::to_value(Settings::default()).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "schemaVersion": 1,
                "secretsBackend": "keyring",
                "packsDir": null,
                "playerName": "Jogador",
                "logLevel": "normal",
                "configDiffBeforeSave": true,
                "showPrereleaseVersions": false,
                "updateCheckIntervalHours": 24,
                "testMemory": { "mode": "auto" },
                "eulaAcceptedAt": null,
            })
        );
    }
}
