//! Apoio dos testes do laço: o servidor simulado do Gemini e o pack fictício do spike.
//!
//! O servidor simulado (`wiremock` com um `Respond` próprio, S-R5-4 §16 item 9) faz o que a API
//! real faz nos pontos que importam (regras do `mock.rs` do spike):
//! - exige a chave no cabeçalho `x-goog-api-key` e recusa `key=` na URL;
//! - no turno atual (depois da última mensagem do usuário com texto), cada `content` do modelo
//!   tem de ser **idêntico** a um que o servidor mandou (recusa histórico alterado) e a primeira
//!   `functionCall` de cada passo tem de trazer a `thoughtSignature` (400 como a API real);
//! - depois de um `content` com N chamadas vem uma mensagem do usuário com as N respostas, com os
//!   mesmos `id` (intercalar dá 400);
//! - responde por roteiro: escrito à mão ou reproduzindo uma gravação real.

// Nos auxiliares dos testes, `unwrap`, `expect` e `panic!` são a forma idiomática de falhar.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    dead_code,
    unreachable_pub,
    missing_docs
)]

pub mod pack;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use secrecy::SecretString;
use serde_json::{Value, json};
use warden_ai::conversation::{
    Conversation, ConversationOrigin, PackSnapshot, Symptom, new_conversation,
};
use warden_ai::gemini::GeminiClient;
use warden_ai::tools::{
    AiPermissions, KnownMod, KnownMods, Needs, Stage, ToolItem, ToolOutput, ToolSource, ToolSpec,
};
use warden_http::{HttpClient, HttpConfig, ManualTimer};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

pub const FAKE_KEY: &str = "chave-de-teste-do-servidor-simulado";

/// O que o servidor devolve a cada `generateContent`: (número da requisição, corpo) →
/// (código, corpo).
pub type Script = Box<dyn Fn(usize, &Value) -> (u16, Value) + Send + Sync>;

#[derive(Default)]
pub struct State {
    pub emitted: Vec<Value>,
    /// Corpos de `generateContent` aceitos, como chegaram (bytes).
    pub raw: Vec<String>,
    pub requests: Vec<Value>,
    pub refusals: Vec<String>,
    pub count_tokens: Vec<Value>,
    pub urls: Vec<String>,
}

pub struct Gemini {
    pub server: MockServer,
    pub state: Arc<Mutex<State>>,
}

impl Gemini {
    pub async fn start(script: Script) -> Self {
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

    pub fn refusals(&self) -> Vec<String> {
        self.state.lock().unwrap().refusals.clone()
    }

    pub fn requests(&self) -> Vec<Value> {
        self.state.lock().unwrap().requests.clone()
    }

    pub fn raw(&self) -> Vec<String> {
        self.state.lock().unwrap().raw.clone()
    }

    pub fn client(&self, timer: &ManualTimer) -> GeminiClient {
        let http =
            HttpClient::with_timer(HttpConfig::for_version("9.9.9"), Arc::new(timer.clone()))
                .unwrap();
        GeminiClient::with_base_url(http, &self.server.uri(), SecretString::from(FAKE_KEY)).unwrap()
    }
}

struct Responder {
    state: Arc<Mutex<State>>,
    script: Script,
    count: AtomicUsize,
}

fn error(status: u16, message: &str) -> ResponseTemplate {
    let api_status = if status == 400 {
        "INVALID_ARGUMENT"
    } else {
        "PERMISSION_DENIED"
    };
    ResponseTemplate::new(status).set_body_json(
        json!({ "error": { "code": status, "message": message, "status": api_status } }),
    )
}

impl Respond for Responder {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let url = request.url.to_string();
        let mut state = self.state.lock().unwrap();
        state.urls.push(url.clone());
        if url.contains("key=") {
            state.refusals.push("chave na URL".into());
            return error(400, "A chave não pode ir na URL.");
        }
        let key = request
            .headers
            .get("x-goog-api-key")
            .and_then(|value| value.to_str().ok());
        if key != Some(FAKE_KEY) {
            state.refusals.push("chave ausente ou errada".into());
            return error(403, "Method doesn't allow unregistered callers.");
        }
        if request.method.as_str() == "GET" {
            return ResponseTemplate::new(200).set_body_json(json!({ "models": [
                { "name": "models/gemini-3.5-flash-lite", "supportedGenerationMethods": ["generateContent", "countTokens"] },
                { "name": "models/gemini-3.8-flash", "supportedGenerationMethods": ["generateContent", "countTokens"] }
            ]}));
        }
        let Ok(body) = serde_json::from_slice::<Value>(&request.body) else {
            return error(400, "Invalid JSON payload received.");
        };
        if url.contains(":countTokens") {
            state.count_tokens.push(body);
            let size = request.body.len() / 4;
            return ResponseTemplate::new(200).set_body_json(json!({ "totalTokens": size }));
        }
        if let Err(message) = validate(&body, &state.emitted) {
            state.refusals.push(message.clone());
            return error(400, &message);
        }
        state.requests.push(body.clone());
        state
            .raw
            .push(String::from_utf8(request.body.clone()).unwrap());
        drop(state);
        let n = self.count.fetch_add(1, Ordering::SeqCst);
        let (status, reply) = (self.script)(n, &body);
        if status == 200
            && let Some(content) = reply.pointer("/candidates/0/content")
        {
            self.state.lock().unwrap().emitted.push(content.clone());
        }
        ResponseTemplate::new(status).set_body_json(reply)
    }
}

