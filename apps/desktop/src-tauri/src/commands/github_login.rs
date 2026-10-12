//! Comandos do domínio `github`, parte do login: "Entrar com o GitHub" (UI-03; ADR-0059).
//!
//! - `github_login_start` pede o código do dispositivo e devolve o código de 8 letras; a
//!   espera pela aprovação roda em segundo plano e avisa a tela pelo evento
//!   `github-login-updated` (esperando, mais devagar, conectado, falhou, cancelado).
//! - `github_login_cancel` para a espera; `github_login_open_page` abre só a página do GitHub
//!   em que se digita o código; `github_logout` apaga os dois tokens e o `github-login.json`.
//! - [`GithubSession`] guarda o login: o token de acesso em `SecretKind::Github` (o mesmo
//!   lugar do token colado), o de renovação em `SecretKind::GithubRefresh` e a conta e as
//!   validades (que não são segredo) em `github-login.json`. [`GithubSession::token`] renova
//!   sozinho quando faltam menos de 5 minutos; renovação recusada desconecta.
//!
//! Nenhum token passa pela interface, pelos eventos nem pelos registros: só o código que a
//! pessoa digita e o nome da conta.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex as StdMutex, PoisonError};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};
use tauri_specta::Event as _;
use warden_core::{AppPaths, CancellationToken};
use warden_github::device::{self, TokenSet};
use warden_github::{Endpoints, GithubApi, SecretString, WaitEvent};
use warden_http::{HttpClient, HttpConfig, SystemTimer, Timer};
use warden_secrets::{SecretKind, Secrets};

use crate::error::AppError;
use crate::state::AppState;

/// Arquivo com a conta e as validades do login, na pasta de configuração.
pub(crate) const LOGIN_FILE: &str = "github-login.json";

/// Renova o token quando falta menos que isto para vencer.
const REFRESH_MARGIN: Duration = Duration::from_mins(5);

/// Versão do formato de `github-login.json`.
const LOGIN_FILE_VERSION: u32 = 1;

/// O que a tela sabe do login.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GithubLoginStatus {
    /// O Warden tem o identificador do GitHub App (sem ele, o login fica indisponível).
    pub(crate) available: bool,
    /// Conta conectada, se houver.
    pub(crate) login: Option<String>,
    /// Há uma espera de aprovação em andamento.
    pub(crate) waiting: bool,
}

/// O código pedido ao GitHub, para a pessoa digitar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GithubLoginCode {
    /// Código de 8 letras ("WDJB-MJHT").
    pub(crate) user_code: String,
    /// Página em que se digita o código.
    pub(crate) verification_uri: String,
    /// Por quantos segundos o código vale.
    pub(crate) expires_in_seconds: u32,
}

/// Situação da espera pela aprovação.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(tag = "state", rename_all = "camelCase")]
pub(crate) enum GithubLoginUpdate {
    /// Ainda esperando a aprovação no GitHub.
    Waiting,
    /// O GitHub pediu para consultar mais devagar.
    #[serde(rename_all = "camelCase")]
    SlowDown {
        /// Novo intervalo entre as consultas.
        interval_seconds: u32,
    },
    /// Login feito.
    Connected {
        /// A conta.
        login: String,
    },
    /// A espera terminou com erro (recusado, código vencido, sem internet…).
    Failed {
        /// O erro.
        error: AppError,
    },
    /// A pessoa cancelou.
    Cancelled,
}

/// `github-login-updated`: a espera pela aprovação mudou.
#[derive(Debug, Clone, Serialize, specta::Type, tauri_specta::Event)]
#[serde(transparent)]
#[tauri_specta(event_name = "github-login-updated")]
pub(crate) struct GithubLoginUpdated(pub(crate) GithubLoginUpdate);

/// `github-login.json` (sem segredo).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LoginFile {
    schema_version: u32,
    /// Conta do login.
    login: String,
    /// Quando o token de acesso vence (segundos desde 1970); `None` = não vence.
    access_expires_at: Option<u64>,
    /// Quando o token de renovação vence.
    refresh_expires_at: Option<u64>,
}

