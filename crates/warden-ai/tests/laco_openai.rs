//! O laço contra o servidor simulado da OpenAI, sem rede (ROADMAP D-16 "Verificação"):
//! chamadas paralelas, `responder` forçada, 8 rodadas e a reserva, novas tentativas, conferência
//! das evidências e os itens de raciocínio devolvidos intactos (CA-T14-23, parte da OpenAI).
//!
//! O simulado (`wiremock` com um `Respond` próprio) faz o que a Responses API faz nos pontos que
//! importam:
//! - exige a chave no cabeçalho `Authorization: Bearer …` (401 sem ela) e nunca na URL;
//! - exige `store: false` e o raciocínio criptografado (`include`), como o Warden promete;
//! - cada item do modelo em `input` tem de vir na sequência inteira e **idêntica** a uma resposta
//!   que ele mandou (o `function_call` sem o `reasoning` dele, ou com o `encrypted_content`
//!   mudado, dá 400 como na API real), seguida de um `function_call_output` por chamada, com o
//!   mesmo `call_id`;
//! - com `allowed_tools` obrigatório, só responde chamadas dos nomes liberados.

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
use warden_ai::providers::openai::OpenAiClient;
use warden_ai::providers::{AiProvider, Provider};
use warden_ai::schema::ROUND_LIMIT_MESSAGE;
use warden_ai::tools::{AiPermissions, ToolRegistry};
use warden_ai::{AiErrorCode, Error};
use warden_core::{CancellationToken, DomainCode, DomainError};
use warden_http::{HttpClient, HttpConfig, ManualTimer};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

const KEY: &str = "chave-de-teste-da-openai-simulada";

/// Uma resposta do roteiro: código, corpo e o `retry-after` (segundos), se houver.
struct Out {
    status: u16,
    body: Value,
    retry_after: Option<u64>,
}

type Script = Box<dyn Fn(usize, &Value) -> Out + Send + Sync>;

#[derive(Default)]
struct State {
    emitted: Vec<Vec<Value>>,
    raw: Vec<String>,
    requests: Vec<Value>,
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

    fn client(&self, timer: &ManualTimer) -> OpenAiClient {
        let http =
            HttpClient::with_timer(HttpConfig::for_version("9.9.9"), Arc::new(timer.clone()))
                .unwrap();
        OpenAiClient::with_base_url(http, &self.server.uri(), SecretString::from(KEY)).unwrap()
    }
}

struct Responder {
    state: Arc<Mutex<State>>,
    script: Script,
    count: AtomicUsize,
}

fn error(status: u16, code: &str, message: &str) -> ResponseTemplate {
    ResponseTemplate::new(status).set_body_json(
        json!({ "error": { "message": message, "type": "invalid_request_error", "param": null, "code": code } }),
    )
}

impl Respond for Responder {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let url = request.url.to_string();
        let mut state = self.state.lock().unwrap();
        state.urls.push(url.clone());
        let auth = request
            .headers
            .get("authorization")
            .and_then(|value| value.to_str().ok());
        if url.contains(KEY) || auth != Some(&format!("Bearer {KEY}")) {
            state
                .refusals
                .push("chave ausente, errada ou na URL".into());
            return error(401, "invalid_api_key", "Incorrect API key provided.");
        }
        if request.method.as_str() == "GET" && url.ends_with("/v1/models") {
            let data: Vec<Value> = [
                "gpt-6-astra",
                "gpt-6-luna",
                "gpt-6-sol",
                "gpt-6.1-sol",
                "o3",
            ]
            .iter()
            .map(|id| json!({ "id": id, "object": "model", "owned_by": "system" }))
            .collect();
            return ResponseTemplate::new(200)
                .set_body_json(json!({ "object": "list", "data": data }));
        }
        if !url.ends_with("/v1/responses") {
            return error(404, "not_found", "Not found.");
        }
        let Ok(body) = serde_json::from_slice::<Value>(&request.body) else {
            return error(
                400,
                "invalid_json",
                "We could not parse the JSON body of your request.",
            );
        };
        if let Err(message) = validate(&body, &state.emitted) {
            state.refusals.push(message.clone());
            return error(400, "invalid_request", &message);
        }
        state.requests.push(body.clone());
        state
            .raw
            .push(String::from_utf8(request.body.clone()).unwrap());
        drop(state);
        let n = self.count.fetch_add(1, Ordering::SeqCst);
        let out = (self.script)(n, &body);
        if out.status == 200
            && let Some(output) = out.body.get("output").and_then(Value::as_array)
        {
            // Com `allowed_tools` obrigatório, a API só chama os nomes liberados.
            if let Some(allowed) = body.pointer("/tool_choice/tools").and_then(Value::as_array) {
                for item in output.iter().filter(|i| i["type"] == "function_call") {
                    assert!(
                        allowed.iter().any(|a| a["name"] == item["name"]),
                        "roteiro chamou {} fora do allowed_tools",
                        item["name"]
                    );
                }
            }
            self.state.lock().unwrap().emitted.push(output.clone());
        }
        let mut template = ResponseTemplate::new(out.status).set_body_json(out.body);
        if let Some(seconds) = out.retry_after {
            template = template.insert_header("retry-after", seconds.to_string().as_str());
        }
        template
    }
}