/// As regras de histórico da API real para o Gemini 3 (thought-signatures, 2026-10).
pub fn validate(request: &Value, emitted: &[Value]) -> Result<(), String> {
    let contents = request
        .get("contents")
        .and_then(Value::as_array)
        .ok_or("contents ausente")?;
    let start = contents
        .iter()
        .rposition(|c| {
            c.get("role").and_then(Value::as_str) == Some("user")
                && c.get("parts")
                    .and_then(Value::as_array)
                    .is_some_and(|ps| ps.iter().any(|p| p.get("text").is_some()))
        })
        .ok_or("nenhuma mensagem do usuário")?;
    for (i, c) in contents.iter().enumerate().skip(start + 1) {
        if c.get("role").and_then(Value::as_str) != Some("model") {
            continue;
        }
        let parts = c
            .get("parts")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let calls: Vec<&Value> = parts
            .iter()
            .filter(|p| p.get("functionCall").is_some())
            .collect();
        if let Some(first) = calls.first()
            && first.get("thoughtSignature").is_none()
        {
            let name = first
                .pointer("/functionCall/name")
                .and_then(Value::as_str)
                .unwrap_or("?");
            return Err(format!(
                "Function call {name} in the {i}. content block is missing a thought_signature."
            ));
        }
        if !emitted.iter().any(|e| e == c) {
            return Err(format!(
                "Corrupted thought signature or modified model content in the {i}. content block."
            ));
        }
        if !calls.is_empty() {
            let next = contents.get(i + 1).ok_or(format!(
                "Function calls in the {i}. content block have no responses."
            ))?;
            let responses: Vec<&Value> = next
                .get("parts")
                .and_then(Value::as_array)
                .map(|ps| {
                    ps.iter()
                        .filter_map(|p| p.get("functionResponse"))
                        .collect()
                })
                .unwrap_or_default();
            if next.get("role").and_then(Value::as_str) != Some("user")
                || responses.len() != calls.len()
            {
                return Err(format!(
                    "Please ensure that the number of function response parts is equal to the number of function call parts of the function call turn (content block {i})."
                ));
            }
            for call in &calls {
                if let Some(id) = call.pointer("/functionCall/id")
                    && !responses.iter().any(|r| r.get("id") == Some(id))
                {
                    return Err(format!("Function response for call id {id} not found."));
                }
            }
        }
    }
    Ok(())
}

// ---------- Respostas para os roteiros escritos à mão ----------

static COUNTER: AtomicUsize = AtomicUsize::new(1);

fn signature() -> String {
    format!("c2lnLWZhbHNh{:04}", COUNTER.fetch_add(1, Ordering::Relaxed))
}

fn usage(prompt: u64, output: u64) -> Value {
    json!({ "promptTokenCount": prompt, "candidatesTokenCount": output, "thoughtsTokenCount": 50, "totalTokenCount": prompt + output + 50 })
}

