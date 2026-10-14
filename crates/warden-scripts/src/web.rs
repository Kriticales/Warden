//! Cliente do servidor web local do KubeJS 7 (ARCHITECTURE §10.2; R5B §3.4).
//!
//! Com o KubeJS 7 e o jogo aberto, o KubeJS escuta em `127.0.0.1`, na porta de
//! `kubejs/config/web_server.json` (61423 por padrão, ou uma das 9 seguintes se ela estiver
//! ocupada). As rotas de recarga e de erros pedem `Authorization: Bearer <token>`, com o token do
//! mesmo arquivo (`auth`), gerado pelo jogo na primeira execução.
//!
//! **O token é segredo** (ADR-0037): é lido aqui, na hora, do arquivo **da instância de teste**,
//! fica num [`SecretString`], vai só no cabeçalho marcado como sensível e só para `127.0.0.1`,
//! e nunca sai desta crate (nada o devolve à interface, nenhum registro o escreve, nenhum erro o
//! carrega). Antes de mandar o token, o cliente confere que quem responde na porta é o KubeJS
//! (a página inicial começa com "KubeJS Local Web Server").

use std::fmt;
use std::path::Path;
use std::time::Duration;

use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use warden_http::{HeaderName, HttpClient, HttpConfig, Url};

use crate::detect::KubejsScriptType;
use crate::error::{Error, Result};
use crate::ids::IdGroup;
use crate::problems::{ProblemLevel, ScriptProblem};

/// Arquivo da configuração, relativo à pasta do jogo.
pub const WEB_SERVER_FILE: &str = "kubejs/config/web_server.json";
/// Porta padrão do KubeJS.
pub const DEFAULT_PORT: u16 = 61_423;
/// Portas tentadas a partir da configurada (o KubeJS tenta até `porta + 9`).
pub const PORT_ATTEMPTS: u16 = 10;
/// Origem dos IDs que vêm do jogo aberto (a interface mostra o nome traduzido).
pub const LIVE_SOURCE: &str = "kubejs-web";
/// Começo da página inicial do servidor do KubeJS.
const HOMEPAGE_PREFIX: &str = "KubeJS Local Web Server";

/// A configuração do servidor web lida da instância. O `Debug` não mostra o token.
pub struct WebServerConfig {
    /// Porta configurada.
    pub port: u16,
    token: SecretString,
}

impl fmt::Debug for WebServerConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WebServerConfig")
            .field("port", &self.port)
            .finish_non_exhaustive()
    }
}

impl WebServerConfig {
    /// Configuração com o token dado (testes e servidor simulado).
    #[must_use]
    pub fn new(port: u16, token: SecretString) -> Self {
        Self { port, token }
    }
}

#[derive(Deserialize)]
struct RawConfig {
    enabled: Option<bool>,
    port: Option<u16>,
    auth: Option<String>,
}

/// Lê `kubejs/config/web_server.json` da pasta do jogo. `None` se o arquivo não existe, não é
/// JSON, o servidor está desligado (`enabled = false`) ou não há token.
#[must_use]
pub fn read_web_server_config(game_dir: &Path) -> Option<WebServerConfig> {
    let path = warden_core::resolve_inside(game_dir, WEB_SERVER_FILE).ok()?;
    let bytes = std::fs::read(path).ok()?;
    let raw: RawConfig = serde_json::from_slice(&bytes).ok()?;
    if raw.enabled == Some(false) {
        return None;
    }
    let token = raw.auth.filter(|auth| !auth.trim().is_empty())?;
    Some(WebServerConfig {
        port: raw.port.filter(|&port| port > 0).unwrap_or(DEFAULT_PORT),
        token: SecretString::from(token),
    })
}

/// Cliente do servidor web do KubeJS numa instância.
pub struct KubejsWeb {
    http: HttpClient,
    config: WebServerConfig,
}

impl fmt::Debug for KubejsWeb {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("KubejsWeb")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

/// Uma linha do console do KubeJS (`ConsoleLine.toJson`).
#[derive(Deserialize)]
struct ConsoleLineJson {
    #[serde(default)]
    message: String,
    #[serde(default)]
    script_source_lines: Vec<SourceLineJson>,
}

#[derive(Deserialize)]
struct SourceLineJson {
    #[serde(default)]
    source: String,
    #[serde(default)]
    line: u32,
}

impl KubejsWeb {
    /// Cliente com tempos curtos (o servidor é local) e sem novas tentativas.
    ///
    /// # Errors
    ///
    /// [`Error::Internal`] se o cliente HTTP não pôde ser montado.
    pub fn new(config: WebServerConfig, version: &str) -> Result<Self> {
        let mut http = HttpConfig::for_version(version);
        http.connect_timeout = Duration::from_secs(1);
        http.request_timeout = Duration::from_secs(10);
        http.read_timeout = Duration::from_secs(10);
        http.max_retries = 0;
        http.max_body_bytes = 16 * 1024 * 1024;
        let http = HttpClient::new(http).map_err(|error| Error::Internal(error.to_string()))?;
        Ok(Self { http, config })
    }

