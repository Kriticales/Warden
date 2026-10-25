//! O laço contra o servidor simulado da Anthropic, sem rede (ROADMAP D-17 "Verificação"):
//! chamadas paralelas, `responder`, 8 rodadas e a reserva, novas tentativas, conferência das
//! evidências e os blocos de raciocínio devolvidos intactos (CA-T14-23, parte da Anthropic).
//!
//! O simulado (`wiremock` com um `Respond` próprio) faz o que a API Messages faz com o Sonnet 5.5
//! e o Opus 5.5 nos pontos que importam:
//! - exige a chave no cabeçalho `x-api-key` (401 sem ela), nunca na URL, e o `anthropic-version`;
//! - recusa a escolha forçada (`tool_choice` `any` ou `tool`) com o 400 da API real;
//! - cada mensagem do assistente tem de ser **idêntica** a uma resposta que ele mandou (bloco de
//!   raciocínio alterado dá 400 de assinatura inválida), e `system` e `tools` não podem mudar
//!   depois que há raciocínio no histórico (a API invalida os blocos);
//! - depois de um `tool_use` vem uma mensagem do usuário que começa com um `tool_result` para
//!   cada `id`; os papéis se alternam;
//! - a reserva pede `output_config.format` com objetos fechados.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use secrecy::SecretString;
use serde_json::{Value, json};
use support::{ValeSombrio, simple_answer};
use warden_ai::conversation::{AnswerVia, Conversation};
use warden_ai::engine::{Engine, Limits, LiveStatus, Observer};
use warden_ai::evidence::ClaimStatus;
use warden_ai::providers::anthropic::AnthropicClient;
use warden_ai::providers::{AiProvider, Message, Provider, SearchMode};
use warden_ai::schema::ROUND_LIMIT_MESSAGE;
use warden_ai::tools::{AiPermissions, ToolRegistry};
use warden_ai::{AiErrorCode, Error};
use warden_core::{CancellationToken, DomainCode, DomainError};
use warden_http::{HttpClient, HttpConfig, ManualTimer};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

const KEY: &str = "chave-de-teste-da-anthropic-simulada";

/// Uma resposta do roteiro: código, corpo e o `retry-after` (segundos), se houver.
struct Out {
    status: u16,
    body: Value,
    retry_after: Option<u64>,
}

type Script = Box<dyn Fn(usize, &Value) -> Out + Send + Sync>;

#[derive(Default)]
struct State {
    emitted: Vec<Value>,
    /// `system` e `tools` do primeiro pedido (o prefixo a que o raciocínio fica preso).
    prefix: Option<(Value, Value)>,
    raw: Vec<String>,
    requests: Vec<Value>,
    count_tokens: Vec<Value>,
    refusals: Vec<String>,
    urls: Vec<String>,
}

struct Simulated {
    server: MockServer,
    state: Arc<Mutex<State>>,
}

impl Simulated {
    async fn start(script: Script) -> Self {
        let server = MockServer::start().await;
        let state = Arc::new(Mutex::new(State::default()));
        Mock::given(wiremock::matchers::any())
            .respond_with(Responder {
                state: Arc::clone(&state),
                script,
                count: AtomicUsize::new(0),
            })
            .mount(&server)
            .await;
        Self { server, state }
    }

    fn refusals(&self) -> Vec<String> {
        self.state.lock().unwrap().refusals.clone()
    }

    fn requests(&self) -> Vec<Value> {
        self.state.lock().unwrap().requests.clone()
    }

    fn raw(&self) -> Vec<String> {
        self.state.lock().unwrap().raw.clone()
    }

    fn client(&self, timer: &ManualTimer) -> AnthropicClient {
        let http =
            HttpClient::with_timer(HttpConfig::for_version("9.9.9"), Arc::new(timer.clone()))
                .unwrap();
        AnthropicClient::with_base_url(http, &self.server.uri(), SecretString::from(KEY)).unwrap()
    }
}

struct Responder {
    state: Arc<Mutex<State>>,
    script: Script,
    count: AtomicUsize,
}

fn error(status: u16, kind: &str, message: &str) -> ResponseTemplate {
    ResponseTemplate::new(status)
        .set_body_json(json!({ "type": "error", "error": { "type": kind, "message": message } }))
}