/// Quem recebe as mudanças da espera (o evento, no app; uma lista, nos testes).
pub(crate) type LoginNotifier = Arc<dyn Fn(GithubLoginUpdate) + Send + Sync>;

/// O login do GitHub.
pub(crate) struct GithubSession {
    http: HttpClient,
    endpoints: Endpoints,
    secrets: Arc<Secrets>,
    file: PathBuf,
    timer: Arc<dyn Timer>,
    /// Uma leitura, renovação, gravação ou saída por vez (a renovação troca os dois tokens).
    tokens: tokio::sync::Mutex<()>,
    /// A espera pela aprovação em andamento.
    waiting: StdMutex<Option<CancellationToken>>,
}

impl std::fmt::Debug for GithubSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GithubSession")
            .field("endpoints", &self.endpoints)
            .field("file", &self.file)
            .finish_non_exhaustive()
    }
}

impl GithubSession {
    /// O login do app: endereços oficiais (ou os de teste, em debug) e o cofre do app.
    pub(crate) fn open(paths: &AppPaths, secrets: Arc<Secrets>) -> Result<Self, AppError> {
        let http = HttpClient::new(HttpConfig::for_version(env!("CARGO_PKG_VERSION")))
            .map_err(|error| AppError::from_domain(&error))?;
        let endpoints = Endpoints::from_env().map_err(|error| AppError::from_domain(&error))?;
        Ok(Self::with(
            http,
            endpoints,
            secrets,
            &paths.config_dir().join(LOGIN_FILE),
            Arc::new(SystemTimer::new()),
        ))
    }

    /// Com tudo dado (testes).
    pub(crate) fn with(
        http: HttpClient,
        endpoints: Endpoints,
        secrets: Arc<Secrets>,
        file: &Path,
        timer: Arc<dyn Timer>,
    ) -> Self {
        Self {
            http,
            endpoints,
            secrets,
            file: file.to_owned(),
            timer,
            tokens: tokio::sync::Mutex::new(()),
            waiting: StdMutex::new(None),
        }
    }

    /// Os endereços em uso.
    pub(crate) fn endpoints(&self) -> &Endpoints {
        &self.endpoints
    }

    /// O cliente HTTP do login.
    pub(crate) fn http(&self) -> &HttpClient {
        &self.http
    }

    /// Situação do login.
    pub(crate) async fn status(&self) -> Result<GithubLoginStatus, AppError> {
        let _guard = self.tokens.lock().await;
        let login = self.read_login().await?;
        Ok(GithubLoginStatus {
            available: self.endpoints.login_available(),
            login: login.map(|file| file.login),
            waiting: self.lock_waiting().is_some(),
        })
    }

    /// Pede o código e começa a esperar a aprovação em segundo plano.
    pub(crate) async fn start(
        self: &Arc<Self>,
        notify: LoginNotifier,
    ) -> Result<GithubLoginCode, AppError> {
        if !self.endpoints.login_available() {
            return Err(AppError::from_domain(&warden_github::Error::LoginUnavailable));
        }
        self.cancel();
        let code = device::request_code(&self.http, &self.endpoints)
            .await
            .map_err(|error| AppError::from_domain(&error))?;
        let cancel = CancellationToken::new();
        *self.lock_waiting() = Some(cancel.clone());
        tracing::info!(validade_s = code.expires_in.as_secs(), "login do GitHub: código pedido");
        let reply = GithubLoginCode {
            user_code: code.user_code.clone(),
            verification_uri: self
                .endpoints
                .device_page()
                .map_err(|error| AppError::from_domain(&error))?
                .to_string(),
            expires_in_seconds: u32::try_from(code.expires_in.as_secs()).unwrap_or(u32::MAX),
        };
        let session = Arc::clone(self);
        tokio::spawn(async move {
            let update = session.wait(&code, &cancel, &notify).await;
            // Só limpa se ainda for esta espera (um novo "Entrar" pode ter começado outra).
            {
                let mut waiting = session.lock_waiting();
                if waiting.as_ref().is_some_and(|current| current == &cancel) {
                    *waiting = None;
                }
            }
            notify(update);
        });
        Ok(reply)
    }