    fn url(port: u16, path: &str) -> Result<Url> {
        Url::parse(&format!("http://127.0.0.1:{port}{path}"))
            .map_err(|error| Error::Internal(error.to_string()))
    }

    /// A porta em que o KubeJS responde (a configurada ou uma das seguintes).
    async fn port(&self) -> Result<u16> {
        let first = self.config.port;
        let last = first.saturating_add(PORT_ATTEMPTS - 1);
        for port in first..=last {
            let Ok(url) = Self::url(port, "/") else {
                continue;
            };
            let Ok(response) = self.http.get(url).idempotent(false).send().await else {
                continue;
            };
            let Ok(body) = response.bytes_limited(4096).await else {
                continue;
            };
            if body.starts_with(HOMEPAGE_PREFIX.as_bytes()) {
                return Ok(port);
            }
        }
        Err(Error::WebServerUnavailable { first, last })
    }

    fn bearer(&self) -> SecretString {
        SecretString::from(format!("Bearer {}", self.config.token.expose_secret()))
    }

    /// `POST /api/reload/<tipo>` (servidor ou startup). Os scripts de cliente não têm rota.
    ///
    /// # Errors
    ///
    /// [`Error::WebServerUnavailable`] se o KubeJS não responde e [`Error::WebServerRejected`]
    /// se a rota respondeu com erro (token recusado, por exemplo).
    pub async fn reload(&self, script_type: KubejsScriptType) -> Result<()> {
        let path = format!("/api/reload/{}", script_type.name());
        let port = self.port().await?;
        let request = self
            .http
            .post_json(Self::url(port, &path)?, &serde_json::json!({}))
            .map_err(|error| Error::Internal(error.to_string()))?
            .secret_header(HeaderName::from_static("authorization"), &self.bearer())
            .map_err(|error| Error::Internal(error.to_string()))?;
        request
            .send()
            .await
            .map_err(|error| rejected(&path, &error))?;
        Ok(())
    }

    /// Os erros e avisos dos scripts de servidor e de startup, ao vivo.
    ///
    /// # Errors
    ///
    /// Os mesmos de [`KubejsWeb::reload`].
    pub async fn problems(&self) -> Result<Vec<ScriptProblem>> {
        let port = self.port().await?;
        let mut problems = Vec::new();
        for script_type in [KubejsScriptType::Startup, KubejsScriptType::Server] {
            for (kind, level) in [
                ("errors", ProblemLevel::Error),
                ("warnings", ProblemLevel::Warning),
            ] {
                let path = format!("/api/console/{}/{kind}", script_type.name());
                let request = self
                    .http
                    .get(Self::url(port, &path)?)
                    .secret_header(HeaderName::from_static("authorization"), &self.bearer())
                    .map_err(|error| Error::Internal(error.to_string()))?;
                let lines: Vec<ConsoleLineJson> = request
                    .json()
                    .await
                    .map_err(|error| rejected(&path, &error))?;
                problems.extend(
                    lines
                        .into_iter()
                        .map(|line| live_problem(line, script_type, level)),
                );
            }
        }
        Ok(problems)
    }

    /// Os IDs de itens, blocos e tags do jogo aberto (rotas sem token).
    ///
    /// # Errors
    ///
    /// Os mesmos de [`KubejsWeb::reload`].
    pub async fn ids(&self) -> Result<IdGroup> {
        let port = self.port().await?;
        let fetch = |path: &'static str| async move {
            self.http
                .get(Self::url(port, path)?)
                .json::<Vec<String>>()
                .await
                .map_err(|error| rejected(path, &error))
        };
        let mut group = IdGroup {
            source: LIVE_SOURCE.to_owned(),
            items: fetch("/api/registries/minecraft/item/keys").await?,
            blocks: fetch("/api/registries/minecraft/block/keys").await?,
            item_tags: fetch("/api/tags/minecraft/item").await?,
            block_tags: fetch("/api/tags/minecraft/block").await?,
            fluid_tags: fetch("/api/tags/minecraft/fluid").await.unwrap_or_default(),
            recipes: Vec::new(),
        };
        for list in [
            &mut group.items,
            &mut group.blocks,
            &mut group.item_tags,
            &mut group.block_tags,
            &mut group.fluid_tags,
        ] {
            list.sort();
            list.dedup();
        }
        Ok(group)
    }
}