/// A lista real da chave do dono em 18/10/2026, em duas páginas.
const MODELS: [&str; 13] = [
    "claude-fable-5",
    "claude-fable-5-1",
    "claude-haiku-4-5-20251001",
    "claude-haiku-5-5",
    "claude-opus-4-5-20251101",
    "claude-opus-4-6",
    "claude-opus-4-7",
    "claude-opus-4-8",
    "claude-opus-5",
    "claude-opus-5-5",
    "claude-sonnet-4-6",
    "claude-sonnet-5",
    "claude-sonnet-5-5",
];

impl Respond for Responder {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let url = request.url.to_string();
        let mut state = self.state.lock().unwrap();
        state.urls.push(url.clone());
        let header = |name: &str| {
            request
                .headers
                .get(name)
                .and_then(|value| value.to_str().ok())
                .map(ToOwned::to_owned)
        };
        if url.contains(KEY) || header("x-api-key").as_deref() != Some(KEY) {
            state
                .refusals
                .push("chave ausente, errada ou na URL".into());
            return error(401, "authentication_error", "invalid x-api-key");
        }
        if header("anthropic-version").as_deref() != Some("2023-06-01") {
            state.refusals.push("anthropic-version".into());
            return error(
                400,
                "invalid_request_error",
                "anthropic-version: header is required",
            );
        }
        if request.method.as_str() == "GET" && request.url.path() == "/v1/models" {
            let second = request.url.query().is_some_and(|q| q.contains("after_id="));
            let (page, more) = if second {
                (&MODELS[7..], false)
            } else {
                (&MODELS[..7], true)
            };
            let data: Vec<Value> = page
                .iter()
                .map(|id| json!({ "type": "model", "id": id, "display_name": id }))
                .collect();
            return ResponseTemplate::new(200)
                .set_body_json(json!({ "data": data, "has_more": more,
                "first_id": page[0], "last_id": page[page.len() - 1] }));
        }
        let Ok(body) = serde_json::from_slice::<Value>(&request.body) else {
            return error(400, "invalid_request_error", "invalid JSON");
        };
        if request.url.path() == "/v1/messages/count_tokens" {
            state.count_tokens.push(body);
            let size = request.body.len() / 4;
            return ResponseTemplate::new(200).set_body_json(json!({ "input_tokens": size }));
        }
        if request.url.path() != "/v1/messages" {
            return error(404, "not_found_error", "Not found");
        }
        let State {
            emitted, prefix, ..
        } = &mut *state;
        if let Err(message) = validate(&body, emitted, prefix) {
            state.refusals.push(message.clone());
            return error(400, "invalid_request_error", &message);
        }
        state.requests.push(body.clone());
        state
            .raw
            .push(String::from_utf8(request.body.clone()).unwrap());
        drop(state);
        let n = self.count.fetch_add(1, Ordering::SeqCst);
        let out = (self.script)(n, &body);
        if out.status == 200
            && let Some(content) = out.body.get("content")
        {
            self.state
                .lock()
                .unwrap()
                .emitted
                .push(json!({ "role": "assistant", "content": content }));
        }
        let mut template = ResponseTemplate::new(out.status).set_body_json(out.body);
        if let Some(seconds) = out.retry_after {
            template = template.insert_header("retry-after", seconds.to_string().as_str());
        }
        template
    }
}

fn all_closed(schema: &Value) -> bool {
    match schema {
        Value::Object(map) => {
            (map.get("type") != Some(&json!("object"))
                || map.get("additionalProperties") == Some(&json!(false)))
                && map.values().all(all_closed)
        }
        Value::Array(list) => list.iter().all(all_closed),
        _ => true,
    }
}