/// As regras da Responses API sem estado com modelos de raciocínio (10/2026).
fn validate(request: &Value, emitted: &[Vec<Value>]) -> Result<(), String> {
    if request.get("store") != Some(&json!(false)) {
        return Err("store precisa ser false (o Warden não guarda nada na OpenAI)".into());
    }
    let include = request.get("include").and_then(Value::as_array);
    if !include.is_some_and(|list| list.contains(&json!("reasoning.encrypted_content"))) {
        return Err("include sem reasoning.encrypted_content".into());
    }
    let input = request
        .get("input")
        .and_then(Value::as_array)
        .ok_or("input ausente")?;
    let mut i = 0;
    while i < input.len() {
        let item = &input[i];
        let kind = item.get("type").and_then(Value::as_str).unwrap_or("");
        if kind == "message" && item.get("role") == Some(&json!("user")) {
            i += 1;
            continue;
        }
        if kind == "function_call_output" {
            return Err(format!(
                "No tool call found for function call output with call_id {}.",
                item["call_id"]
            ));
        }
        // Um item do modelo: a resposta inteira, igual, na ordem.
        let Some(output) = emitted
            .iter()
            .find(|output| input.get(i..i + output.len()) == Some(output.as_slice()))
        else {
            return Err(match kind {
                "function_call" => format!(
                    "Item '{}' of type 'function_call' was provided without its required 'reasoning' item, or was modified.",
                    item["id"]
                ),
                "reasoning" => "The encrypted content for item could not be verified.".into(),
                _ => format!("Item {i} of type '{kind}' was modified."),
            });
        };
        i += output.len();
        let calls: Vec<&Value> = output
            .iter()
            .filter(|o| o["type"] == "function_call")
            .collect();
        for call in calls {
            let next = input.get(i).ok_or_else(|| {
                format!(
                    "No tool output found for function call {}.",
                    call["call_id"]
                )
            })?;
            if next["type"] != "function_call_output" || next["call_id"] != call["call_id"] {
                return Err(format!(
                    "No tool output found for function call {}.",
                    call["call_id"]
                ));
            }
            i += 1;
        }
    }
    Ok(())
}

// ---------- Respostas do roteiro ----------

static COUNTER: AtomicUsize = AtomicUsize::new(1);

fn next() -> usize {
    COUNTER.fetch_add(1, Ordering::Relaxed)
}

fn ok(output: Vec<Value>) -> Out {
    Out {
        status: 200,
        body: json!({
            "id": format!("resp_{}", next()), "object": "response", "status": "completed",
            "model": "gpt-6-luna", "output": Value::Array(output),
            "usage": { "input_tokens": 3000, "input_tokens_details": { "cached_tokens": 2048 },
                "output_tokens": 140, "output_tokens_details": { "reasoning_tokens": 100 }, "total_tokens": 3140 }
        }),
        retry_after: None,
    }
}

fn reasoning() -> Value {
    let n = next();
    json!({ "type": "reasoning", "id": format!("rs_{n}"), "summary": [],
        "encrypted_content": format!("Z0FBQUFBQm9fcmFjaW9jaW5pby1zaW11bGFkby0{n}") })
}

/// Chamadas paralelas, depois do item de raciocínio.
fn calls(list: &[(&str, Value)]) -> Out {
    let mut output = vec![reasoning()];
    for (name, args) in list {
        let n = next();
        output.push(json!({ "type": "function_call", "id": format!("fc_{n}"),
            "call_id": format!("call_{n}"), "name": name, "arguments": args.to_string(), "status": "completed" }));
    }
    ok(output)
}

fn respond(answer: &Value) -> Out {
    calls(&[("responder", answer.clone())])
}

fn text(text: &str) -> Out {
    ok(vec![
        reasoning(),
        json!({ "type": "message", "id": format!("msg_{}", next()), "role": "assistant", "status": "completed",
            "content": [{ "type": "output_text", "text": text, "annotations": [] }] }),
    ])
}