/// O erro HTTP sem nada do pedido além da rota (status 0: sem resposta ou resposta ilegível).
fn rejected(path: &str, error: &warden_http::Error) -> Error {
    let status = match error {
        warden_http::Error::UnexpectedStatus { status, .. }
        | warden_http::Error::NotFound { status, .. }
        | warden_http::Error::ServerError { status, .. } => *status,
        _ => 0,
    };
    Error::WebServerRejected {
        path: path.to_owned(),
        status,
    }
}

/// Uma linha do console ao vivo como item do painel.
fn live_problem(
    line: ConsoleLineJson,
    script_type: KubejsScriptType,
    level: ProblemLevel,
) -> ScriptProblem {
    let source = line
        .script_source_lines
        .iter()
        .find(|source| !source.source.is_empty() && source.line > 0);
    let file =
        source.and_then(|source| crate::problems::kubejs_file_for(&source.source, script_type));
    let number = source.map(|source| source.line);
    // O texto do KubeJS já começa com `arquivo#linha: `.
    let message = source
        .map(|source| format!("{}#{}: ", source.source, source.line))
        .and_then(|prefix| line.message.strip_prefix(&prefix).map(ToOwned::to_owned))
        .unwrap_or_else(|| line.message.clone());
    ScriptProblem {
        level,
        file,
        line: number,
        column: None,
        message,
        log_line: line.message,
        log_file: None,
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write as _;
    use std::sync::{Arc, Mutex};

    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;

    const TOKEN: &str = "c2VncmVkby1kby1rdWJlanMtcGFyYS10ZXN0ZXM_";

    fn port_of(server: &MockServer) -> u16 {
        server.address().port()
    }

    async fn kubejs_server() -> MockServer {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string("KubeJS Local Web Server [Minecraft]\n2026-10-13\n"),
            )
            .mount(&server)
            .await;
        server
    }

    fn client(port: u16) -> KubejsWeb {
        KubejsWeb::new(
            WebServerConfig::new(port, SecretString::from(TOKEN.to_owned())),
            "0.0.0-teste",
        )
        .unwrap()
    }

    /// Guarda tudo que os registros escreveram (para a varredura do token).
    #[derive(Clone, Default)]
    struct Captured(Arc<Mutex<Vec<u8>>>);

    impl std::io::Write for Captured {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for Captured {
        type Writer = Self;
        fn make_writer(&'a self) -> Self::Writer {
            self.clone()
        }
    }

    #[test]
    fn le_a_configuracao_da_instancia() {
        let dir = tempfile::tempdir().unwrap();
        assert!(read_web_server_config(dir.path()).is_none());
        let file = dir.path().join(WEB_SERVER_FILE);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(
            &file,
            format!(r#"{{"enabled":true,"port":61500,"public_address":"","auth":"{TOKEN}"}}"#),
        )
        .unwrap();
        let config = read_web_server_config(dir.path()).unwrap();
        assert_eq!(config.port, 61500);
        assert!(!format!("{config:?}").contains(TOKEN));
        std::fs::write(&file, format!(r#"{{"auth":"{TOKEN}"}}"#)).unwrap();
        assert_eq!(
            read_web_server_config(dir.path()).unwrap().port,
            DEFAULT_PORT
        );
        std::fs::write(&file, format!(r#"{{"enabled":false,"auth":"{TOKEN}"}}"#)).unwrap();
        assert!(read_web_server_config(dir.path()).is_none());
        std::fs::write(&file, r#"{"enabled":true}"#).unwrap();
        assert!(read_web_server_config(dir.path()).is_none());
        std::fs::write(&file, "{").unwrap();
        assert!(read_web_server_config(dir.path()).is_none());
    }

    #[tokio::test]
    async fn recarrega_com_o_token_e_sem_registrar_o_token() {
        let server = kubejs_server().await;
        Mock::given(method("POST"))
            .and(path("/api/reload/server"))
            .and(header("authorization", format!("Bearer {TOKEN}").as_str()))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&server)
            .await;

        let captured = Captured::default();
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::TRACE)
            .with_writer(captured.clone())
            .with_ansi(false)
            .finish();
        let web = client(port_of(&server));
        let guard = tracing::subscriber::set_default(subscriber);
        web.reload(KubejsScriptType::Server).await.unwrap();
        tracing::debug!(?web, "cliente usado");
        drop(guard);

        let logs = String::from_utf8(captured.0.lock().unwrap().clone()).unwrap();
        assert!(logs.contains("cliente usado"), "{logs}");
        assert!(!logs.contains(TOKEN), "o token apareceu nos registros");
        assert!(!format!("{web:?}").contains(TOKEN));
        server.verify().await;
    }

    #[tokio::test]
    async fn token_recusado_vira_erro_sem_o_token() {
        let server = kubejs_server().await;
        Mock::given(method("POST"))
            .and(path("/api/reload/startup"))
            .respond_with(
                ResponseTemplate::new(403).set_body_string("Authorization header does not match"),
            )
            .mount(&server)
            .await;
        let error = client(port_of(&server))
            .reload(KubejsScriptType::Startup)
            .await
            .unwrap_err();
        assert!(matches!(
            &error,
            Error::WebServerRejected { status: 403, path } if path == "/api/reload/startup"
        ));
        let text = format!("{error} {error:?} {}", warden_core::error_chain(&error));
        assert!(!text.contains(TOKEN));
    }

    #[tokio::test]
    async fn outro_programa_na_porta_nao_recebe_o_token() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_string("outro programa"))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200))
            .expect(0)
            .mount(&server)
            .await;
        // Só a porta do servidor simulado e as seguintes (fechadas ou de outros programas).
        let error = client(port_of(&server))
            .reload(KubejsScriptType::Server)
            .await
            .unwrap_err();
        assert!(matches!(error, Error::WebServerUnavailable { .. }));
        server.verify().await;
    }

    #[tokio::test]
    async fn erros_ao_vivo_e_ids_do_jogo() {
        let server = kubejs_server().await;
        let bearer = format!("Bearer {TOKEN}");
        Mock::given(method("GET"))
            .and(path("/api/console/server/errors"))
            .and(header("authorization", bearer.as_str()))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([{
                "type": "error",
                "message": "receitas.js#16: Failed to create recipe for type 'kubejs:shapeless': ItemStack 'result' can't be empty!",
                "timestamp": 1,
                "script_source_lines": [{"source": "receitas.js", "line": 16}],
                "all_source_lines": [{"source": "receitas.js", "line": 16}],
                "stack_trace": []
            }])))
            .mount(&server)
            .await;
        for route in [
            "/api/console/server/warnings",
            "/api/console/startup/errors",
            "/api/console/startup/warnings",
        ] {
            Mock::given(method("GET"))
                .and(path(route))
                .and(header("authorization", bearer.as_str()))
                .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
                .mount(&server)
                .await;
        }
        let web = client(port_of(&server));
        let problems = web.problems().await.unwrap();
        assert_eq!(problems.len(), 1);
        assert_eq!(
            problems[0].file.as_deref(),
            Some("kubejs/server_scripts/receitas.js")
        );
        assert_eq!(problems[0].line, Some(16));
        assert!(problems[0].message.starts_with("Failed to create recipe"));
        assert_eq!(problems[0].log_file, None);

        for (route, body) in [
            (
                "/api/registries/minecraft/item/keys",
                serde_json::json!(["minecraft:stone", "minecraft:copper_ingot"]),
            ),
            (
                "/api/registries/minecraft/block/keys",
                serde_json::json!(["minecraft:stone"]),
            ),
            (
                "/api/tags/minecraft/item",
                serde_json::json!(["c:ingots/copper"]),
            ),
            (
                "/api/tags/minecraft/block",
                serde_json::json!(["minecraft:mineable/pickaxe"]),
            ),
        ] {
            Mock::given(method("GET"))
                .and(path(route))
                .respond_with(ResponseTemplate::new(200).set_body_json(body))
                .mount(&server)
                .await;
        }
        let ids = web.ids().await.unwrap();
        assert_eq!(ids.source, LIVE_SOURCE);
        assert_eq!(ids.items, ["minecraft:copper_ingot", "minecraft:stone"]);
        assert_eq!(ids.item_tags, ["c:ingots/copper"]);
        assert!(ids.fluid_tags.is_empty());
        let _ = std::io::stderr().flush();
    }
}