/// As regras da API Messages com o Sonnet 5.5 e o Opus 5.5 (10/2026).
fn validate(
    request: &Value,
    emitted: &[Value],
    prefix: &mut Option<(Value, Value)>,
) -> Result<(), String> {
    let choice = request.pointer("/tool_choice/type").and_then(Value::as_str);
    if matches!(choice, Some("any" | "tool")) {
        return Err(
            "tool_choice: type \"tool\" and \"any\" are not supported for this model.".into(),
        );
    }
    if request.get("max_tokens").and_then(Value::as_u64).is_none() {
        return Err("max_tokens: Field required".into());
    }
    if let Some(schema) = request.pointer("/output_config/format/schema")
        && !all_closed(schema)
    {
        return Err("output_config.format.schema: additionalProperties must be false".into());
    }
    let messages = request
        .get("messages")
        .and_then(Value::as_array)
        .ok_or("messages: Field required")?;
    let current = (
        request.get("system").cloned().unwrap_or(Value::Null),
        request.get("tools").cloned().unwrap_or(Value::Null),
    );
    let has_thinking = messages.iter().any(|m| {
        m["role"] == "assistant"
            && m["content"]
                .as_array()
                .is_some_and(|blocks| blocks.iter().any(|b| b["type"] == "thinking"))
    });
    match prefix {
        Some(first) if has_thinking && *first != current => {
            return Err("messages.1.content.0: Invalid `signature` in `thinking` block. The block is bound to a different conversation.".into());
        }
        None => *prefix = Some(current),
        _ => {}
    }
    for (i, message) in messages.iter().enumerate() {
        let role = message["role"].as_str().unwrap_or("");
        let expected = if i % 2 == 0 { "user" } else { "assistant" };
        if role != expected {
            return Err(format!(
                "messages.{i}: roles must alternate between \"user\" and \"assistant\""
            ));
        }
        if role != "assistant" {
            continue;
        }
        if !emitted.contains(message) {
            return Err(format!(
                "messages.{i}.content.0: Invalid `signature` in `thinking` block"
            ));
        }
        let ids: Vec<&Value> = message["content"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|b| b["type"] == "tool_use")
            .map(|b| &b["id"])
            .collect();
        if ids.is_empty() {
            continue;
        }
        let next = messages.get(i + 1).and_then(|m| m["content"].as_array());
        let results: Vec<&Value> = next
            .map(|blocks| {
                blocks
                    .iter()
                    .take_while(|b| b["type"] == "tool_result")
                    .map(|b| &b["tool_use_id"])
                    .collect()
            })
            .unwrap_or_default();
        if ids.iter().any(|id| !results.contains(id)) {
            return Err(format!(
                "messages.{i}: `tool_use` ids were found without `tool_result` blocks immediately after"
            ));
        }
    }
    Ok(())
}

// ---------- Respostas do roteiro ----------

static COUNTER: AtomicUsize = AtomicUsize::new(1);

fn next() -> usize {
    COUNTER.fetch_add(1, Ordering::Relaxed)
}

fn thinking() -> Value {
    json!({ "type": "thinking", "thinking": "",
        "signature": format!("RXJBbVVtQ2FzbzAwMXNpbXVsYWRv{}", next()) })
}

fn ok(content: Vec<Value>, stop: &str) -> Out {
    Out {
        status: 200,
        body: json!({
            "id": format!("msg_{}", next()), "type": "message", "role": "assistant",
            "model": "claude-sonnet-5-5", "content": Value::Array(content), "stop_reason": stop, "stop_sequence": null,
            "usage": { "input_tokens": 40, "cache_creation_input_tokens": 0, "cache_read_input_tokens": 2960, "output_tokens": 40 }
        }),
        retry_after: None,
    }
}

/// Chamadas paralelas, depois do bloco de raciocínio.
fn calls(list: &[(&str, Value)]) -> Out {
    let mut content = vec![thinking()];
    for (name, input) in list {
        content.push(json!({ "type": "tool_use", "id": format!("toolu_{}", next()), "name": name, "input": input }));
    }
    ok(content, "tool_use")
}

fn respond(answer: &Value) -> Out {
    calls(&[("responder", answer.clone())])
}

fn text(text: &str) -> Out {
    ok(
        vec![thinking(), json!({ "type": "text", "text": text })],
        "end_turn",
    )
}

fn api_error(status: u16, kind: &str, message: &str, retry_after: Option<u64>) -> Out {
    Out {
        status,
        body: json!({ "type": "error", "error": { "type": kind, "message": message } }),
        retry_after,
    }
}

// ---------- Montagem ----------

#[derive(Default)]
struct Watch {
    lives: Mutex<Vec<LiveStatus>>,
}

#[async_trait]
impl Observer for Watch {
    async fn changed(&self, _conversation: &Conversation) {}
    fn live(&self, live: LiveStatus) {
        self.lives.lock().unwrap().push(live);
    }
}

