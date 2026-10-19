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
use crate::watcher::PackWatchers;

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
    /// Registro local dos packs (P1-07).
    pub packs: Arc<warden_project::registry::Registry>,
    /// Cliente do Modrinth com o cache de metadados (P1-08: inventário e detalhes).
    pub modrinth: warden_modrinth::ModrinthClient,
    /// O jogo do Testar, um por vez, e o motor do launcher (L-04).
    pub(crate) tests: Arc<crate::test_session::TestSessions>,
    /// Último relatório de atualizações de cada pack (P1-12; só memória).
    pub update_reports: crate::commands::updates::UpdateReports,
    /// Downloads manuais da CurseForge em espera, por pack (L-06; só memória).
    pub(crate) manual_downloads: Arc<crate::commands::cf_blocked::ManualDownloadsHub>,
    /// Vigias de mudanças externas dos packs abertos (A-05).
    pub watchers: PackWatchers,
    /// Índices da busca em todas as configs, por origem (C-05; só memória).
    pub(crate) config_search: Arc<crate::commands::configs_search::ConfigSearchHub>,
    /// Console agrupado das sessões abertas na tela (D-09; só memória).
    pub(crate) console_groups: crate::commands::console::ConsoleGroups,
    /// O servidor local (L-09): um por vez, com o evento `server-state`.
    pub(crate) servers: Arc<crate::server_session::ServerSessions>,
    /// ✦ Diagnóstico com IA: prévias do consentimento, conversas respondendo e modelos (D-04;
    /// só memória).
    pub(crate) ai: crate::commands::ai::AiHub,
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
        secret_testers.register(
            warden_secrets::SecretKind::Github,
            Arc::new(crate::commands::github::GithubTokenTester),
        );
        secret_testers.register(
            warden_secrets::SecretKind::Gemini,
            Arc::new(crate::commands::ai::GeminiKeyTester::default()),
        );
        secret_testers.register(
            warden_secrets::SecretKind::Openai,
            Arc::new(AiKeyTester::new(warden_ai::providers::AiProvider::Openai)),
        );
        secret_testers.register(
            warden_secrets::SecretKind::Anthropic,
            Arc::new(AiKeyTester::new(
                warden_ai::providers::AiProvider::Anthropic,
            )),
        );
        let catalog = crate::commands::catalog::open_catalog(&paths)?;
        let java = open_java(&paths)?;
        let packs = warden_project::registry::Registry::open(paths.packs_registry_file())
            .map_err(|error| AppError::from_domain(&error))?;
        let packs = Arc::new(packs);
        // A tabela de Java de Configurações lista os packs registrados (handoff da L-01).
        let java_packs = JavaPackSources::default();
        java_packs.register(Arc::new(crate::commands::packs::RegistryJavaPacks(
            Arc::clone(&packs),
        )));
        let modrinth = crate::commands::inventory::open_modrinth(&paths)?;
        let tests = Arc::new(crate::test_session::TestSessions::new(&paths, &catalog));
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
                java_packs,
                packs,
                modrinth,
                tests,
                update_reports: crate::commands::updates::UpdateReports::default(),
                manual_downloads: Arc::default(),
                watchers: PackWatchers::new(),
                config_search: Arc::default(),
                console_groups: crate::commands::console::ConsoleGroups::default(),
                servers: Arc::default(),
                ai: crate::commands::ai::AiHub::default(),
            },
            outcome,
        ))
    }
}

/// Configurações › Testar da chave da OpenAI e da Anthropic (D-16 e D-17): pede a lista de
/// modelos com a chave e aplica a regra do provedor. Chave recusada vira `KEY_INVALID`; uma chave
/// sem nenhum modelo da regra, `PROVIDER_NO_MODEL`.
struct AiKeyTester {
    provider: warden_ai::providers::AiProvider,
    /// Endereço da API (`None`: o oficial, ou o de `WARDEN_API_BASE_*` em debug).
    base_url: Option<String>,
}

impl AiKeyTester {
    fn new(provider: warden_ai::providers::AiProvider) -> Self {
        Self {
            provider,
            base_url: None,
        }
    }
}

#[async_trait::async_trait]
impl crate::commands::secrets::SecretTester for AiKeyTester {
    async fn test(&self, value: secrecy::SecretString) -> Result<(), AppError> {
        use warden_ai::providers;
        let http = warden_http::HttpClient::new(providers::http_config(
            warden_http::HttpConfig::for_version(env!("CARGO_PKG_VERSION")),
        ))
        .map_err(|error| AppError::from_domain(&error))?;
        let client = match &self.base_url {
            Some(base) => providers::connect_with_base_url(self.provider, http, base, value),
            None => providers::connect(self.provider, http, value),
        }
        .map_err(|error| AppError::from_domain(&error))?;
        client
            .models()
            .await
            .map(|_| ())
            .map_err(|error| AppError::from_domain(&error))
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
        let testers = format!("{:?}", state.secret_testers);
        assert!(
            testers.contains("Openai") && testers.contains("Anthropic"),
            "{testers}"
        );
    }

    #[tokio::test]
    async fn testador_da_chave_da_openai_e_da_anthropic() {
        use crate::commands::secrets::SecretTester as _;
        use secrecy::SecretString;
        use warden_ai::providers::AiProvider;
        use wiremock::matchers::{header, method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let code = |error: AppError| serde_json::to_value(error.code).unwrap()["code"].clone();
        let server = MockServer::start().await;
        let list = |ids: &[&str]| serde_json::json!({ "data": ids.iter().map(|id| serde_json::json!({ "id": id })).collect::<Vec<_>>() });
        Mock::given(method("GET"))
            .and(path("/v1/models"))
            .and(header("authorization", "Bearer chave-openai-boa"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(list(&["gpt-6-luna", "gpt-6-sol"])),
            )
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/v1/models"))
            .and(header("authorization", "Bearer chave-openai-sem-modelo"))
            .respond_with(ResponseTemplate::new(200).set_body_json(list(&["gpt-6.1-sol", "o3"])))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/v1/models"))
            .and(header("x-api-key", "chave-anthropic-boa"))
            .and(header("anthropic-version", "2023-06-01"))
            .respond_with(ResponseTemplate::new(200).set_body_json(list(&["claude-sonnet-5-5"])))
            .mount(&server)
            .await;
        Mock::given(method("GET")).respond_with(ResponseTemplate::new(401).set_body_json(
            serde_json::json!({ "error": { "type": "authentication_error", "code": "invalid_api_key", "message": "x" } }),
        ))
        .mount(&server)
        .await;

        let openai = AiKeyTester {
            provider: AiProvider::Openai,
            base_url: Some(server.uri()),
        };
        openai
            .test(SecretString::from("chave-openai-boa"))
            .await
            .unwrap();
        let error = openai
            .test(SecretString::from("chave-openai-sem-modelo"))
            .await
            .unwrap_err();
        assert_eq!(code(error), "PROVIDER_NO_MODEL");
        let error = openai.test(SecretString::from("outra")).await.unwrap_err();
        assert_eq!(code(error), "KEY_INVALID");

        let anthropic = AiKeyTester {
            provider: AiProvider::Anthropic,
            base_url: Some(server.uri()),
        };
        anthropic
            .test(SecretString::from("chave-anthropic-boa"))
            .await
            .unwrap();
        let error = anthropic
            .test(SecretString::from("outra"))
            .await
            .unwrap_err();
        assert_eq!(code(error), "KEY_INVALID");
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