    async fn wait(
        &self,
        code: &device::DeviceCode,
        cancel: &CancellationToken,
        notify: &LoginNotifier,
    ) -> GithubLoginUpdate {
        let tokens = device::wait_for_token(
            &self.http,
            &self.endpoints,
            code,
            self.timer.as_ref(),
            cancel,
            |event| {
                notify(match event {
                    WaitEvent::Pending => GithubLoginUpdate::Waiting,
                    WaitEvent::SlowDown(interval) => GithubLoginUpdate::SlowDown {
                        interval_seconds: u32::try_from(interval.as_secs()).unwrap_or(u32::MAX),
                    },
                });
            },
        )
        .await;
        let result = match tokens {
            Ok(tokens) => self.finish_login(tokens).await,
            Err(warden_github::Error::Cancelled) => {
                tracing::info!("login do GitHub cancelado");
                return GithubLoginUpdate::Cancelled;
            }
            Err(error) => Err(AppError::from_domain(&error)),
        };
        match result {
            Ok(login) => {
                tracing::info!(%login, "login do GitHub feito");
                GithubLoginUpdate::Connected { login }
            }
            Err(error) => {
                tracing::warn!(code = %error.code, "login do GitHub não terminou");
                GithubLoginUpdate::Failed { error }
            }
        }
    }

    /// Com os tokens: descobre a conta e grava tudo.
    async fn finish_login(&self, tokens: TokenSet) -> Result<String, AppError> {
        let user = GithubApi::new(self.http.clone(), self.endpoints.clone(), &tokens.access_token)
            .user()
            .await
            .map_err(|error| AppError::from_domain(&error))?;
        let _guard = self.tokens.lock().await;
        self.save(tokens, user.login.clone()).await?;
        Ok(user.login)
    }

    /// Para a espera pela aprovação, se houver.
    pub(crate) fn cancel(&self) {
        if let Some(cancel) = self.lock_waiting().take() {
            cancel.cancel();
        }
    }

    /// Sai do GitHub: para a espera e apaga os dois tokens e o `github-login.json`.
    pub(crate) async fn logout(&self) -> Result<(), AppError> {
        self.cancel();
        let _guard = self.tokens.lock().await;
        self.clear().await?;
        tracing::info!("saiu do GitHub");
        Ok(())
    }

    /// O token de acesso para usar agora, renovado se estiver para vencer. Sem login:
    /// `github.NOT_SIGNED_IN`; renovação recusada: desconecta e `github.SESSION_EXPIRED`.
    pub(crate) async fn token(&self) -> Result<SecretString, AppError> {
        let _guard = self.tokens.lock().await;
        let Some(file) = self.read_login().await? else {
            return Err(AppError::from_domain(&warden_github::Error::NotSignedIn));
        };
        let expiring = file
            .access_expires_at
            .is_some_and(|at| at <= now_unix().saturating_add(REFRESH_MARGIN.as_secs()));
        if expiring {
            return self.refresh_locked(file).await;
        }
        self.read_secret(SecretKind::Github)
            .await?
            .ok_or_else(|| AppError::from_domain(&warden_github::Error::NotSignedIn))
    }

    /// Renova agora (o GitHub recusou o token antes da hora: 401).
    pub(crate) async fn refresh_now(&self) -> Result<SecretString, AppError> {
        let _guard = self.tokens.lock().await;
        let Some(file) = self.read_login().await? else {
            return Err(AppError::from_domain(&warden_github::Error::NotSignedIn));
        };
        self.refresh_locked(file).await
    }