struct Setup {
    claude: Simulated,
    timer: ManualTimer,
    client: AnthropicClient,
    registry: ToolRegistry,
    source: ValeSombrio,
    watch: Watch,
}

impl Setup {
    async fn new(script: Script) -> Self {
        let claude = Simulated::start(script).await;
        let timer = ManualTimer::new();
        let client = claude.client(&timer);
        Self {
            claude,
            timer,
            client,
            registry: ToolRegistry::new(),
            source: ValeSombrio::default(),
            watch: Watch::default(),
        }
    }

    fn engine(&self) -> Engine<'_> {
        Engine {
            provider: &self.client,
            registry: &self.registry,
            source: &self.source,
            observer: &self.watch,
            timer: Arc::new(self.timer.clone()),
            cancel: CancellationToken::new(),
            limits: Limits::default(),
        }
    }

    fn sleeps(&self) -> Vec<u64> {
        self.timer.sleeps().iter().map(Duration::as_secs).collect()
    }
}

fn conversation() -> Conversation {
    let mut conv = support::conversation(AiPermissions::default());
    conv.provider = AiProvider::Anthropic;
    conv.model = "claude-sonnet-5-5".into();
    conv
}

// ---------- Testes ----------

#[tokio::test]
async fn paralelas_malformada_e_resposta_pela_responder() {
    let setup = Setup::new(Box::new(|n, _| match n {
        0 => calls(&[
            ("get_findings", json!({})),
            ("get_crash_report", json!({ "session_id": "s3" })),
        ]),
        2 => calls(&[("get_mod_details", json!({ "mod_id": "railways" }))]),
        // Sem conteúdo: malformada.
        1 => ok(vec![], "end_turn"),
        _ => respond(&simple_answer()),
    }))
    .await;
    let mut conv = conversation();
    setup
        .engine()
        .ask(&mut conv, "Por que travou?".into())
        .await
        .unwrap();
    assert!(
        setup.claude.refusals().is_empty(),
        "{:?}",
        setup.claude.refusals()
    );

    let turn = &conv.turns[0];
    let answer = turn.answer.as_ref().unwrap();
    assert_eq!(answer.via, AnswerVia::Respond);
    assert_eq!(answer.checked.claims[0].status, ClaimStatus::Verified);
    assert_eq!(turn.steps.len(), 3);
    assert_eq!(conv.requests, 4);
    assert_eq!(conv.usage.cached, 4 * 2960);
    assert_eq!(conv.usage.prompt, 4 * 3000);

    let requests = setup.claude.requests();
    for request in &requests {
        assert_eq!(request["tool_choice"], json!({ "type": "auto" }));
        assert_eq!(request["tools"], requests[0]["tools"], "tools fixo");
        assert_eq!(request["system"], requests[0]["system"], "system fixo");
        assert!(request.get("output_config").is_none());
    }
    assert_eq!(requests[0]["tools"].as_array().unwrap().len(), 18);
    // A malformada foi repetida sem mexer no histórico.
    assert_eq!(requests[1]["messages"], requests[2]["messages"]);
    // Os dois resultados numa mensagem só, com o id de cada chamada.
    let messages = requests[1]["messages"].as_array().unwrap();
    let uses: Vec<&Value> = messages[1]["content"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|b| b["type"] == "tool_use")
        .map(|b| &b["id"])
        .collect();
    let results: Vec<&Value> = messages[2]["content"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| &b["tool_use_id"])
        .collect();
    assert_eq!(uses, results);
    assert!(
        setup
            .claude
            .state
            .lock()
            .unwrap()
            .urls
            .iter()
            .all(|u| !u.contains(KEY))
    );
}

