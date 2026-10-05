//! Estado do app, guardado pelo Tauri (`app.manage`) e recebido pelos comandos como
//! `State<'_, AppState>` (ARCHITECTURE §15; QUALITY §11: o único estado global mutável).
//!
//! Registro acréscimo-apenas (ROADMAP §1): cada tarefa acrescenta o seu campo no fim da
//! struct e a sua linha em [`AppState::open`].

use std::sync::Arc;

use warden_core::{AppPaths, remove_temp_files};
use warden_secrets::{BackendOverride, Secrets, SecretsConfig};

use crate::commands::java::JavaPackSources;
use crate::commands::secrets::SecretTesters;
use crate::error::AppError;
use crate::locks::PackLocks;
use crate::logging::Logging;
use crate::operations::{Notifier, OperationRegistry};
use crate::settings::{LoadOutcome, SettingsStore};

/// Estado do app.
#[derive(Debug)]
pub struct AppState {
    /// Pastas do app.
    pub paths: AppPaths,
    /// `settings.json`.
    pub settings: Arc<SettingsStore>,
    /// Chaves e tokens.
    pub secrets: Arc<Secrets>,
    /// Quem sabe testar cada chave (ligado pelas tarefas das APIs).
    pub secret_testers: SecretTesters,
    /// Operações longas.
    pub operations: OperationRegistry,
    /// Travas por pack.
    pub locks: PackLocks,
    /// Registros (ausente nos testes).
    pub logging: Option<Logging>,
    /// Catálogo de versões do Minecraft e dos loaders (P1-05).
    pub catalog: warden_catalog::Catalog,
    /// Javas do teste (L-01).
    pub java: warden_java::JavaRuntimes,
    /// Quem lista os packs para a tabela de Java (registrado pela P1-07).
    pub java_packs: JavaPackSources,
}

impl AppState {
    /// Abre o estado: cria as pastas básicas, apaga temporários de quedas anteriores, lê as
    /// configurações e abre as chaves no modo gravado.
    pub(crate) fn open(
        paths: AppPaths,
        keyring_override: Option<BackendOverride>,
        notifier: Notifier,
        logging: Option<Logging>,
    ) -> Result<(Self, LoadOutcome), AppError> {
        paths.create_base_dirs()?;
        let removed = remove_temp_files(paths.config_dir())?;
        if removed > 0 {
            tracing::info!(removed, "temporários de uma gravação interrompida apagados");
        }
        let (settings, outcome) = SettingsStore::open(paths.settings_file())?;
        if let Some(logging) = &logging {
            logging.set_level(settings.get().log_level);
        }
        let secrets = Secrets::open(
            SecretsConfig::for_app(paths.secrets_env_file(), keyring_override),
            settings.get().secrets_backend,
        );
        let secret_testers = SecretTesters::default();
        secret_testers.register(
            warden_secrets::SecretKind::Curseforge,
            Arc::new(crate::commands::curseforge::CurseforgeKeyTester::default()),
        );
        let catalog = crate::commands::catalog::open_catalog(&paths)?;
        let java = open_java(&paths)?;
        Ok((
            Self {
                paths,
                settings: Arc::new(settings),
                secrets: Arc::new(secrets),
                secret_testers,
                operations: OperationRegistry::new(notifier),
                locks: PackLocks::new(),
                logging,
                catalog,
                java,
                java_packs: JavaPackSources::default(),
            },
            outcome,
        ))
    }
}

/// Serviço de Java com as fontes oficiais. Apaga sobras de instalações interrompidas e Javas
/// substituídos sem uso; uma falha nessa limpeza não impede o app de abrir.
fn open_java(paths: &AppPaths) -> Result<warden_java::JavaRuntimes, AppError> {
    let http = warden_http::HttpClient::new(warden_http::HttpConfig::for_version(env!(
        "CARGO_PKG_VERSION"
    )))
    .map_err(|error| AppError::from_domain(&error))?;
    let java = warden_java::JavaRuntimes::new(
        warden_java::JavaRuntimesConfig::for_app(paths)?,
        http,
        Arc::new(warden_java::ProcessProbe::default()),
    )?;
    match java.startup_cleanup() {
        Ok(0) => {}
        Ok(removed) => tracing::info!(removed, "sobras de Javas apagadas"),
        Err(error) => tracing::warn!(%error, "limpeza dos Javas não terminou"),
    }
    Ok(java)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::path::Path;
    use warden_secrets::BackendKind;

    /// Estado de teste numa pasta temporária, com o cofre de teste em arquivo.
    pub(crate) fn test_state(root: &Path) -> AppState {
        let paths = AppPaths::from_dev_root(root).unwrap();
        let vault = BackendOverride::TestFile(root.join("cofre-de-teste"));
        AppState::open(paths, Some(vault), Arc::new(|_| {}), None)
            .unwrap()
            .0
    }

    #[test]
    fn abre_numa_pasta_vazia_sem_criar_settings() {
        let dir = tempfile::tempdir().unwrap();
        let state = test_state(dir.path());
        assert!(state.paths.logs_dir().is_dir());
        assert!(!state.settings.exists());
        assert_eq!(state.secrets.backend(), BackendKind::Keyring);
        assert!(format!("{:?}", state.secret_testers).contains("Curseforge"));
    }

    #[test]
    fn apaga_temporarios_e_abre_no_modo_gravado() {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("config");
        std::fs::create_dir_all(&config).unwrap();
        std::fs::write(config.join("settings.json.1-0.warden-tmp"), b"{").unwrap();
        std::fs::write(
            config.join("settings.json"),
            r#"{"schemaVersion":1,"secretsBackend":"envfile"}"#,
        )
        .unwrap();
        let state = test_state(dir.path());
        assert!(!config.join("settings.json.1-0.warden-tmp").exists());
        assert_eq!(state.secrets.backend(), BackendKind::Envfile);
    }
}
