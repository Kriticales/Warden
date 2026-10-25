//! Comandos do domínio `secrets` (ARCHITECTURE §4.1 e §14; T21 → Chaves e contas).
//!
//! Não existe comando para ler um segredo: a interface só conhece o estado
//! ([`SecretsStatus`]). O valor digitado chega em `secrets_set`, vira `SecretString` na hora e
//! nunca é registrado nem devolvido.
//!
//! `secrets_test` usa o testador registrado para cada chave em [`SecretTesters`]; as tarefas
//! das APIs (CurseForge, Gemini, GitHub) registram o seu. Sem testador, o resultado é
//! [`SecretTestResult::NotTestable`]. O resultado (funcionou ou o serviço recusou) fica em
//! `key-checks.json` ([`crate::key_checks`]).
//!
//! As chaves ficam sempre no cofre do sistema (ADR-0069). `secrets_overview` dá à tela de
//! Configurações o estado de cada uma (com o final `••••xxxx` e o último teste) e o aviso da
//! migração do `.env` antigo, feita ao abrir o app ([`run_env_migration`]);
//! `secrets_migration_retry` tenta a migração de novo quando ela não terminou.

use std::collections::HashMap;
use std::sync::{Arc, PoisonError, RwLock};

use serde::Serialize;
use tauri::State;
use warden_secrets::{SecretKind, SecretString, Secrets, SecretsStatus};

use crate::error::AppError;
use crate::key_checks::{KeyCheck, KeyCheckOutcome, KeyChecks, is_refusal};
use crate::settings::SettingsStore;
use crate::state::AppState;

/// Quem sabe conferir uma chave na API dela.
#[async_trait::async_trait]
pub trait SecretTester: Send + Sync {
    /// Confere a chave. Chave recusada é erro do domínio da API (ex.: a CurseForge recusou).
    async fn test(&self, value: SecretString) -> Result<(), AppError>;
}

/// Testadores registrados, por chave.
#[derive(Default)]
pub struct SecretTesters {
    testers: RwLock<HashMap<SecretKind, Arc<dyn SecretTester>>>,
}

impl std::fmt::Debug for SecretTesters {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let testers = self.testers.read().unwrap_or_else(PoisonError::into_inner);
        f.debug_struct("SecretTesters")
            .field("kinds", &testers.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl SecretTesters {
    /// Registra (ou troca) o testador de uma chave.
    pub fn register(&self, kind: SecretKind, tester: Arc<dyn SecretTester>) {
        self.testers
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(kind, tester);
    }

    fn get(&self, kind: SecretKind) -> Option<Arc<dyn SecretTester>> {
        self.testers
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&kind)
            .cloned()
    }
}

/// Resultado de "Testar".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum SecretTestResult {
    /// A API aceitou a chave.
    Valid,
    /// Ainda não há como testar esta chave nesta versão.
    NotTestable,
}

/// Roda trabalho bloqueante (cofre do sistema, disco) fora da thread do IPC.
async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, AppError> + Send + 'static,
) -> Result<T, AppError> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|error| AppError::internal(format!("tarefa de segredos interrompida: {error}")))?
}

/// O que a migração do `.env` antigo para o cofre fez ao abrir o app (ADR-0069).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub(crate) enum EnvMigrationNotice {
    /// As chaves foram para o cofre, conferidas uma a uma.
    #[serde(rename_all = "camelCase")]
    Moved {
        /// As chaves levadas.
        moved: Vec<SecretKind>,
        /// O `.env` foi apagado (`false`: sobrou e o Warden tenta apagar na próxima abertura).
        file_removed: bool,
    },
    /// Não terminou: o `.env` continua onde estava e nenhuma chave se perdeu.
    Failed {
        /// Por quê.
        error: AppError,
    },
}

/// Uma chave em Configurações → Chaves e contas. Nunca leva o valor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct KeyOverview {
    /// Qual chave.
    pub(crate) kind: SecretKind,
    /// Está no cofre.
    pub(crate) configured: bool,
    /// Os últimos 4 caracteres, para `••••xxxx` (`null` sem chave ou com chave curta).
    pub(crate) hint: Option<String>,
    /// O último **Testar** desta chave.
    pub(crate) check: Option<KeyCheck>,
}

