//! Comandos do domínio `secrets` (ARCHITECTURE §4.1 e §14; T21 → Chaves e contas).
//!
//! Não existe comando para ler um segredo: a interface só conhece o estado
//! ([`SecretsStatus`]). O valor digitado chega em `secrets_set`, vira `SecretString` na hora e
//! nunca é registrado nem devolvido.
//!
//! `secrets_test` usa o testador registrado para cada chave em [`SecretTesters`]; as tarefas
//! das APIs (CurseForge, Gemini, GitHub) registram o seu. Sem testador, o resultado é
//! [`SecretTestResult::NotTestable`].

use std::collections::HashMap;
use std::sync::{Arc, PoisonError, RwLock};

use serde::Serialize;
use tauri::State;
use warden_secrets::{BackendKind, SecretKind, SecretString, Secrets, SecretsStatus};

use crate::error::AppError;
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

/// Onde as chaves ficam e quais estão configuradas.
#[tauri::command]
#[specta::specta]
pub(crate) async fn secrets_status(state: State<'_, AppState>) -> Result<SecretsStatus, AppError> {
    let secrets = Arc::clone(&state.secrets);
    blocking(move || Ok(secrets.status()?)).await
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
    blocking(move || set_impl(&secrets, kind, &value)).await
}

/// Apaga uma chave e devolve o estado.
#[tauri::command]
#[specta::specta]
pub(crate) async fn secrets_remove(
    state: State<'_, AppState>,
    kind: SecretKind,
) -> Result<SecretsStatus, AppError> {
    let secrets = Arc::clone(&state.secrets);
    blocking(move || remove_impl(&secrets, kind)).await
}

/// Testa uma chave na API dela.
#[tauri::command]
#[specta::specta]
pub(crate) async fn secrets_test(
    state: State<'_, AppState>,
    kind: SecretKind,
) -> Result<SecretTestResult, AppError> {
    let secrets = Arc::clone(&state.secrets);
    let value = blocking(move || Ok(secrets.require(kind)?)).await?;
    test_impl(state.secret_testers.get(kind), kind, value).await
}

/// Modo das chaves em uso.
// pendente-na-ui: P1-13 sem tela: Configurações lê o modo em secrets_status.backend; remover ou dar uso
#[tauri::command]
#[specta::specta]
#[allow(clippy::unnecessary_wraps, clippy::needless_pass_by_value)] // contrato do Tauri
pub(crate) fn secrets_backend_get(state: State<'_, AppState>) -> Result<BackendKind, AppError> {
    Ok(state.secrets.backend())
}

/// Troca o modo das chaves (cofre ↔ `.env`), movendo-as, e devolve o estado.
#[tauri::command]
#[specta::specta]
pub(crate) async fn secrets_backend_set(
    state: State<'_, AppState>,
    backend: BackendKind,
) -> Result<SecretsStatus, AppError> {
    let secrets = Arc::clone(&state.secrets);
    let settings = Arc::clone(&state.settings);
    blocking(move || backend_set_impl(&secrets, &settings, backend)).await
}