fn api_error(status: u16, code: &str, kind: &str, retry_after: Option<u64>) -> Out {
    Out {
        status,
        body: json!({ "error": { "message": format!("erro simulado {code}"), "type": kind, "param": null, "code": code } }),
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
    openai: Simulated,
    timer: ManualTimer,
    client: OpenAiClient,
    registry: ToolRegistry,
    source: ValeSombrio,
    watch: Watch,
}

impl Setup {
    async fn new(script: Script) -> Self {
        let openai = Simulated::start(script).await;
        let timer = ManualTimer::new();
        let client = openai.client(&timer);
        Self {
            openai,
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
    conv.provider = AiProvider::Openai;
    conv.model = "gpt-6-luna".into();
    conv
}

fn allowed(request: &Value) -> Vec<String> {
    request
        .pointer("/tool_choice/tools")
        .and_then(Value::as_array)
        .map(|tools| {
            tools
                .iter()
                .map(|t| t["name"].as_str().unwrap().to_owned())
                .collect()
        })
        .unwrap_or_default()
}

// ---------- Testes ----------

#[tokio::test]
async fn paralelas_malformada_e_resposta_pela_responder() {
    let setup = Setup::new(Box::new(|n, _| match n {
        0 => calls(&[
            ("get_findings", json!({})),
            ("get_crash_report", json!({ "session_id": "s3" })),
        ]),
        // Argumentos cortados: malformada.
        1 => ok(vec![
            reasoning(),
            json!({ "type": "function_call", "id": "fc_x", "call_id": "call_x",
            "name": "get_mod_details", "arguments": "{\"mod_id\":", "status": "completed" }),
        ]),
        2 => calls(&[("get_mod_details", json!({ "mod_id": "railways" }))]),
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
        setup.openai.refusals().is_empty(),
        "{:?}",
        setup.openai.refusals()
    );

    let turn = &conv.turns[0];
    let answer = turn.answer.as_ref().unwrap();
    assert_eq!(answer.via, AnswerVia::Respond);
    assert_eq!(answer.checked.claims[0].status, ClaimStatus::Verified);
    assert_eq!(turn.steps.len(), 3);
    assert_eq!(conv.requests, 4);
    // Uso com cache e raciocínio somado, a malformada inclusive: 4 respostas de 3.000 + 40 + 100.
    assert_eq!(conv.usage.prompt, 4 * 3000);
    assert_eq!(conv.usage.cached, 4 * 2048);
    assert_eq!(conv.usage.output, 4 * 40);
    assert_eq!(conv.usage.thoughts, 4 * 100);

    let requests = setup.openai.requests();
    // Escalonamento por etapa pelo allowed_tools; tools fixo; sistema em instructions.
    assert_eq!(allowed(&requests[0]).len(), 12);
    assert!(!allowed(&requests[0]).contains(&"read_config".to_owned()));
    assert!(allowed(&requests[0]).contains(&"responder".to_owned()));
    assert_eq!(allowed(&requests[2]).len(), 16);
    for request in &requests {
        assert_eq!(request["tool_choice"]["type"], "allowed_tools");
        assert_eq!(request["tool_choice"]["mode"], "required");
        assert_eq!(request["tools"], requests[0]["tools"]);
        assert_eq!(request["instructions"], requests[0]["instructions"]);
        assert!(request.get("text").is_none());
    }
    // A nova tentativa da malformada não mexeu no histórico.
    assert_eq!(requests[1]["input"], requests[2]["input"]);
    // O raciocínio e as duas chamadas voltaram iguais, seguidos dos dois resultados com o mesmo
    // call_id.
    let input = requests[1]["input"].as_array().unwrap();
    assert_eq!(input[1]["type"], "reasoning");
    assert_eq!(input[2]["call_id"], input[4]["call_id"]);
    assert_eq!(input[3]["call_id"], input[5]["call_id"]);
    assert_eq!(input[4]["type"], "function_call_output");
    // A chave só no cabeçalho.
    assert!(
        setup
            .openai
            .state
            .lock()
            .unwrap()
            .urls
            .iter()
            .all(|u| !u.contains(KEY))
    );
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
    let raw = setup.openai.raw();
    let first: Value = serde_json::from_str(&raw[0]).unwrap();
    assert_eq!(first["input"][0]["content"], question);
    for step in &conv.turns[0].steps {
        assert!(raw[1].contains(&step.sent), "{}", step.sent);
        assert!(step.sent.starts_with("{\"call_id\""), "{}", step.sent);
    }
    // Cada rodada guardada na conversa é o corpo exato enviado.
    assert_eq!(conv.rounds.len(), 2);
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
        if request["tool_choice"] == "none" {
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
        setup.openai.refusals().is_empty(),
        "{:?}",
        setup.openai.refusals()
    );
    let requests = setup.openai.requests();
    assert_eq!(requests.len(), 9, "8 rodadas e a reserva");
    let last = requests.last().unwrap();
    assert_eq!(last["tool_choice"], "none");
    assert_eq!(last["tools"], requests[0]["tools"]);
    assert_eq!(last["text"]["format"]["type"], "json_schema");
    assert_eq!(last["text"]["format"]["schema"]["required"][0], "resumo");
    let input = last["input"].as_array().unwrap();
    assert_eq!(input.last().unwrap()["content"], ROUND_LIMIT_MESSAGE);
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
        setup.openai.refusals().is_empty(),
        "{:?}",
        setup.openai.refusals()
    );
    let requests = setup.openai.requests();
    let emitted = setup.openai.state.lock().unwrap().emitted.clone();
    // Os itens da 1.ª resposta (raciocínio criptografado e chamada) voltam iguais na 4.ª.
    let input = requests[3]["input"].as_array().unwrap();
    assert_eq!(&input[1..3], emitted[0].as_slice());
    assert!(input[1]["encrypted_content"].is_string());
}

#[tokio::test]
async fn simulado_recusa_raciocinio_ausente_alterado_e_resultado_faltando() {
    let user = json!({ "type": "message", "role": "user", "content": "oi" });
    let output = vec![
        json!({ "type": "reasoning", "id": "rs_1", "summary": [], "encrypted_content": "QUJDREVGR0g=" }),
        json!({ "type": "function_call", "id": "fc_1", "call_id": "call_1", "name": "list_mods", "arguments": "{}" }),
    ];
    let result = json!({ "type": "function_call_output", "call_id": "call_1", "output": "{}" });
    let request = |input: Vec<Value>| json!({ "store": false, "include": ["reasoning.encrypted_content"], "input": input });
    let emitted = vec![output.clone()];
    assert!(
        validate(
            &request(vec![
                user.clone(),
                output[0].clone(),
                output[1].clone(),
                result.clone()
            ]),
            &emitted
        )
        .is_ok()
    );
    // Sem o raciocínio.
    let missing = validate(
        &request(vec![user.clone(), output[1].clone(), result.clone()]),
        &emitted,
    );
    assert!(
        missing
            .unwrap_err()
            .contains("without its required 'reasoning'")
    );
    // Raciocínio alterado.
    let mut altered = output[0].clone();
    altered["encrypted_content"] = json!("QUJDREVGR0k=");
    assert!(
        validate(
            &request(vec![
                user.clone(),
                altered,
                output[1].clone(),
                result.clone()
            ]),
            &emitted
        )
        .is_err()
    );
    // Sem o resultado.
    assert!(
        validate(
            &request(vec![user.clone(), output[0].clone(), output[1].clone()]),
            &emitted
        )
        .is_err()
    );
    // Guardar na OpenAI: recusado pelo simulado.
    let mut stored = request(vec![user]);
    stored["store"] = json!(true);
    assert!(validate(&stored, &emitted).is_err());
}

#[tokio::test]
async fn texto_leva_a_reserva_sem_ferramentas() {
    let setup = Setup::new(Box::new(|n, request| match n {
        0 => calls(&[("get_crash_report", json!({ "session_id": "s3" }))]),
        1 => text("O travamento é do Steam 'n' Rails. Atualize."),
        _ => {
            assert_eq!(request["tool_choice"], "none");
            text(&format!("```json\n{}\n```", simple_answer()))
        }
    }))
    .await;
    let mut conv = conversation();
    setup.engine().ask(&mut conv, "?".into()).await.unwrap();
    let requests = setup.openai.requests();
    assert_eq!(requests.len(), 3);
    assert_eq!(
        requests[1]["input"], requests[2]["input"],
        "o texto não entrou no histórico"
    );
    let answer = conv.turns[0].answer.as_ref().unwrap();
    assert_eq!(answer.via, AnswerVia::Fallback);
    assert!(!answer.round_limit);
}

#[tokio::test]
async fn sobrecarga_ate_4_vezes_e_depois_erro() {
    let setup = Setup::new(Box::new(|n, _| match n {
        0 => api_error(
            503,
            "server_is_overloaded",
            "service_unavailable_error",
            None,
        ),
        1..=3 => api_error(500, "server_error", "server_error", None),
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
        api_error(
            503,
            "server_is_overloaded",
            "service_unavailable_error",
            None,
        )
    }))
    .await;
    let mut conv = conversation();
    let error = setup.engine().ask(&mut conv, "?".into()).await.unwrap_err();
    assert_eq!(error.code(), DomainCode::Domain(AiErrorCode::Unavailable));
    assert_eq!(setup.openai.requests().len(), 5);
    assert!(conv.turns[0].failure.as_ref().unwrap().retryable);
}

#[tokio::test]
async fn limite_por_minuto_espera_o_tempo_pedido() {
    let setup = Setup::new(Box::new(|n, _| match n {
        0 => api_error(429, "rate_limit_exceeded", "requests", Some(12)),
        1 => calls(&[("get_findings", json!({}))]),
        _ => respond(&simple_answer()),
    }))
    .await;
    let mut conv = conversation();
    setup.engine().ask(&mut conv, "?".into()).await.unwrap();
    assert_eq!(setup.sleeps(), [13]);
    assert!(
        setup
            .watch
            .lives
            .lock()
            .unwrap()
            .contains(&LiveStatus::WaitingPerMinute { seconds: 13 })
    );
}

#[tokio::test]
async fn cota_cobranca_e_chave_param_na_hora() {
    let setup = Setup::new(Box::new(|_, _| {
        api_error(429, "insufficient_quota", "insufficient_quota", None)
    }))
    .await;
    let mut conv = conversation();
    let error = setup.engine().ask(&mut conv, "?".into()).await.unwrap_err();
    assert_eq!(error.code(), DomainCode::Domain(AiErrorCode::QuotaExceeded));
    assert_eq!(setup.openai.requests().len(), 1, "nenhuma nova tentativa");
    assert!(setup.sleeps().is_empty());
    let failure = conv.turns[0].failure.as_ref().unwrap();
    assert_eq!(
        failure.params["freeTier"], "false",
        "a OpenAI não tem plano gratuito"
    );

    let setup = Setup::new(Box::new(|_, _| {
        api_error(429, "credit_balance_exhausted", "insufficient_quota", None)
    }))
    .await;
    let mut conv = conversation();
    let error = setup.engine().ask(&mut conv, "?".into()).await.unwrap_err();
    assert_eq!(error.code(), DomainCode::Domain(AiErrorCode::Billing));

    // Chave errada: o simulado recusa com 401.
    let openai = Simulated::start(Box::new(|_, _| unreachable!())).await;
    let http = HttpClient::new(HttpConfig::for_version("9.9.9")).unwrap();
    let wrong =
        OpenAiClient::with_base_url(http, &openai.server.uri(), SecretString::from("sk-outra"))
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
async fn tempo_esgotado_repetido_e_depois_erro() {
    let slow = MockServer::start().await;
    Mock::given(wiremock::matchers::any())
        .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(5)))
        .mount(&slow)
        .await;
    let timer = ManualTimer::new();
    let http =
        HttpClient::with_timer(HttpConfig::for_version("9.9.9"), Arc::new(timer.clone())).unwrap();
    let client = OpenAiClient::with_base_url(http, &slow.uri(), SecretString::from(KEY))
        .unwrap()
        .with_timeout(Duration::from_millis(100));
    let registry = ToolRegistry::new();
    let source = ValeSombrio::default();
    let watch = Watch::default();
    let engine = Engine {
        provider: &client,
        registry: &registry,
        source: &source,
        observer: &watch,
        timer: Arc::new(timer.clone()),
        cancel: CancellationToken::new(),
        limits: Limits::default(),
    };
    let mut conv = conversation();
    let error = engine.ask(&mut conv, "?".into()).await.unwrap_err();
    assert!(matches!(error, Error::Timeout { .. }), "{error}");
    assert_eq!(slow.received_requests().await.unwrap().len(), 5);
}

#[tokio::test]
async fn modelos_pela_regra_e_testador_da_chave() {
    let setup = Setup::new(Box::new(|_, _| unreachable!())).await;
    let choices = setup.client.models().await.unwrap();
    assert_eq!(choices.economical.as_deref(), Some("gpt-6-luna"));
    assert_eq!(choices.careful.as_deref(), Some("gpt-6-sol"));
    assert_eq!(setup.client.id(), AiProvider::Openai);
    assert_eq!(
        setup
            .client
            .count_tokens(
                &warden_ai::providers::ChatRequest {
                    model: "gpt-6-luna".into(),
                    messages: vec![],
                    tools: vec![],
                    choice: warden_ai::providers::ToolChoice::Auto,
                    response_schema: None,
                },
                &CancellationToken::new()
            )
            .await
            .unwrap(),
        None,
        "a OpenAI não conta antes"
    );
}