/// As cinco chaves e o aviso da migração do `.env`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct KeysOverview {
    /// Na ordem de [`SecretKind::ALL`].
    pub(crate) keys: Vec<KeyOverview>,
    /// O que a migração do `.env` fez nesta abertura do app, se fez algo.
    pub(crate) migration: Option<EnvMigrationNotice>,
}

/// Quais chaves estão configuradas.
#[tauri::command]
#[specta::specta]
pub(crate) async fn secrets_status(state: State<'_, AppState>) -> Result<SecretsStatus, AppError> {
    let secrets = Arc::clone(&state.secrets);
    blocking(move || Ok(secrets.status()?)).await
}

/// Cada chave com o final e o último teste, e o aviso da migração do `.env`.
#[tauri::command]
#[specta::specta]
pub(crate) async fn secrets_overview(
    state: State<'_, AppState>,
) -> Result<KeysOverview, AppError> {
    let secrets = Arc::clone(&state.secrets);
    let keys = blocking(move || key_rows(&secrets)).await?;
    Ok(overview_of(&state, keys))
}

/// Tenta de novo levar as chaves do `.env` antigo para o cofre.
#[tauri::command]
#[specta::specta]
pub(crate) async fn secrets_migration_retry(
    state: State<'_, AppState>,
) -> Result<KeysOverview, AppError> {
    let secrets = Arc::clone(&state.secrets);
    let settings = Arc::clone(&state.settings);
    let (notice, keys) = blocking(move || {
        let notice = run_env_migration(&secrets, &settings);
        Ok((notice, key_rows(&secrets)?))
    })
    .await?;
    *lock_migration(&state) = notice;
    Ok(overview_of(&state, keys))
}

/// Grava uma chave digitada e devolve o estado.
#[tauri::command]
#[specta::specta]
pub(crate) async fn secrets_set(
    state: State<'_, AppState>,
    kind: SecretKind,
    value: String,
) -> Result<SecretsStatus, AppError> {
    let value = SecretString::from(value);
    let secrets = Arc::clone(&state.secrets);
    let status = blocking(move || set_impl(&secrets, kind, &value)).await?;
    forget_check(&state.key_checks, kind);
    Ok(status)
}

/// Apaga uma chave e devolve o estado.
#[tauri::command]
#[specta::specta]
pub(crate) async fn secrets_remove(
    state: State<'_, AppState>,
    kind: SecretKind,
) -> Result<SecretsStatus, AppError> {
    let secrets = Arc::clone(&state.secrets);
    let status = blocking(move || remove_impl(&secrets, kind)).await?;
    forget_check(&state.key_checks, kind);
    Ok(status)
}

/// Testa uma chave na API dela e guarda o resultado (funcionou ou recusada).
#[tauri::command]
#[specta::specta]
pub(crate) async fn secrets_test(
    state: State<'_, AppState>,
    kind: SecretKind,
) -> Result<SecretTestResult, AppError> {
    let secrets = Arc::clone(&state.secrets);
    let value = blocking(move || Ok(secrets.require(kind)?)).await?;
    let result = test_impl(state.secret_testers.get(kind), kind, value).await;
    record_check(&state.key_checks, kind, &result);
    result
}

/// Leva as chaves do `.env` antigo para o cofre (ADR-0069). Com o `settings.json` ainda no
/// modo `.env`, o arquivo vale; depois de tudo conferido, o campo antigo sai do
/// `settings.json`. Devolve o aviso para a tela (`None`: nada a dizer).
pub(crate) fn run_env_migration(
    secrets: &Secrets,
    settings: &SettingsStore,
) -> Option<EnvMigrationNotice> {
    match secrets.migrate_legacy_env(settings.legacy_env_mode()) {
        Ok(report) => {
            if let Err(error) = settings.forget_secrets_backend() {
                tracing::warn!(%error, "settings.json: modo antigo das chaves não apagado");
            }
            let report = report?;
            (!report.moved.is_empty() || !report.file_removed).then_some(
                EnvMigrationNotice::Moved {
                    moved: report.moved,
                    file_removed: report.file_removed,
                },
            )
        }
        Err(error) => {
            tracing::error!(%error, "chaves do .env antigo não levadas para o cofre");
            Some(EnvMigrationNotice::Failed {
                error: AppError::from(error),
            })
        }
    }
}

