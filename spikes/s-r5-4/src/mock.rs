//! Servidor simulado do `generateContent` que se comporta como a API real nos pontos que
//! importam para o laço:
//!
//! - exige a chave no cabeçalho `x-goog-api-key` e recusa `?key=` na URL;
//! - no turno atual (depois da última mensagem do usuário com texto), cada `content` do modelo
//!   tem de ser **idêntico** ao que o servidor mandou (recusa histórico alterado) e a primeira
//!   parte `functionCall` de cada passo tem de trazer a `thoughtSignature` (400 com a mesma
//!   mensagem da API real);
//! - depois de um `content` com N chamadas, a mensagem seguinte tem de ser do usuário com as N
//!   respostas, com os mesmos `id` (intercalar dá 400, como na API real);
//! - responde segundo um roteiro (`Roteiro`), que pode ser escrito à mão ou uma gravação real.

use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use serde_json::{Value, json};

pub const CHAVE_FALSA: &str = "chave-de-teste-do-servidor-simulado";

/// O que o servidor devolve a cada `generateContent`, dado o número da requisição e o corpo.
pub type Roteiro = Box<dyn FnMut(usize, &Value) -> Value + Send>;

#[derive(Default)]
struct Estado {
    emitidos: Vec<Value>,
    requisicoes: Vec<Value>,
    recusas: Vec<String>,
}

pub struct Servidor {
    pub url: String,
    estado: Arc<Mutex<Estado>>,
    servidor: Arc<tiny_http::Server>,
    thread: Option<JoinHandle<()>>,
}

impl Servidor {
    pub fn iniciar(mut roteiro: Roteiro) -> Servidor {
        let servidor = Arc::new(tiny_http::Server::http("127.0.0.1:0").expect("porta local"));
        let url = format!("http://{}", servidor.server_addr().to_ip().unwrap());
        let estado = Arc::new(Mutex::new(Estado::default()));
        let (s2, e2) = (servidor.clone(), estado.clone());
        let thread = std::thread::spawn(move || {
            let mut n = 0usize;
            for mut req in s2.incoming_requests() {
                let mut corpo = String::new();
                let _ = req.as_reader().read_to_string(&mut corpo);
                let url = req.url().to_string();
                let chave = req.headers().iter().find(|h| h.field.equiv("x-goog-api-key")).map(|h| h.value.to_string());
                let (status, resposta) = atender(&url, chave.as_deref(), &corpo, &mut n, &mut roteiro, &e2);
                let r = tiny_http::Response::from_string(resposta.to_string())
                    .with_status_code(status)
                    .with_header("Content-Type: application/json".parse::<tiny_http::Header>().unwrap());
                let _ = req.respond(r);
            }
        });
        Servidor { url, estado, servidor, thread: Some(thread) }
    }