/// Chamadas paralelas: só a primeira parte leva assinatura (como o Gemini 3).
pub fn calls(list: &[(&str, Value)]) -> (u16, Value) {
    let parts: Vec<Value> = list
        .iter()
        .enumerate()
        .map(|(i, (name, args))| {
            let mut part = json!({ "functionCall": { "name": name, "args": args, "id": format!("call_{}", signature()) } });
            if i == 0 {
                part["thoughtSignature"] = json!(signature());
            }
            part
        })
        .collect();
    (
        200,
        json!({
            "candidates": [{ "content": { "role": "model", "parts": parts }, "finishReason": "STOP", "index": 0 }],
            "usageMetadata": usage(3000, 40),
            "modelVersion": "simulado"
        }),
    )
}

/// Resposta final pela `responder`.
pub fn respond(answer: &Value) -> (u16, Value) {
    calls(&[("responder", answer.clone())])
}

/// Resposta em texto (com a assinatura na última parte, como o Gemini 3).
pub fn text(text: &str) -> (u16, Value) {
    (
        200,
        json!({
            "candidates": [{ "content": { "role": "model", "parts": [{ "text": text, "thoughtSignature": signature() }] }, "finishReason": "STOP", "index": 0 }],
            "usageMetadata": usage(5000, 400),
            "modelVersion": "simulado"
        }),
    )
}

/// Um corpo real gravado de `respostas-com-problema-reais.json`.
pub fn real_problem(name: &str) -> (u16, Value) {
    let all: Value = serde_json::from_str(include_str!(
        "../fixtures/respostas-com-problema-reais.json"
    ))
    .unwrap();
    (200, all[name]["resposta"].clone())
}

/// Um corpo real de erro de `erros-http-reais.json`.
pub fn real_error(name: &str) -> (u16, Value) {
    let all: Value =
        serde_json::from_str(include_str!("../fixtures/erros-http-reais.json")).unwrap();
    let body = all[name].clone();
    let status = u16::try_from(body["error"]["code"].as_u64().unwrap()).unwrap();
    (status, body)
}

/// Uma resposta final simples, citando o crash report da sessão s3.
pub fn simple_answer() -> Value {
    json!({
        "resumo": "O Steam 'n' Rails 1.6.4 quebra com o Create 6.", "semConclusao": false, "propostas": [],
        "achados": [{ "afirmacao": "O mixin do Rails chama um método que o Create 6 tirou.", "mods": ["railways"], "confianca": "alta",
            "evidencias": [{ "id": "crash:s3", "citacao": "railways$tickBogeys" }] }]
    })
}

pub fn fixture(name: &str) -> Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

/// Uma conversa nova no pack fictício.
pub fn conversation(permissions: AiPermissions) -> Conversation {
    new_conversation(
        ulid::Ulid::generate().to_string(),
        1,
        "gemini-3.5-flash-lite".into(),
        permissions,
        Symptom::CrashInWorld,
        ConversationOrigin::PackOnly,
        None,
        PackSnapshot::default(),
    )
}

// ---------- O pack fictício como fonte das ferramentas ----------

/// As ferramentas de consulta externa da D-14 nas gravações (changelog e issues), declaradas
/// pelo ponto de extensão do registro.
pub fn external_tools() -> Vec<ToolSpec> {
    let mod_id = json!({ "type": "object", "properties": { "mod_id": { "type": "string" } }, "required": ["mod_id"] });
    vec![
        ToolSpec {
            name: "get_mod_changelog",
            description: "Changelog do mod (teste).",
            parameters: mod_id.clone(),
            stage: Stage::AfterSuspect,
            needs: Needs::Nothing,
            opens_suspects: false,
        },
        ToolSpec {
            name: "search_mod_issues",
            description: "Issues do mod no GitHub (teste).",
            parameters: mod_id,
            stage: Stage::AfterSuspect,
            needs: Needs::Github,
            opens_suspects: false,
        },
    ]
}