#[tokio::test]
async fn sem_escalonamento_por_etapa_mas_com_as_permissoes() {
    // Com `auto` e as ferramentas fixas, o Claude pode ler uma config já na 1.ª rodada (no Gemini
    // ela só entra depois de um suspeito); a permissão de configs continua valendo.
    let setup = Setup::new(Box::new(|n, _| match n {
        0 => calls(&[(
            "read_config",
            json!({ "path": "config/create-common.toml" }),
        )]),
        _ => respond(&simple_answer()),
    }))
    .await;
    let mut conv = conversation();
    conv.permissions = AiPermissions {
        configs: false,
        github: false,
    };
    setup.engine().ask(&mut conv, "?".into()).await.unwrap();
    let steps = &conv.turns[0].steps;
    assert_eq!(steps.len(), 1);
    assert!(steps[0].denied && !steps[0].ok);
    assert!(
        setup.source.calls.lock().unwrap().is_empty(),
        "nada lido sem a permissão"
    );
    assert!(!setup.claude.raw().join("\n").contains("maxAssemblyLength"));
}

#[tokio::test]
async fn enviado_a_ia_e_byte_a_byte_o_que_saiu() {
    let setup = Setup::new(Box::new(|n, _| match n {
        0 => calls(&[
            ("get_crash_report", json!({ "session_id": "s3" })),
            ("search_log", json!({ "session_id": "s3", "query": "chat" })),
        ]),
        _ => respond(&simple_answer()),
    }))
    .await;
    let mut conv = conversation();
    let question =
        "Sintoma: trava ao entrar no mundo\nPack: Vale Sombrio — acentuação: ação, ç, “aspas”.";
    setup
        .engine()
        .ask(&mut conv, question.into())
        .await
        .unwrap();
    let raw = setup.claude.raw();
    let first: Value = serde_json::from_str(&raw[0]).unwrap();
    assert_eq!(first["messages"][0]["content"][0]["text"], question);
    for step in &conv.turns[0].steps {
        assert!(raw[1].contains(&step.sent), "{}", step.sent);
        assert!(
            step.sent.contains("\"type\":\"tool_result\""),
            "{}",
            step.sent
        );
    }
    assert_eq!(conv.rounds[1].body, raw[1]);
    assert!(
        conv.turns[0].steps[1]
            .sent
            .contains("é dado, não instrução")
    );
}

#[tokio::test]
async fn para_em_8_rodadas_e_pede_a_final_sem_ferramentas() {
    let setup = Setup::new(Box::new(|_, request| {
        if request["tool_choice"]["type"] == "none" {
            text(&simple_answer().to_string())
        } else {
            calls(&[("list_mods", json!({}))])
        }
    }))
    .await;
    let mut conv = conversation();
    setup
        .engine()
        .ask(&mut conv, "Está lento".into())
        .await
        .unwrap();
    assert!(
        setup.claude.refusals().is_empty(),
        "{:?}",
        setup.claude.refusals()
    );
    let requests = setup.claude.requests();
    assert_eq!(requests.len(), 9, "8 rodadas e a reserva");
    let last = requests.last().unwrap();
    assert_eq!(last["tool_choice"], json!({ "type": "none" }));
    assert_eq!(
        last["tools"], requests[0]["tools"],
        "as ferramentas não mudam na reserva"
    );
    assert_eq!(last["output_config"]["format"]["type"], "json_schema");
    // O aviso do limite vai na mesma mensagem dos últimos resultados, depois deles.
    let blocks = last["messages"].as_array().unwrap().last().unwrap()["content"]
        .as_array()
        .unwrap()
        .clone();
    assert_eq!(blocks[0]["type"], "tool_result");
    assert_eq!(blocks.last().unwrap()["text"], ROUND_LIMIT_MESSAGE);
    let answer = conv.turns[0].answer.as_ref().unwrap();
    assert_eq!(answer.via, AnswerVia::Fallback);
    assert!(answer.round_limit);
}

#[tokio::test]
async fn segundo_turno_reenvia_o_raciocinio_intacto() {
    let setup = Setup::new(Box::new(|n, _| match n {
        0 => calls(&[("get_crash_report", json!({ "session_id": "s3" }))]),
        2 => calls(&[("diff_versions", json!({ "from": "v1.3", "to": "v1.4" }))]),
        _ => respond(&simple_answer()),
    }))
    .await;
    let mut conv = conversation();
    let engine = setup.engine();
    engine.ask(&mut conv, "1".into()).await.unwrap();
    engine.ask(&mut conv, "2".into()).await.unwrap();
    assert!(
        setup.claude.refusals().is_empty(),
        "{:?}",
        setup.claude.refusals()
    );
    let requests = setup.claude.requests();
    let emitted = setup.claude.state.lock().unwrap().emitted.clone();
    let messages = requests[3]["messages"].as_array().unwrap();
    assert_eq!(messages[1], emitted[0], "o bloco de raciocínio volta igual");
    assert_eq!(messages[1]["content"][0]["type"], "thinking");
    // A 2.ª pergunta entra depois do "ok" da responder, na mesma mensagem.
    let second = requests[2]["messages"].as_array().unwrap();
    let last = second.last().unwrap()["content"].as_array().unwrap();
    assert_eq!(last[0]["type"], "tool_result");
    assert_eq!(last[1]["text"], "2");
}