    pub fn recusas(&self) -> Vec<String> {
        self.estado.lock().unwrap().recusas.clone()
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn requisicoes(&self) -> Vec<Value> {
        self.estado.lock().unwrap().requisicoes.clone()
    }
}

impl Drop for Servidor {
    fn drop(&mut self) {
        self.servidor.unblock();
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

fn erro(status: u16, msg: &str) -> (u16, Value) {
    let st = if status == 400 { "INVALID_ARGUMENT" } else { "PERMISSION_DENIED" };
    (status, json!({ "error": { "code": status, "message": msg, "status": st } }))
}

fn atender(url: &str, chave: Option<&str>, corpo: &str, n: &mut usize, roteiro: &mut Roteiro, estado: &Mutex<Estado>) -> (u16, Value) {
    let mut st = estado.lock().unwrap();
    if url.contains("key=") {
        st.recusas.push("chave na URL".into());
        return erro(400, "A chave não pode ir na URL neste servidor simulado.");
    }
    if chave != Some(CHAVE_FALSA) {
        st.recusas.push("chave ausente ou errada".into());
        return erro(403, "Method doesn't allow unregistered callers.");
    }
    let Ok(req) = serde_json::from_str::<Value>(corpo) else {
        return erro(400, "Invalid JSON payload received.");
    };
    if url.contains(":countTokens") {
        let tamanho = corpo.len() as u64 / 4;
        return (200, json!({ "totalTokens": tamanho }));
    }
    if let Err(msg) = validar(&req, &st.emitidos) {
        st.recusas.push(msg.clone());
        return erro(400, &msg);
    }
    st.requisicoes.push(req.clone());
    let i = *n;
    *n += 1;
    drop(st);
    let resposta = roteiro(i, &req);
    let mut st = estado.lock().unwrap();
    if let Some(c) = resposta.pointer("/candidates/0/content") {
        st.emitidos.push(c.clone());
    }
    (200, resposta)
}

/// As regras de histórico da API real para o Gemini 3 (thought-signatures, 2026-10).
pub fn validar(req: &Value, emitidos: &[Value]) -> Result<(), String> {
    let contents = req.get("contents").and_then(Value::as_array).ok_or("contents ausente")?;
    // Início do turno atual: última mensagem do usuário com uma parte de texto.
    let inicio = contents
        .iter()
        .rposition(|c| {
            c.get("role").and_then(Value::as_str) == Some("user")
                && c.get("parts").and_then(Value::as_array).is_some_and(|ps| ps.iter().any(|p| p.get("text").is_some()))
        })
        .ok_or("nenhuma mensagem do usuário")?;
    for (i, c) in contents.iter().enumerate().skip(inicio + 1) {
        if c.get("role").and_then(Value::as_str) != Some("model") {
            continue;
        }
        let partes = c.get("parts").and_then(Value::as_array).cloned().unwrap_or_default();
        let chamadas: Vec<&Value> = partes.iter().filter(|p| p.get("functionCall").is_some()).collect();
        if let Some(primeira) = chamadas.first() {
            if primeira.get("thoughtSignature").is_none() {
                let nome = primeira.pointer("/functionCall/name").and_then(Value::as_str).unwrap_or("?");
                return Err(format!("Function call {nome} in the {i}. content block is missing a thought_signature."));
            }
        }
        if !emitidos.iter().any(|e| e == c) {
            return Err(format!("Corrupted thought signature or modified model content in the {i}. content block."));
        }
        if !chamadas.is_empty() {
            let prox = contents.get(i + 1).ok_or(format!("Function calls in the {i}. content block have no responses."))?;
            let respostas: Vec<&Value> = prox
                .get("parts")
                .and_then(Value::as_array)
                .map(|ps| ps.iter().filter_map(|p| p.get("functionResponse")).collect())
                .unwrap_or_default();
            if prox.get("role").and_then(Value::as_str) != Some("user") || respostas.len() != chamadas.len() {
                return Err(format!(
                    "Please ensure that the number of function response parts is equal to the number of function call parts of the function call turn (content block {i})."
                ));
            }
            for fc in &chamadas {
                let id = fc.pointer("/functionCall/id");
                if id.is_some() && !respostas.iter().any(|r| r.get("id") == id) {
                    return Err(format!("Function response for call id {} not found.", id.unwrap()));
                }
            }
        }
    }
    Ok(())
}

// ---------- Construtores de respostas para os roteiros escritos à mão ----------

static CONTADOR: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(1);

fn assinatura() -> String {
    format!("c2lnLWZhbHNh{:04}", CONTADOR.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
}

fn uso(prompt: u64, saida: u64) -> Value {
    json!({ "promptTokenCount": prompt, "candidatesTokenCount": saida, "thoughtsTokenCount": 50, "totalTokenCount": prompt + saida + 50 })
}

/// Resposta com chamadas paralelas: só a primeira parte leva assinatura (como o Gemini 3).
pub fn chamadas(lista: &[(&str, Value)]) -> Value {
    let partes: Vec<Value> = lista
        .iter()
        .enumerate()
        .map(|(i, (nome, args))| {
            let mut p = json!({ "functionCall": { "name": nome, "args": args, "id": format!("fc-{}-{}", nome, assinatura()) } });
            if i == 0 {
                p["thoughtSignature"] = json!(assinatura());
            }
            p
        })
        .collect();
    json!({
        "candidates": [{ "content": { "role": "model", "parts": partes }, "finishReason": "STOP", "index": 0 }],
        "usageMetadata": uso(3000, 40),
        "modelVersion": "simulado"
    })
}

pub fn malformada() -> Value {
    json!({
        "candidates": [{ "finishReason": "MALFORMED_FUNCTION_CALL", "finishMessage": "Malformed function call: get_mod_details(mod_id=railways", "index": 0 }],
        "usageMetadata": uso(3000, 0),
        "modelVersion": "simulado"
    })
}

/// Resposta final em texto JSON, com a assinatura na última parte (como o Gemini 3).
pub fn final_json(v: &Value) -> Value {
    json!({
        "candidates": [{ "content": { "role": "model", "parts": [{ "text": v.to_string(), "thoughtSignature": assinatura() }] }, "finishReason": "STOP", "index": 0 }],
        "usageMetadata": uso(5000, 400),
        "modelVersion": "simulado"
    })
}

/// Roteiro que reproduz uma gravação real (as respostas na ordem em que vieram).
pub fn roteiro_de_gravacao(respostas: Vec<Value>) -> Roteiro {
    Box::new(move |i, _req| respostas.get(i).cloned().unwrap_or_else(|| json!({ "error": "gravação acabou" })))
}