fn set_impl(
    secrets: &Secrets,
    kind: SecretKind,
    value: &SecretString,
) -> Result<SecretsStatus, AppError> {
    secrets.set(kind, value)?;
    tracing::info!(?kind, backend = ?secrets.backend(), "chave gravada");
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

fn backend_set_impl(
    secrets: &Secrets,
    settings: &SettingsStore,
    backend: BackendKind,
) -> Result<SecretsStatus, AppError> {
    let report = secrets.switch_backend(backend, |mode| {
        settings
            .set_secrets_backend(mode)
            .map_err(|error| error.to_string())
    })?;
    if !report.leftovers.is_empty() {
        tracing::warn!(leftovers = ?report.leftovers, "cópias antigas de chaves não apagadas");
    }
    Ok(secrets.status()?)
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
            for value in [CF, GEMINI, GITHUB] {
                assert!(
                    !text.contains(value),
                    "segredo encontrado em {}",
                    file.display()
                );
            }
        }
    }

    /// Critério 3 da F0-05, CA-T21-01 e CA-T21-03 (parte de backend), com varredura: gravar,
    /// reabrir, trocar para o `.env` e voltar, com uma falha injetada no meio; em nenhum
    /// momento um valor aparece em `settings.json`, nos registros (nível detalhado) ou fora do
    /// lugar escolhido.
    #[test]
    fn f0_05_ca3_ca_t21_01_ca_t21_03_varredura_de_segredos() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let logs = root.join("data").join("logs");
        std::fs::create_dir_all(&logs).unwrap();
        let vault = root.join("cofre-de-teste");
        let env_file = root.join("config").join(".env");

        with_file_logging(&logs, LogLevel::Detailed, || {
            // Modo cofre: gravar e "reiniciar".
            let state = test_state(root);
            set_impl(&state.secrets, SecretKind::Curseforge, &secret(CF)).unwrap();
            set_impl(&state.secrets, SecretKind::Gemini, &secret(GEMINI)).unwrap();
            let status = set_impl(&state.secrets, SecretKind::Github, &secret(GITHUB)).unwrap();
            assert!(status.curseforge && status.gemini && status.github);
            drop(state);
            let state = test_state(root);
            let status = state.secrets.status().unwrap();
            assert_eq!(status.backend, BackendKind::Keyring);
            assert!(status.curseforge && status.gemini && status.github);
            assert_no_secret_in(&read_all_under(root, &[&vault]));

            // Falha injetada no meio da troca: nada muda, nada se perde.
            {
                let _guard = warden_core::fault::arm_after("secrets.switch.after_copy", 1);
                let error = backend_set_impl(&state.secrets, &state.settings, BackendKind::Envfile)
                    .unwrap_err();
                assert_eq!(
                    error.code,
                    ErrorCode::Secrets(SecretsErrorCode::BackendSwitchFailed)
                );
            }
            assert_eq!(state.settings.get().secrets_backend, BackendKind::Keyring);
            assert!(!env_file.exists());

            // Para o `.env`: as três chaves vão para o arquivo e saem do cofre.
            let status =
                backend_set_impl(&state.secrets, &state.settings, BackendKind::Envfile).unwrap();
            assert_eq!(status.backend, BackendKind::Envfile);
            assert!(status.curseforge && status.gemini && status.github);
            let env_text = std::fs::read_to_string(&env_file).unwrap();
            assert!(
                env_text.contains(CF) && env_text.contains(GEMINI) && env_text.contains(GITHUB)
            );
            assert!(
                read_all_under(&vault, &[]).is_empty(),
                "cofre não esvaziado"
            );
            assert_no_secret_in(&read_all_under(root, &[&env_file]));
            drop(state);

            // Reiniciar no modo `.env`.
            let state = test_state(root);
            assert_eq!(state.secrets.backend(), BackendKind::Envfile);
            let status = state.secrets.status().unwrap();
            assert!(status.curseforge && status.gemini && status.github);

            // De volta ao cofre: o `.env` some.
            let status =
                backend_set_impl(&state.secrets, &state.settings, BackendKind::Keyring).unwrap();
            assert_eq!(status.backend, BackendKind::Keyring);
            assert!(!env_file.exists());
            assert_eq!(
                state
                    .secrets
                    .get(SecretKind::Curseforge)
                    .unwrap()
                    .unwrap()
                    .expose_secret(),
                CF
            );
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
        assert!(log_text.contains("chaves movidas"), "{log_text}");
        assert_no_secret_in(&log_files);
        let settings = std::fs::read_to_string(root.join("config/settings.json")).unwrap();
        assert!(
            settings.contains("\"secretsBackend\": \"keyring\""),
            "{settings}"
        );
        assert_no_secret_in(&read_all_under(root, &[&vault]));
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