#[tokio::test]
async fn simulado_recusa_raciocinio_alterado_escolha_forcada_e_prefixo_mudado() {
    let user = json!({ "role": "user", "content": [{ "type": "text", "text": "oi" }] });
    let assistant = json!({ "role": "assistant", "content": [
        { "type": "thinking", "thinking": "", "signature": "QUJDREVGR0g=" },
        { "type": "tool_use", "id": "toolu_1", "name": "list_mods", "input": {} }
    ]});
    let result = json!({ "role": "user", "content": [{ "type": "tool_result", "tool_use_id": "toolu_1", "content": "{}" }] });
    let request = |messages: Vec<Value>| json!({ "max_tokens": 10, "system": [], "tools": [], "messages": messages });
    let emitted = vec![assistant.clone()];
    let mut prefix = None;
    assert!(
        validate(
            &request(vec![user.clone(), assistant.clone(), result.clone()]),
            &emitted,
            &mut prefix
        )
        .is_ok()
    );
    // Assinatura alterada.
    let mut altered = assistant.clone();
    altered["content"][0]["signature"] = json!("QUJDREVGR0k=");
    let refused = validate(
        &request(vec![user.clone(), altered, result.clone()]),
        &emitted,
        &mut prefix,
    );
    assert!(refused.unwrap_err().contains("Invalid `signature`"));
    // Bloco de raciocínio tirado.
    let mut stripped = assistant.clone();
    stripped["content"].as_array_mut().unwrap().remove(0);
    assert!(
        validate(
            &request(vec![user.clone(), stripped, result.clone()]),
            &emitted,
            &mut prefix
        )
        .is_err()
    );
    // Sem o resultado logo depois.
    assert!(
        validate(
            &request(vec![user.clone(), assistant.clone(), user.clone()]),
            &emitted,
            &mut prefix
        )
        .is_err()
    );
    // Escolha forçada: o 400 do Sonnet 5.5 e do Opus 5.5.
    let mut forced = request(vec![user.clone()]);
    forced["tool_choice"] = json!({ "type": "any" });
    assert!(
        validate(&forced, &emitted, &mut prefix)
            .unwrap_err()
            .contains("not supported")
    );
    // `tools` mudou com raciocínio no histórico.
    let mut changed = request(vec![user, assistant, result]);
    changed["tools"] = json!([{ "name": "outra" }]);
    assert!(
        validate(&changed, &emitted, &mut prefix)
            .unwrap_err()
            .contains("different conversation")
    );
}

#[tokio::test]
async fn texto_leva_a_reserva_sem_ferramentas() {
    let setup = Setup::new(Box::new(|n, request| match n {
        0 => calls(&[("get_crash_report", json!({ "session_id": "s3" }))]),
        1 => text("O travamento é do Steam 'n' Rails. Atualize."),
        _ => {
            assert_eq!(request["tool_choice"]["type"], "none");
            text(&simple_answer().to_string())
        }
    }))
    .await;
    let mut conv = conversation();
    setup.engine().ask(&mut conv, "?".into()).await.unwrap();
    let requests = setup.claude.requests();
    assert_eq!(requests.len(), 3);
    assert_eq!(
        requests[1]["messages"], requests[2]["messages"],
        "o texto não entrou no histórico"
    );
    let answer = conv.turns[0].answer.as_ref().unwrap();
    assert_eq!(answer.via, AnswerVia::Fallback);
    assert!(!answer.round_limit);
}