fn key_rows(secrets: &Secrets) -> Result<Vec<(SecretKind, bool, Option<String>)>, AppError> {
    let status = secrets.status()?;
    SecretKind::ALL
        .iter()
        .map(|&kind| Ok((kind, status.is_configured(kind), secrets.hint(kind)?)))
        .collect()
}

fn overview_of(state: &AppState, keys: Vec<(SecretKind, bool, Option<String>)>) -> KeysOverview {
    KeysOverview {
        keys: keys
            .into_iter()
            .map(|(kind, configured, hint)| KeyOverview {
                kind,
                configured,
                hint,
                check: configured.then(|| state.key_checks.get(kind)).flatten(),
            })
            .collect(),
        migration: lock_migration(state).clone(),
    }
}

fn lock_migration(state: &AppState) -> std::sync::MutexGuard<'_, Option<EnvMigrationNotice>> {
    state
        .env_migration
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
}

/// Guarda o resultado de um **Testar**: só a resposta do serviço conta (sem internet, nada muda).
fn record_check(
    checks: &KeyChecks,
    kind: SecretKind,
    result: &Result<SecretTestResult, AppError>,
) {
    let outcome = match result {
        Ok(SecretTestResult::Valid) => KeyCheckOutcome::Working,
        Err(error) if is_refusal(error.code) => KeyCheckOutcome::Refused,
        _ => return,
    };
    if let Err(error) = checks.record(kind, outcome) {
        tracing::warn!(?kind, %error, "resultado do teste da chave não gravado");
    }
}

fn forget_check(checks: &KeyChecks, kind: SecretKind) {
    if let Err(error) = checks.clear(kind) {
        tracing::warn!(?kind, %error, "resultado do teste da chave não apagado");
    }
}

fn set_impl(
    secrets: &Secrets,
    kind: SecretKind,
    value: &SecretString,
) -> Result<SecretsStatus, AppError> {
    secrets.set(kind, value)?;
    tracing::info!(?kind, "chave gravada");
    Ok(secrets.status()?)
}

fn remove_impl(secrets: &Secrets, kind: SecretKind) -> Result<SecretsStatus, AppError> {
    secrets.remove(kind)?;
    tracing::info!(?kind, "chave removida");
    Ok(secrets.status()?)
}