    async fn refresh_locked(&self, file: LoginFile) -> Result<SecretString, AppError> {
        let refresh_token = self.read_secret(SecretKind::GithubRefresh).await?;
        let expired = || {
            AppError::from_domain(&warden_github::Error::SessionExpired {
                reason: "sem token de renovação válido".to_owned(),
            })
        };
        let usable = refresh_token.filter(|_| {
            file.refresh_expires_at
                .is_none_or(|at| at > now_unix())
        });
        let Some(refresh_token) = usable else {
            self.clear().await?;
            return Err(expired());
        };
        match device::refresh(&self.http, &self.endpoints, &refresh_token).await {
            Ok(tokens) => {
                let access = tokens.access_token.clone();
                self.save(tokens, file.login).await?;
                tracing::info!("token do GitHub renovado");
                Ok(access)
            }
            Err(error) if error.needs_login() => {
                tracing::warn!("renovação do token do GitHub recusada: login desfeito");
                self.clear().await?;
                Err(AppError::from_domain(&error))
            }
            Err(error) => Err(AppError::from_domain(&error)),
        }
    }

    /// Grava os tokens no cofre e a conta no arquivo. Chame com `tokens` travado.
    async fn save(&self, tokens: TokenSet, login: String) -> Result<(), AppError> {
        let now = now_unix();
        let file = LoginFile {
            schema_version: LOGIN_FILE_VERSION,
            login,
            access_expires_at: tokens.expires_in.map(|valid| now.saturating_add(valid.as_secs())),
            refresh_expires_at: tokens
                .refresh_token_expires_in
                .map(|valid| now.saturating_add(valid.as_secs())),
        };
        let secrets = Arc::clone(&self.secrets);
        let path = self.file.clone();
        blocking(move || {
            secrets.set(SecretKind::Github, &tokens.access_token)?;
            match &tokens.refresh_token {
                Some(refresh) => secrets.set(SecretKind::GithubRefresh, refresh)?,
                None => secrets.remove(SecretKind::GithubRefresh)?,
            }
            let json = serde_json::to_vec_pretty(&file)
                .map_err(|error| AppError::internal(format!("github-login.json: {error}")))?;
            warden_core::atomic_write(&path, &json)?;
            Ok(())
        })
        .await
    }

    /// Apaga os dois tokens e o arquivo. Chame com `tokens` travado.
    async fn clear(&self) -> Result<(), AppError> {
        let secrets = Arc::clone(&self.secrets);
        let path = self.file.clone();
        blocking(move || {
            secrets.remove(SecretKind::Github)?;
            secrets.remove(SecretKind::GithubRefresh)?;
            match std::fs::remove_file(&path) {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(error) => Err(warden_core::CoreError::io("apagar", &path, error).into()),
            }
        })
        .await
    }

    /// O login gravado, se o token também estiver guardado pelo Warden (o token pode ter sido
    /// removido pela linha do GitHub em Chaves e contas). O recuo de desenvolvimento pela
    /// variável `GITHUB_TOKEN` não conta como login.
    async fn read_login(&self) -> Result<Option<LoginFile>, AppError> {
        let secrets = Arc::clone(&self.secrets);
        let path = self.file.clone();
        blocking(move || {
            let file = match std::fs::read(&path) {
                Ok(bytes) => serde_json::from_slice::<LoginFile>(&bytes).ok(),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
                Err(error) => return Err(warden_core::CoreError::io("ler", &path, error).into()),
            };
            match file {
                Some(file) if secrets.status()?.github => Ok(Some(file)),
                _ => Ok(None),
            }
        })
        .await
    }

    async fn read_secret(&self, kind: SecretKind) -> Result<Option<SecretString>, AppError> {
        let secrets = Arc::clone(&self.secrets);
        blocking(move || Ok(secrets.get(kind)?)).await
    }