#[tokio::test]
async fn sobrecarga_529_ate_4_vezes_e_depois_erro() {
    let setup = Setup::new(Box::new(|n, _| match n {
        0..=2 => api_error(529, "overloaded_error", "Overloaded", None),
        3 => api_error(500, "api_error", "Internal server error", None),
        4 => calls(&[("get_findings", json!({}))]),
        _ => respond(&simple_answer()),
    }))
    .await;
    let mut conv = conversation();
    setup.engine().ask(&mut conv, "?".into()).await.unwrap();
    assert_eq!(setup.sleeps(), [2, 4, 8, 16]);
    assert!(
        setup
            .watch
            .lives
            .lock()
            .unwrap()
            .contains(&LiveStatus::Retrying { attempt: 4, of: 4 })
    );

    let setup = Setup::new(Box::new(|_, _| {
        api_error(529, "overloaded_error", "Overloaded", None)
    }))
    .await;
    let mut conv = conversation();
    let error = setup.engine().ask(&mut conv, "?".into()).await.unwrap_err();
    assert_eq!(error.code(), DomainCode::Domain(AiErrorCode::Unavailable));
    assert_eq!(error.params()["status"], "529");
    assert_eq!(setup.claude.requests().len(), 5);
}

#[tokio::test]
async fn limite_por_minuto_espera_o_retry_after() {
    let setup = Setup::new(Box::new(|n, _| match n {
        0 => api_error(429, "rate_limit_error", "rate limit", Some(20)),
        1 => calls(&[("get_findings", json!({}))]),
        _ => respond(&simple_answer()),
    }))
    .await;
    let mut conv = conversation();
    setup.engine().ask(&mut conv, "?".into()).await.unwrap();
    assert_eq!(setup.sleeps(), [21]);
    assert!(
        setup
            .watch
            .lives
            .lock()
            .unwrap()
            .contains(&LiveStatus::WaitingPerMinute { seconds: 21 })
    );
}

#[tokio::test]
async fn espera_longa_cobranca_e_chave_param_na_hora() {
    let setup = Setup::new(Box::new(|_, _| {
        api_error(429, "rate_limit_error", "rate limit", Some(3600))
    }))
    .await;
    let mut conv = conversation();
    let error = setup.engine().ask(&mut conv, "?".into()).await.unwrap_err();
    assert_eq!(error.code(), DomainCode::Domain(AiErrorCode::QuotaExceeded));
    assert_eq!(setup.claude.requests().len(), 1, "nenhuma nova tentativa");
    assert!(setup.sleeps().is_empty());
    assert_eq!(
        conv.turns[0].failure.as_ref().unwrap().params["freeTier"],
        "false"
    );

    let setup = Setup::new(Box::new(|_, _| {
        api_error(
            400,
            "invalid_request_error",
            "Your credit balance is too low to access the Anthropic API.",
            None,
        )
    }))
    .await;
    let mut conv = conversation();
    let error = setup.engine().ask(&mut conv, "?".into()).await.unwrap_err();
    assert_eq!(error.code(), DomainCode::Domain(AiErrorCode::Billing));

    let claude = Simulated::start(Box::new(|_, _| unreachable!())).await;
    let http = HttpClient::new(HttpConfig::for_version("9.9.9")).unwrap();
    let wrong = AnthropicClient::with_base_url(
        http,
        &claude.server.uri(),
        SecretString::from("sk-ant-outra"),
    )
    .unwrap();
    let registry = ToolRegistry::new();
    let source = ValeSombrio::default();
    let watch = Watch::default();
    let engine = Engine {
        provider: &wrong,
        registry: &registry,
        source: &source,
        observer: &watch,
        timer: Arc::new(ManualTimer::new()),
        cancel: CancellationToken::new(),
        limits: Limits::default(),
    };
    let mut conv = conversation();
    let error = engine.ask(&mut conv, "?".into()).await.unwrap_err();
    assert_eq!(error.code(), DomainCode::Domain(AiErrorCode::KeyInvalid));
    assert!(matches!(
        wrong.models().await,
        Err(Error::KeyInvalid { status: 401 })
    ));
}