async fn test_impl(
    tester: Option<Arc<dyn SecretTester>>,
    kind: SecretKind,
    value: SecretString,
) -> Result<SecretTestResult, AppError> {
    let Some(tester) = tester else {
        return Ok(SecretTestResult::NotTestable);
    };
    tester.test(value).await?;
    tracing::info!(?kind, "chave conferida na API");
    Ok(SecretTestResult::Valid)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ErrorCode;
    use crate::logging::with_file_logging;
    use crate::settings::LogLevel;
    use crate::state::tests::test_state;
    use std::io::ErrorKind;
    use std::path::{Path, PathBuf};
    use std::time::{Duration, Instant};
    use warden_secrets::{ExposeSecret as _, SecretsErrorCode};

    const CF: &str = "$2a$10$valorDeVarreduraDaCurseforge0000000";
    const GEMINI: &str = "AIzaValorDeVarreduraDoGemini000000";
    const GITHUB: &str = "ghp_valorDeVarreduraDoGithub000000";
    const OPENAI: &str = "sk-proj-valorDeVarreduraDaOpenai000000";
    const ANTHROPIC: &str = "sk-ant-api03-valorDeVarreduraDaAnthropic0";

    fn secret(text: &str) -> SecretString {
        SecretString::from(text.to_owned())
    }

    /// Arquivo lido pela varredura: caminho e conteúdo.
    type Scanned = Vec<(PathBuf, Vec<u8>)>;

    /// Prazo para a pasta parar de mudar por ação de outro processo.
    const SCAN_DEADLINE: Duration = Duration::from_secs(10);

    /// Lê todos os arquivos abaixo de `dir`, exceto os de `skip`.
    ///
    /// No Windows, outro processo pode criar e apagar arquivos na pasta por alguns
    /// milissegundos depois que a operação terminou: com o Kaspersky ativo, apagar o `.env`
    /// faz surgir um `config/.ENV.tmp` vazio, aberto com acesso exclusivo, que some em seguida
    /// (1 em cada 5 execuções, em média). Se um arquivo listado some antes da leitura, ou se
    /// outro processo o segura, a varredura recomeça do zero depois de uma pausa curta, até o
    /// prazo. Nada é ignorado: o resultado é uma leitura completa de todo arquivo que existia
    /// no fim.
    fn read_all_under(dir: &Path, skip: &[&Path]) -> Scanned {
        let deadline = Instant::now() + SCAN_DEADLINE;
        loop {
            match try_read_all_under(dir, skip) {
                Ok(scanned) => return scanned,
                Err((_, error)) if is_transient(&error) && Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err((path, error)) => panic!("varredura: {}: {error}", path.display()),
            }
        }
    }

    /// Uma passada da varredura; o primeiro erro interrompe e diz em qual caminho.
    fn try_read_all_under(
        dir: &Path,
        skip: &[&Path],
    ) -> Result<Scanned, (PathBuf, std::io::Error)> {
        let mut scanned = Vec::new();
        let mut pending = vec![dir.to_path_buf()];
        while let Some(current) = pending.pop() {
            if skip.iter().any(|skipped| current.starts_with(skipped)) {
                continue;
            }
            let entries = match std::fs::read_dir(&current) {
                Ok(entries) => entries,
                // A pasta-raiz pode não existir (ex.: o cofre já esvaziado e apagado).
                Err(error) if error.kind() == ErrorKind::NotFound && current == dir => {
                    return Ok(scanned);
                }
                Err(error) => return Err((current, error)),
            };
            for entry in entries {
                let entry = entry.map_err(|error| (current.clone(), error))?;
                let path = entry.path();
                let kind = entry.file_type().map_err(|error| (path.clone(), error))?;
                if kind.is_dir() {
                    pending.push(path);
                } else if !skip.iter().any(|skipped| path.starts_with(skipped)) {
                    let bytes = std::fs::read(&path).map_err(|error| (path.clone(), error))?;
                    scanned.push((path, bytes));
                }
            }
        }
        Ok(scanned)
    }

    /// Erros de um arquivo que outro processo está criando, segurando ou apagando: sumiu
    /// (`NotFound`), acesso negado enquanto a exclusão está pendente (5) e compartilhamento
    /// ou trava em uso (32 e 33, `ERROR_SHARING_VIOLATION` e `ERROR_LOCK_VIOLATION`).
    fn is_transient(error: &std::io::Error) -> bool {
        error.kind() == ErrorKind::NotFound
            || (cfg!(windows) && matches!(error.raw_os_error(), Some(5 | 32 | 33)))
    }

    /// Nenhum valor nos arquivos lidos e nenhum temporário da escrita atômica sobrando.
    fn assert_no_secret_in(scanned: &Scanned) {
        for (file, bytes) in scanned {
            assert!(
                !file.to_string_lossy().ends_with(warden_core::TEMP_SUFFIX),
                "temporário sobrando: {}",
                file.display()
            );
            let text = String::from_utf8_lossy(bytes);
            for value in [CF, GEMINI, OPENAI, ANTHROPIC, GITHUB] {
                assert!(
                    !text.contains(value),
                    "segredo encontrado em {}",
                    file.display()
                );
            }
        }
    }

    /// Critério 3 da F0-05, CA-T21-01 e CA-T21-07 (parte de backend; ADR-0069), com varredura:
    /// gravar e reabrir; abrir o app no antigo modo `.env` (com uma falha injetada na primeira
    /// tentativa) leva as chaves para o cofre e apaga o arquivo; em nenhum momento um valor
    /// aparece em `settings.json`, em `key-checks.json`, nos registros (nível detalhado) ou fora
    /// do cofre, a não ser no próprio `.env` antigo enquanto a migração não terminou.
    #[test]
    fn f0_05_ca3_ca_t21_01_ca_t21_07_varredura_de_segredos() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let logs = root.join("data").join("logs");
        std::fs::create_dir_all(&logs).unwrap();
        let vault = root.join("cofre-de-teste");
        let env_file = root.join("config").join(".env");

        with_file_logging(&logs, LogLevel::Detailed, || {
            // Gravar no cofre e "reiniciar".
            let state = test_state(root);
            set_impl(&state.secrets, SecretKind::Curseforge, &secret(CF)).unwrap();
            set_impl(&state.secrets, SecretKind::Gemini, &secret(GEMINI)).unwrap();
            set_impl(&state.secrets, SecretKind::Openai, &secret(OPENAI)).unwrap();
            set_impl(&state.secrets, SecretKind::Anthropic, &secret(ANTHROPIC)).unwrap();
            let status = set_impl(&state.secrets, SecretKind::Github, &secret(GITHUB)).unwrap();
            assert!(status.curseforge && status.gemini && status.github);
            assert!(status.openai && status.anthropic);
            record_check(
                &state.key_checks,
                SecretKind::Github,
                &Ok(SecretTestResult::Valid),
            );
            drop(state);
            let state = test_state(root);
            let status = state.secrets.status().unwrap();
            assert!(status.curseforge && status.gemini && status.github);
            assert_no_secret_in(&read_all_under(root, &[&vault]));
            for kind in SecretKind::ALL {
                remove_impl(&state.secrets, kind).unwrap();
            }
            drop(state);

            // O antigo modo `.env`: a primeira tentativa falha no meio e não apaga nada.
            std::fs::write(
                root.join("config/settings.json"),
                r#"{"schemaVersion":1,"secretsBackend":"envfile"}"#,
            )
            .unwrap();
            std::fs::write(
                &env_file,
                format!(
                    "CURSEFORGE_API_KEY='{CF}'\nGEMINI_API_KEY='{GEMINI}'\nOPENAI_API_KEY='{OPENAI}'\nANTHROPIC_API_KEY='{ANTHROPIC}'\nGITHUB_TOKEN='{GITHUB}'\n"
                ),
            )
            .unwrap();
            {
                let _guard = warden_core::fault::arm_after("secrets.migrate.after_copy", 2);
                let state = test_state(root);
                let notice = state.env_migration.lock().unwrap().clone();
                let Some(EnvMigrationNotice::Failed { error }) = notice else {
                    panic!("esperava a migração falhar: {notice:?}");
                };
                assert_eq!(
                    error.code,
                    ErrorCode::Secrets(SecretsErrorCode::EnvMigrationFailed)
                );
                assert!(env_file.exists(), "o .env não pode sumir numa falha");
                let settings =
                    std::fs::read_to_string(root.join("config/settings.json")).unwrap();
                assert!(settings.contains("envfile"), "{settings}");
            }

            // Reabrir termina a migração.
            let state = test_state(root);
            assert!(!env_file.exists(), "o .env deveria ter sido apagado");
            let status = state.secrets.status().unwrap();
            assert!(status.curseforge && status.gemini && status.github);
            assert!(status.openai && status.anthropic);
            assert_eq!(
                state
                    .secrets
                    .get(SecretKind::Curseforge)
                    .unwrap()
                    .unwrap()
                    .expose_secret(),
                CF
            );
            assert!(matches!(
                *state.env_migration.lock().unwrap(),
                Some(EnvMigrationNotice::Moved {
                    file_removed: true,
                    ..
                })
            ));
            assert_no_secret_in(&read_all_under(root, &[&vault]));
        });

        // Depois de gravados os registros (nível detalhado): nada nos registros nem no
        // `settings.json`.
        let log_files = read_all_under(&logs, &[]);
        assert!(!log_files.is_empty(), "nenhum registro gravado");
        let log_text: String = log_files
            .iter()
            .map(|(_, bytes)| String::from_utf8_lossy(bytes))
            .collect();
        assert!(log_text.contains("chave gravada"), "{log_text}");
        assert!(log_text.contains("levadas para o cofre"), "{log_text}");
        assert_no_secret_in(&log_files);
        let settings = std::fs::read_to_string(root.join("config/settings.json")).unwrap();
        assert!(!settings.contains("secretsBackend"), "{settings}");
        assert_no_secret_in(&read_all_under(root, &[&vault]));
    }

    /// O último teste: só a resposta do serviço conta; gravar a chave de novo esquece o teste.
    #[test]
    fn ultimo_teste_da_chave() {
        let dir = tempfile::tempdir().unwrap();
        let state = test_state(dir.path());
        let checks = &state.key_checks;
        let refused = AppError::new(warden_ai::AiErrorCode::KeyInvalid);
        let offline = AppError::new(warden_core::CoreErrorCode::NetworkUnavailable);
        record_check(checks, SecretKind::Openai, &Err(refused));
        assert!(checks.refused(SecretKind::Openai));
        record_check(checks, SecretKind::Openai, &Err(offline));
        assert!(checks.refused(SecretKind::Openai), "sem internet não muda o teste");
        record_check(checks, SecretKind::Openai, &Ok(SecretTestResult::NotTestable));
        assert!(checks.refused(SecretKind::Openai));
        record_check(checks, SecretKind::Openai, &Ok(SecretTestResult::Valid));
        assert_eq!(
            checks.get(SecretKind::Openai).map(|check| check.outcome),
            Some(KeyCheckOutcome::Working)
        );
        forget_check(checks, SecretKind::Openai);
        assert_eq!(checks.get(SecretKind::Openai), None);
    }

    /// A visão geral leva o final da chave e o teste, nunca o valor; o teste de uma chave que
    /// não está mais no cofre não aparece.
    #[test]
    fn visao_geral_das_chaves() {
        let dir = tempfile::tempdir().unwrap();
        let state = test_state(dir.path());
        set_impl(&state.secrets, SecretKind::Openai, &secret(OPENAI)).unwrap();
        record_check(
            &state.key_checks,
            SecretKind::Openai,
            &Err(AppError::new(warden_ai::AiErrorCode::KeyInvalid)),
        );
        record_check(
            &state.key_checks,
            SecretKind::Gemini,
            &Ok(SecretTestResult::Valid),
        );
        let overview = overview_of(&state, key_rows(&state.secrets).unwrap());
        let kinds: Vec<_> = overview.keys.iter().map(|key| key.kind).collect();
        assert_eq!(kinds, SecretKind::ALL.to_vec());
        let openai = &overview.keys[2];
        assert!(openai.configured);
        assert_eq!(openai.hint.as_deref(), Some("0000"));
        assert_eq!(
            openai.check.as_ref().map(|check| check.outcome),
            Some(KeyCheckOutcome::Refused)
        );
        let gemini = &overview.keys[1];
        assert!(!gemini.configured && gemini.hint.is_none() && gemini.check.is_none());
        assert_eq!(overview.migration, None);
        let json = serde_json::to_string(&overview).unwrap();
        assert!(!json.contains(OPENAI));
        assert!(json.contains("\"hint\":\"0000\""), "{json}");
    }

    #[test]
    fn valor_invalido_vira_erro_sem_o_valor() {
        let dir = tempfile::tempdir().unwrap();
        let state = test_state(dir.path());
        let error = set_impl(&state.secrets, SecretKind::Gemini, &secret("a'b")).unwrap_err();
        assert_eq!(
            error.code,
            ErrorCode::Secrets(SecretsErrorCode::InvalidValue)
        );
        assert!(!error.detail.unwrap_or_default().contains("a'b"));
    }

    #[test]
    fn remover_atualiza_o_estado() {
        let dir = tempfile::tempdir().unwrap();
        let state = test_state(dir.path());
        set_impl(&state.secrets, SecretKind::Github, &secret(GITHUB)).unwrap();
        let status = remove_impl(&state.secrets, SecretKind::Github).unwrap();
        assert!(!status.github);
    }

    struct Fixed(Result<(), AppError>);

    #[async_trait::async_trait]
    impl SecretTester for Fixed {
        async fn test(&self, value: SecretString) -> Result<(), AppError> {
            assert_eq!(value.expose_secret(), CF);
            self.0.clone()
        }
    }

    #[tokio::test]
    async fn testar_usa_o_testador_registrado() {
        let testers = SecretTesters::default();
        assert_eq!(
            test_impl(
                testers.get(SecretKind::Curseforge),
                SecretKind::Curseforge,
                secret(CF)
            )
            .await
            .unwrap(),
            SecretTestResult::NotTestable
        );
        testers.register(SecretKind::Curseforge, Arc::new(Fixed(Ok(()))));
        assert_eq!(
            test_impl(
                testers.get(SecretKind::Curseforge),
                SecretKind::Curseforge,
                secret(CF)
            )
            .await
            .unwrap(),
            SecretTestResult::Valid
        );
        testers.register(
            SecretKind::Curseforge,
            Arc::new(Fixed(Err(AppError::internal("recusada")))),
        );
        assert!(
            test_impl(
                testers.get(SecretKind::Curseforge),
                SecretKind::Curseforge,
                secret(CF)
            )
            .await
            .is_err()
        );
        assert!(format!("{testers:?}").contains("Curseforge"));
    }

    #[test]
    fn resultado_do_teste_em_json() {
        assert_eq!(
            serde_json::to_value(SecretTestResult::NotTestable).unwrap(),
            serde_json::json!({ "status": "notTestable" })
        );
    }
}