/// O pack "Vale Sombrio" com os mesmos ids de evidência do spike.
#[derive(Default)]
pub struct ValeSombrio {
    pub calls: Mutex<Vec<String>>,
}

fn mod_text(m: &pack::Mod) -> String {
    format!(
        "{} ({}) {}, lado: {}, fonte: {}{}",
        m.id,
        m.nome,
        m.versao,
        m.lado,
        m.fonte,
        if m.biblioteca { ", biblioteca" } else { "" }
    )
}

#[async_trait]
impl ToolSource for ValeSombrio {
    #[allow(clippy::too_many_lines)]
    async fn call(&self, name: &str, args: &Value) -> Result<ToolOutput, String> {
        self.calls.lock().unwrap().push(name.to_owned());
        let arg = |k: &str| {
            args.get(k)
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string()
        };
        let int = |k: &str| args.get(k).and_then(Value::as_i64);
        let mut game_data = false;
        let list: Result<Vec<(String, String)>, String> = match name {
            "get_pack_overview" => Ok(vec![("pack:overview".into(), pack::VISAO_GERAL.into())]),
            "list_mods" => {
                let f = format!("{}{}", arg("filtro"), arg("filter")).to_lowercase();
                Ok(pack::MODS
                    .iter()
                    .filter(|m| {
                        f.is_empty() || m.id.contains(&f) || m.nome.to_lowercase().contains(&f)
                    })
                    .map(|m| (format!("mod:{}", m.id), mod_text(m)))
                    .collect())
            }
            "get_mod_details" => match pack::mod_por_id(&arg("mod_id")) {
                None => Err(format!("O mod '{}' não está no pack.", arg("mod_id"))),
                Some(m) => {
                    let deps = if m.dependencias.is_empty() {
                        "nenhuma".to_string()
                    } else {
                        m.dependencias
                            .iter()
                            .map(|(d, f)| format!("{d} {f} (obrigatória)"))
                            .collect::<Vec<_>>()
                            .join("; ")
                    };
                    Ok(vec![(
                        format!("jar:{}", m.id),
                        format!(
                            "{}\nDependências declaradas no jar: {}\nVersão mais nova compatível com Minecraft 1.20.1 + Forge na fonte: {}",
                            mod_text(m),
                            deps,
                            m.mais_nova
                        ),
                    )])
                }
            },
            "get_findings" => Ok(pack::ACHADOS
                .iter()
                .map(|a| (a.id.to_string(), a.texto.to_string()))
                .collect()),
            "list_sessions" => Ok(pack::SESSOES
                .iter()
                .map(|s| (format!("session:{}", s.id), s.resumo.to_string()))
                .collect()),
            "get_session" => match pack::SESSOES.iter().find(|s| s.id == arg("session_id")) {
                Some(s) => Ok(vec![(format!("session:{}", s.id), s.resumo.to_string())]),
                None => Err(format!("Sessão '{}' não existe.", arg("session_id"))),
            },
            "get_crash_report" => {
                game_data = true;
                match arg("session_id").as_str() {
                    "s3" => Ok(vec![("crash:s3".into(), pack::CRASH_S3.into())]),
                    "s2" => Ok(vec![(
                        "crash:s2".into(),
                        pack::CRASH_S3.replace("2026-10-03 21:14:07", "2026-10-02 19:43:11"),
                    )]),
                    outra => Err(format!("A sessão '{outra}' não tem crash report.")),
                }
            }
            "search_log" => {
                game_data = true;
                let q = arg("query").to_lowercase();
                if !matches!(arg("session_id").as_str(), "s2" | "s3") {
                    Err(format!(
                        "Sem log guardado para a sessão '{}'.",
                        arg("session_id")
                    ))
                } else if q.is_empty() {
                    Err("query vazia".into())
                } else {
                    let s = arg("session_id");
                    Ok(pack::LOG_S3
                        .iter()
                        .filter(|(_, l)| l.to_lowercase().contains(&q))
                        .take(40)
                        .map(|(n, l)| (format!("log:{s}/latest.log#L{n}"), l.to_string()))
                        .collect())
                }
            }
            "read_log" => {
                game_data = true;
                let (from, to) = (int("from").unwrap_or(1), int("to").unwrap_or(1));
                let s = arg("session_id");
                if !matches!(s.as_str(), "s2" | "s3") {
                    Err(format!("Sem log guardado para a sessão '{s}'."))
                } else if to < from || to - from > 300 {
                    Err("Intervalo inválido.".into())
                } else {
                    Ok(pack::LOG_S3
                        .iter()
                        .filter(|(n, _)| i64::from(*n) >= from && i64::from(*n) <= to)
                        .map(|(n, l)| (format!("log:{s}/latest.log#L{n}"), l.to_string()))
                        .collect())
                }
            }
            "search_configs" => {
                game_data = true;
                let q = arg("query").to_lowercase();
                Ok(pack::CONFIGS
                    .iter()
                    .flat_map(|(p, lines)| lines.iter().map(move |(n, l)| (p, n, l)))
                    .filter(|(p, _, l)| l.to_lowercase().contains(&q) || p.contains(&q))
                    .take(40)
                    .map(|(p, n, l)| (format!("config:{p}#L{n}"), l.to_string()))
                    .collect())
            }
            "read_config" => {
                game_data = true;
                match pack::CONFIGS.iter().find(|(p, _)| *p == arg("path")) {
                    Some((p, lines)) => Ok(lines
                        .iter()
                        .map(|(n, l)| (format!("config:{p}#L{n}"), l.to_string()))
                        .collect()),
                    None => Err(format!("Config '{}' não existe.", arg("path"))),
                }
            }
            "get_history" => Ok(pack::HISTORICO
                .iter()
                .map(|(v, t)| (format!("version:{v}"), t.to_string()))
                .collect()),
            "diff_versions" => match (arg("from").as_str(), arg("to").as_str()) {
                ("v1.3", "v1.4") => Ok(vec![(
                    "version:v1.3..v1.4".into(),
                    pack::DIFF_V13_V14.into(),
                )]),
                (a, b) => Err(format!("Sem diferença guardada entre '{a}' e '{b}'.")),
            },
            "get_dependents" | "why_in_pack" => {
                let id = arg("mod_id");
                if pack::mod_por_id(&id).is_none() {
                    Err(format!("O mod '{id}' não está no pack."))
                } else {
                    let users: Vec<&str> = pack::MODS
                        .iter()
                        .filter(|m| m.dependencias.iter().any(|(d, _)| *d == id))
                        .map(|m| m.id)
                        .collect();
                    Ok(vec![(
                        format!("graph:{id}"),
                        format!("Dependem de {id}: {}", users.join(", ")),
                    )])
                }
            }
            "get_mod_changelog" => {
                game_data = true;
                match arg("mod_id").as_str() {
                    "railways" => Ok(pack::CHANGELOG_RAILWAYS
                        .iter()
                        .map(|(v, t)| (format!("changelog:railways@{v}"), t.to_string()))
                        .collect()),
                    id => Ok(vec![(
                        format!("changelog:{id}"),
                        "A versão instalada já é a mais nova.".into(),
                    )]),
                }
            }
            "search_mod_issues" => {
                game_data = true;
                match arg("mod_id").as_str() {
                    "railways" => Ok(vec![(
                        "issue:Layers-of-Railways/Railway#712".into(),
                        "[fechada, 2026-09-14] Crash NoSuchMethodError getPositionVec with Create 6.0 — Fixed in 1.6.7, please update.".into(),
                    )]),
                    id => Ok(vec![(format!("issue:{id}"), "Nenhuma issue encontrada com essas palavras.".into())]),
                }
            }
            other => Err(format!("Ferramenta desconhecida: {other}")),
        };
        list.map(|items| ToolOutput {
            items: items
                .into_iter()
                .map(|(id, text)| ToolItem::new(id, text))
                .collect(),
            game_data,
            ..ToolOutput::default()
        })
    }

    async fn known_mods(&self) -> KnownMods {
        let mut mods = KnownMods::default();
        for m in pack::MODS {
            mods.insert(
                &KnownMod {
                    id: m.id.into(),
                    name: m.nome.into(),
                    path: format!("mods/{}.pw.toml", m.id),
                },
                [],
            );
        }
        mods
    }
}