#[tokio::test]
async fn respostas_com_problema_viram_nova_tentativa_e_depois_erro() {
    let setup = Setup::new(Box::new(|n, _| match n {
        0 => ok(vec![], "end_turn"),
        1 => ok(
            vec![thinking(), json!({ "type": "text", "text": "  " })],
            "end_turn",
        ),
        _ => respond(&simple_answer()),
    }))
    .await;
    let mut conv = conversation();
    setup.engine().ask(&mut conv, "?".into()).await.unwrap();
    let requests = setup.claude.requests();
    assert_eq!(requests.len(), 3);
    assert_eq!(requests[0]["messages"], requests[2]["messages"]);

    let setup = Setup::new(Box::new(|_, _| ok(vec![], "end_turn"))).await;
    let mut conv = conversation();
    let error = setup.engine().ask(&mut conv, "?".into()).await.unwrap_err();
    assert_eq!(error.code(), DomainCode::Domain(AiErrorCode::BadResponse));
    assert_eq!(setup.claude.requests().len(), 3);
}

#[tokio::test]
async fn modelos_pela_regra_com_paginas_e_contagem_de_tokens() {
    let setup = Setup::new(Box::new(|_, _| unreachable!())).await;
    let choices = setup.client.models().await.unwrap();
    assert_eq!(choices.economical.as_deref(), Some("claude-sonnet-5-5"));
    assert_eq!(choices.careful.as_deref(), Some("claude-opus-5-5"));
    assert_eq!(setup.client.id(), AiProvider::Anthropic);

    let tokens = setup
        .engine()
        .count_tokens("claude-sonnet-5-5", &[Message::user("texto inicial")])
        .await
        .unwrap()
        .unwrap();
    assert!(tokens > 0);
    let sent = setup.claude.state.lock().unwrap().count_tokens.clone();
    assert_eq!(sent[0]["model"], "claude-sonnet-5-5");
    assert!(sent[0]["system"][0]["text"].is_string());
    assert_eq!(sent[0]["tools"].as_array().unwrap().len(), 18);
    assert_eq!(
        sent[0]["messages"][0]["content"][0]["text"],
        "texto inicial"
    );
    assert!(
        setup.claude.requests().is_empty(),
        "contar não gera resposta"
    );
}

#[tokio::test]
async fn cada_pergunta_usa_o_modelo_e_o_limite_do_seu_modo() {
    // ADR-0064: "Procurar rápido" faz menos consultas que "Procurar a fundo", e cada pergunta
    // sai no modelo do seu modo; a conversa guarda os dois por pergunta.
    let setup = Setup::new(Box::new(|_, request| {
        if request["tool_choice"]["type"] == "none" {
            text(&simple_answer().to_string())
        } else {
            calls(&[("list_mods", json!({}))])
        }
    }))
    .await;
    let mut conv = conversation();
    warden_ai::engine::push_question(&mut conv, "Está lento".into());
    conv.turns[0].mode = Some(SearchMode::Quick);
    conv.turns[0].model = Some("claude-sonnet-5-5".into());
    let quick = Engine {
        limits: Limits::for_mode(Some(SearchMode::Quick)),
        ..setup.engine()
    };
    quick.run(&mut conv).await.unwrap();
    let first = setup.claude.requests().len();
    assert_eq!(
        first,
        usize::try_from(SearchMode::Quick.max_rounds()).unwrap() + 1,
        "as rodadas do modo rápido e a reserva"
    );

    warden_ai::engine::push_question(&mut conv, "E a fundo?".into());
    conv.turns[1].mode = Some(SearchMode::Deep);
    conv.turns[1].model = Some("claude-opus-5-5".into());
    let deep = Engine {
        limits: Limits::for_mode(Some(SearchMode::Deep)),
        ..setup.engine()
    };
    deep.run(&mut conv).await.unwrap();
    assert!(
        setup.claude.refusals().is_empty(),
        "{:?}",
        setup.claude.refusals()
    );
    let requests = setup.claude.requests();
    assert_eq!(
        requests.len() - first,
        usize::try_from(SearchMode::Deep.max_rounds()).unwrap() + 1
    );
    assert!(
        requests[..first]
            .iter()
            .all(|r| r["model"] == "claude-sonnet-5-5")
    );
    assert!(
        requests[first..]
            .iter()
            .all(|r| r["model"] == "claude-opus-5-5")
    );
    assert_eq!(conv.turns[1].model_or(&conv.model), "claude-opus-5-5");
    assert_eq!(
        conv.model, "claude-sonnet-5-5",
        "o modelo da conversa não muda"
    );
}