    fn lock_waiting(&self) -> std::sync::MutexGuard<'_, Option<CancellationToken>> {
        self.waiting.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

/// Cofre do sistema e disco fora da thread do IPC.
async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, AppError> + Send + 'static,
) -> Result<T, AppError> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|error| AppError::internal(format!("tarefa do login do GitHub interrompida: {error}")))?
}

/// Situação do login do GitHub.
#[tauri::command]
#[specta::specta]
pub(crate) async fn github_login_status(
    state: State<'_, AppState>,
) -> Result<GithubLoginStatus, AppError> {
    state.github.status().await
}

/// "Entrar com o GitHub": pede o código; a espera avisa por `github-login-updated`.
#[tauri::command]
#[specta::specta]
pub(crate) async fn github_login_start(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<GithubLoginCode, AppError> {
    let notify: LoginNotifier = Arc::new(move |update| {
        if let Err(error) = GithubLoginUpdated(update).emit(&app) {
            tracing::warn!(%error, "evento github-login-updated não enviado");
        }
    });
    state.github.start(notify).await
}

/// Para a espera pela aprovação.
#[tauri::command]
#[specta::specta]
#[allow(clippy::unnecessary_wraps, clippy::needless_pass_by_value)] // contrato do Tauri
pub(crate) fn github_login_cancel(state: State<'_, AppState>) -> Result<(), AppError> {
    state.github.cancel();
    Ok(())
}

/// "Abrir o GitHub": abre no navegador só a página em que se digita o código.
#[tauri::command]
#[specta::specta]
#[allow(clippy::needless_pass_by_value)] // contrato do Tauri
pub(crate) fn github_login_open_page(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    let page = state
        .github
        .endpoints()
        .device_page()
        .map_err(|error| AppError::from_domain(&error))?;
    crate::commands::report::open_in_browser(&app, page.as_str())
}

/// "Sair do GitHub": apaga os dois tokens e a conta.
#[tauri::command]
#[specta::specta]
pub(crate) async fn github_logout(
    state: State<'_, AppState>,
) -> Result<GithubLoginStatus, AppError> {
    state.github.logout().await?;
    state.github.status().await
}

#[cfg(test)]
pub(crate) mod tests {
    use std::sync::Mutex;

    use serde_json::json;
    use warden_github::ExposeSecret as _;
    use warden_http::ManualTimer;
    use wiremock::matchers::{body_string_contains, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    use crate::error::ErrorCode;

    pub(crate) const CLIENT_ID: &str = "Iv1.cliente-de-teste";

    pub(crate) fn access(n: u32) -> String {
        format!("ghu_{}{n:04}", "AcessoFalsoDoTesteDoApp0123456789")
    }
    pub(crate) fn refresh(n: u32) -> String {
        format!("ghr_{}{n:04}", "RenovacaoFalsaDoTesteDoApp0123456789abcdefghij")
    }
    const DEVICE_CODE: &str = "dispositivo-secreto-do-teste-0123456789abcdef";

    pub(crate) fn endpoints(server: &MockServer, client_id: &str) -> Endpoints {
        Endpoints::custom(
            &format!("{}/api/", server.uri()),
            &format!("{}/site/", server.uri()),
            client_id,
        )
        .unwrap()
    }

    pub(crate) fn session(
        root: &Path,
        server: &MockServer,
        client_id: &str,
    ) -> (Arc<GithubSession>, Arc<Secrets>) {
        let state = crate::state::tests::test_state(root);
        let secrets = Arc::clone(&state.secrets);
        let timer = ManualTimer::new();
        let http = HttpClient::with_timer(
            HttpConfig::for_version("9.9.9"),
            Arc::new(timer.clone()),
        )
        .unwrap();
        let session = GithubSession::with(
            http,
            endpoints(server, client_id),
            Arc::clone(&secrets),
            &root.join(LOGIN_FILE),
            Arc::new(timer),
        );
        (Arc::new(session), secrets)
    }

    /// Grava um login como se a pessoa tivesse entrado, com o token vencendo em `expires_in`
    /// segundos (negativo = já venceu).
    pub(crate) fn signed_in(root: &Path, secrets: &Secrets, expires_in: i64) {
        secrets
            .set(SecretKind::Github, &SecretString::from(access(1)))
            .unwrap();
        secrets
            .set(SecretKind::GithubRefresh, &SecretString::from(refresh(1)))
            .unwrap();
        let at = now_unix().saturating_add_signed(expires_in);
        std::fs::write(
            root.join(LOGIN_FILE),
            serde_json::to_vec(&json!({
                "schemaVersion": 1,
                "login": "Kriticales",
                "accessExpiresAt": at,
                "refreshExpiresAt": now_unix() + 1_000_000,
            }))
            .unwrap(),
        )
        .unwrap();
    }

    pub(crate) async fn mount_refresh(server: &MockServer, from: u32, to: u32) {
        Mock::given(method("POST"))
            .and(path("/site/login/oauth/access_token"))
            .and(body_string_contains("grant_type=refresh_token"))
            .and(body_string_contains(refresh(from).as_str()))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "access_token": access(to),
                "expires_in": 28800,
                "refresh_token": refresh(to),
                "refresh_token_expires_in": 15_897_600,
                "token_type": "bearer",
                "scope": ""
            })))
            .mount(server)
            .await;
    }

    /// O valor guardado pelo Warden (sem o recuo pelo ambiente do desenvolvimento: o
    /// contêiner da CI pode ter um `GITHUB_TOKEN`).
    pub(crate) fn value(secrets: &Secrets, kind: SecretKind) -> Option<String> {
        if !secrets.status().unwrap().is_configured(kind) {
            return None;
        }
        secrets
            .get(kind)
            .unwrap()
            .map(|value| value.expose_secret().to_owned())
    }

    fn code(error: &AppError) -> String {
        error.code.to_string()
    }

    async fn wait_for(updates: &Arc<Mutex<Vec<GithubLoginUpdate>>>, last: impl Fn(&GithubLoginUpdate) -> bool) {
        for _ in 0..500 {
            if updates.lock().unwrap().last().is_some_and(&last) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("a espera não terminou: {:?}", updates.lock().unwrap());
    }

    fn recorder() -> (LoginNotifier, Arc<Mutex<Vec<GithubLoginUpdate>>>) {
        let updates = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&updates);
        (
            Arc::new(move |update| sink.lock().unwrap().push(update)),
            updates,
        )
    }

    async fn mount_device(server: &MockServer) {
        Mock::given(method("POST"))
            .and(path("/site/login/device/code"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "device_code": DEVICE_CODE,
                "user_code": "WDJB-MJHT",
                "verification_uri": "https://github.com/login/device",
                "expires_in": 900,
                "interval": 5
            })))
            .mount(server)
            .await;
    }

    /// CA-UI03-02 e CA-UI03-09: o login grava os dois tokens e a conta, os eventos levam só o
    /// código e a conta, e sair apaga tudo.
    #[tokio::test]
    async fn ca_ui03_02_entrar_e_sair() {
        let dir = tempfile::tempdir().unwrap();
        let server = MockServer::start().await;
        mount_device(&server).await;
        Mock::given(method("POST"))
            .and(path("/site/login/oauth/access_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "error": "authorization_pending"
            })))
            .up_to_n_times(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/site/login/oauth/access_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "access_token": access(1),
                "expires_in": 28800,
                "refresh_token": refresh(1),
                "refresh_token_expires_in": 15_897_600,
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/api/user"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "login": "Kriticales" })))
            .mount(&server)
            .await;
        let (session, secrets) = session(dir.path(), &server, CLIENT_ID);
        let before = session.status().await.unwrap();
        assert_eq!(
            before,
            GithubLoginStatus {
                available: true,
                login: None,
                waiting: false
            }
        );
        let (notify, updates) = recorder();
        let code = session.start(notify).await.unwrap();
        assert_eq!(code.user_code, "WDJB-MJHT");
        assert_eq!(
            code.verification_uri,
            format!("{}/site/login/device", server.uri())
        );
        assert_eq!(code.expires_in_seconds, 900);
        wait_for(&updates, |update| matches!(update, GithubLoginUpdate::Connected { .. })).await;
        assert_eq!(
            *updates.lock().unwrap(),
            [
                GithubLoginUpdate::Waiting,
                GithubLoginUpdate::Connected {
                    login: "Kriticales".into()
                }
            ]
        );
        assert_eq!(value(&secrets, SecretKind::Github), Some(access(1)));
        assert_eq!(value(&secrets, SecretKind::GithubRefresh), Some(refresh(1)));
        let status = session.status().await.unwrap();
        assert_eq!(status.login.as_deref(), Some("Kriticales"));
        assert!(!status.waiting);
        // O arquivo do login não tem segredo; os eventos também não.
        let file = std::fs::read_to_string(dir.path().join(LOGIN_FILE)).unwrap();
        let events = serde_json::to_string(&*updates.lock().unwrap()).unwrap();
        for secret in [access(1), refresh(1), DEVICE_CODE.to_owned()] {
            assert!(!file.contains(&secret), "{file}");
            assert!(!events.contains(&secret), "{events}");
        }
        assert!(file.contains("\"login\": \"Kriticales\""), "{file}");

        session.logout().await.unwrap();
        assert_eq!(value(&secrets, SecretKind::Github), None);
        assert_eq!(value(&secrets, SecretKind::GithubRefresh), None);
        assert!(!dir.path().join(LOGIN_FILE).exists());
        assert_eq!(session.status().await.unwrap().login, None);
    }

    /// CA-UI03-03: recusado no GitHub vira `Failed` com o código certo; nada é gravado.
    #[tokio::test]
    async fn ca_ui03_03_recusado_nao_grava_nada() {
        let dir = tempfile::tempdir().unwrap();
        let server = MockServer::start().await;
        mount_device(&server).await;
        Mock::given(method("POST"))
            .and(path("/site/login/oauth/access_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "error": "access_denied" })))
            .mount(&server)
            .await;
        let (session, secrets) = session(dir.path(), &server, CLIENT_ID);
        let (notify, updates) = recorder();
        session.start(notify).await.unwrap();
        wait_for(&updates, |update| matches!(update, GithubLoginUpdate::Failed { .. })).await;
        let Some(GithubLoginUpdate::Failed { error }) = updates.lock().unwrap().last().cloned()
        else {
            panic!("sem falha");
        };
        assert_eq!(code(&error), "github.LOGIN_DENIED");
        assert_eq!(value(&secrets, SecretKind::Github), None);
        assert!(!dir.path().join(LOGIN_FILE).exists());
    }

    /// CA-UI03-03: cancelar para a espera e avisa `Cancelled`.
    #[tokio::test]
    async fn ca_ui03_03_cancelar() {
        let dir = tempfile::tempdir().unwrap();
        let server = MockServer::start().await;
        mount_device(&server).await;
        // O GitHub demora a responder: a espera fica parada no pedido até o cancelamento.
        Mock::given(method("POST"))
            .and(path("/site/login/oauth/access_token"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(json!({ "error": "authorization_pending" }))
                    .set_delay(Duration::from_secs(30)),
            )
            .mount(&server)
            .await;
        let (session, _) = session(dir.path(), &server, CLIENT_ID);
        let (notify, updates) = recorder();
        session.start(notify).await.unwrap();
        assert!(session.status().await.unwrap().waiting);
        session.cancel();
        wait_for(&updates, |update| *update == GithubLoginUpdate::Cancelled).await;
        assert!(!session.status().await.unwrap().waiting);
    }

    /// CA-UI03-10: sem o identificador do GitHub App, nada sai e o login fica indisponível.
    #[tokio::test]
    async fn ca_ui03_10_sem_client_id() {
        let dir = tempfile::tempdir().unwrap();
        let server = MockServer::start().await;
        let (session, _) = session(dir.path(), &server, "");
        assert!(!session.status().await.unwrap().available);
        let (notify, _) = recorder();
        let error = session.start(notify).await.unwrap_err();
        assert_eq!(code(&error), "github.LOGIN_UNAVAILABLE");
        assert!(server.received_requests().await.unwrap().is_empty());
        // O valor real do app continua vazio até o dono criar o GitHub App (ADR-0059).
        assert!(!Endpoints::official().unwrap().login_available());
    }

    /// CA-UI03-04: token perto de vencer é renovado, e os dois tokens novos substituem os
    /// antigos; renovação recusada desconecta.
    #[tokio::test]
    async fn ca_ui03_04_renova_antes_de_vencer() {
        let dir = tempfile::tempdir().unwrap();
        let server = MockServer::start().await;
        mount_refresh(&server, 1, 2).await;
        let (session, secrets) = session(dir.path(), &server, CLIENT_ID);
        // Longe de vencer: usa o guardado, sem pedido.
        signed_in(dir.path(), &secrets, 3_600);
        assert_eq!(session.token().await.unwrap().expose_secret(), access(1));
        assert!(server.received_requests().await.unwrap().is_empty());
        // A 2 minutos de vencer: renova.
        signed_in(dir.path(), &secrets, 120);
        assert_eq!(session.token().await.unwrap().expose_secret(), access(2));
        assert_eq!(value(&secrets, SecretKind::Github), Some(access(2)));
        assert_eq!(value(&secrets, SecretKind::GithubRefresh), Some(refresh(2)));
        // E a validade nova foi gravada: a próxima leitura não renova de novo.
        assert_eq!(session.token().await.unwrap().expose_secret(), access(2));
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn ca_ui03_04_renovacao_recusada_desconecta() {
        let dir = tempfile::tempdir().unwrap();
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/site/login/oauth/access_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "error": "bad_refresh_token"
            })))
            .mount(&server)
            .await;
        let (session, secrets) = session(dir.path(), &server, CLIENT_ID);
        signed_in(dir.path(), &secrets, -10);
        let error = session.token().await.unwrap_err();
        assert_eq!(code(&error), "github.SESSION_EXPIRED");
        assert_eq!(value(&secrets, SecretKind::Github), None);
        assert_eq!(value(&secrets, SecretKind::GithubRefresh), None);
        assert_eq!(session.status().await.unwrap().login, None);
        let error = session.token().await.unwrap_err();
        assert_eq!(code(&error), "github.NOT_SIGNED_IN");
    }

    #[tokio::test]
    async fn token_removido_em_chaves_e_contas_desfaz_o_login() {
        let dir = tempfile::tempdir().unwrap();
        let server = MockServer::start().await;
        let (session, secrets) = session(dir.path(), &server, CLIENT_ID);
        signed_in(dir.path(), &secrets, 3_600);
        assert!(session.status().await.unwrap().login.is_some());
        secrets.remove(SecretKind::Github).unwrap();
        assert_eq!(session.status().await.unwrap().login, None);
        assert!(matches!(
            session.token().await.unwrap_err().code,
            ErrorCode::Github(warden_github::GithubErrorCode::NotSignedIn)
        ));
    }

    #[test]
    fn evento_em_camel_case_sem_segredo() {
        let json = serde_json::to_value(GithubLoginUpdated(GithubLoginUpdate::SlowDown {
            interval_seconds: 10,
        }))
        .unwrap();
        assert_eq!(json, json!({ "state": "slowDown", "intervalSeconds": 10 }));
        let json = serde_json::to_value(GithubLoginUpdate::Connected {
            login: "Kriticales".into(),
        })
        .unwrap();
        assert_eq!(json, json!({ "state": "connected", "login": "Kriticales" }));
    }
}
