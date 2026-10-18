//! O laço `generateContent` sem estado (ARCHITECTURE §9.5 "Laço"; ADR-0055; S-R5-4 §16 itens
//! 2 a 6), a partir do `laco.rs` do spike.
//!
//! - O histórico inteiro vai em cada requisição; o `content` de cada resposta do modelo é
//!   guardado e reenviado **intacto** (`serde_json::Value`, nunca reconstruído), o que preserva
//!   as *thought signatures* (CA-T14-14).
//! - Declarações fixas por conversa; entre as rodadas muda só `allowedFunctionNames`
//!   ([`ToolRegistry::allowed`]). `VALIDATED` nas rodadas de ferramenta; temperatura 1.0.
//! - Todas as `functionResponse` de uma rodada numa só mensagem, cada uma com o `id` da chamada.
//! - Resposta final pela ferramenta `responder`. Reserva: quando o modelo responde em texto, ou
//!   ao passar de 8 rodadas ou do teto de tokens da conversa, uma requisição com `mode: NONE` e
//!   `responseFormat` (CA-T14-18).
//! - Novas tentativas (S-R5-4 §13): 503, 500, falta de conexão e tempo esgotado até 4 vezes com
//!   espera exponencial (2, 4, 8, 16 s); `MALFORMED_FUNCTION_CALL`, `UNEXPECTED_TOOL_CALL`,
//!   `MALFORMED_RESPONSE`, resposta sem `content`, texto vazio e resposta final vazia até 2 vezes,
//!   **sem mexer no histórico**; 429 por minuto espera o tempo pedido (até 60 s), com a espera
//!   visível ([`LiveStatus::WaitingPerMinute`]); 429 diário para na hora, sem horário; sinal de
//!   cobrança para tudo.

use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use serde::Serialize;
use serde_json::{Value, json};
use warden_core::{CancellationToken, DomainCode, DomainError};
use warden_http::Timer;

use crate::conversation::{
    AnswerVia, Conversation, ConversationTurn, FinalAnswer, TokenUsage, ToolStep, TurnFailure,
};
use crate::error::{Error, Result};
use crate::evidence::check_answer;
use crate::gemini::{GeminiClient, Problem, Reply, api_message, classify};
use crate::proposals;
use crate::schema::{self, ROUND_LIMIT_MESSAGE, SYSTEM_PROMPT};
use crate::tools::{
    NOT_ALLOWED, RESPOND, ToolRegistry, ToolSource, error_response, permitted, response_of,
};

/// Limites do laço.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Rodadas de ferramenta por pergunta (ARCHITECTURE §9.5: 8).
    pub max_rounds: u32,
    /// Novas tentativas por requisição em 503, 500, rede e tempo esgotado.
    pub transient_retries: u32,
    /// Novas tentativas por rodada em respostas do modelo com problema.
    pub model_retries: u32,
    /// Esperas do limite por minuto por requisição.
    pub rate_waits: u32,
    /// Teto de tokens processados por conversa (ARCHITECTURE §9.5 "Laço"): passando dele, a
    /// pergunta vai direto para a reserva, sem mais consultas, como no estouro das rodadas.
    pub max_tokens: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_rounds: 8,
            transient_retries: 4,
            model_retries: 2,
            rate_waits: 4,
            // Uma conversa típica processa de 10 a 100 mil (S-R5-4 §14); o teto só pega a que
            // fugiu do normal (cerca de US$ 0,15 no Flash-Lite).
            max_tokens: 400_000,
        }
    }
}

/// O que a conversa está fazendo agora (para a tela).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum LiveStatus {
    /// Esperando a IA.
    Thinking,
    /// "Aguardando o limite por minuto do plano gratuito…".
    #[serde(rename_all = "camelCase")]
    WaitingPerMinute {
        /// Segundos de espera.
        #[specta(type = specta_typescript::Number)]
        seconds: u64,
    },
    /// O Google ficou sobrecarregado ou não respondeu; nova tentativa.
    #[serde(rename_all = "camelCase")]
    Retrying {
        /// Qual tentativa (1 a 4).
        attempt: u32,
        /// De quantas.
        of: u32,
    },
}

/// Quem acompanha o laço (a `warden-app`: grava a conversa, avisa a interface).
#[async_trait]
pub trait Observer: Send + Sync {
    /// A conversa mudou (pergunta, rodada, resposta ou erro): gravar e avisar.
    async fn changed(&self, conversation: &Conversation);
    /// O estado de agora mudou (esperas visíveis).
    fn live(&self, live: LiveStatus);
}

/// Observador que não faz nada (testes, contagem de tokens).
#[derive(Debug, Default)]
pub struct NoObserver;

#[async_trait]
impl Observer for NoObserver {
    async fn changed(&self, _conversation: &Conversation) {}
    fn live(&self, _live: LiveStatus) {}
}

/// O laço de uma conversa.
pub struct Engine<'a> {
    /// O cliente do Gemini.
    pub client: &'a GeminiClient,
    /// As ferramentas declaradas.
    pub registry: &'a ToolRegistry,
    /// Quem lê os dados.
    pub source: &'a dyn ToolSource,
    /// Quem acompanha.
    pub observer: &'a dyn Observer,
    /// O relógio das esperas.
    pub timer: Arc<dyn Timer>,
    /// Cancelamento (conversa apagada).
    pub cancel: CancellationToken,
    /// Os limites.
    pub limits: Limits,
}

/// Agora, em milissegundos desde 1970.
#[must_use]
pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| {
            u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
        })
}

/// Acrescenta uma pergunta (novo turno) sem mandar nada: [`Engine::run`] manda depois.
pub fn push_question(conversation: &mut Conversation, question: String) {
    conversation
        .contents
        .push(json!({ "role": "user", "parts": [{ "text": question }] }));
    conversation.turns.push(ConversationTurn {
        question,
        asked_at_ms: now_ms(),
        steps: Vec::new(),
        answer: None,
        failure: None,
    });
    conversation.updated_at_ms = now_ms();
}

/// A última pergunta ainda não tem resposta final (esperando a IA ou parada por um erro).
#[must_use]
pub fn pending(conversation: &Conversation) -> bool {
    conversation
        .turns
        .last()
        .is_some_and(|turn| turn.answer.is_none())
}

/// O texto de uma parte de usuário com texto (`None` se não for).
fn user_text(content: &Value) -> Option<&str> {
    if content.get("role").and_then(Value::as_str) != Some("user") {
        return None;
    }
    content
        .get("parts")
        .and_then(Value::as_array)?
        .iter()
        .find_map(|part| part.get("text").and_then(Value::as_str))
}

fn parts(content: &Value) -> &[Value] {
    content
        .get("parts")
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}

/// O que a requisição pede.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// Rodada de ferramentas (`VALIDATED`).
    Tools {
        /// A IA já abriu um suspeito.
        opened: bool,
    },
    /// Reserva: `NONE` + `responseFormat`.
    Final,
}

/// Resultado de uma requisição bem-sucedida, já validado o código.
struct Ok200 {
    body: Value,
}

impl Engine<'_> {
    /// O corpo de uma requisição `generateContent`.
    #[must_use]
    pub fn request_body(&self, contents: &[Value], fallback: bool, opened: bool) -> Value {
        let mode = if fallback {
            Mode::Final
        } else {
            Mode::Tools { opened }
        };
        self.body(contents, mode)
    }

    fn body(&self, contents: &[Value], mode: Mode) -> Value {
        let mut generation = json!({ "temperature": 1.0 });
        let function_calling = match mode {
            Mode::Tools { opened } => json!({
                "mode": "VALIDATED",
                "allowedFunctionNames": self.registry.allowed(opened),
            }),
            Mode::Final => {
                generation["responseFormat"] = json!({
                    "text": { "mimeType": "APPLICATION_JSON", "schema": schema::answer_schema() }
                });
                json!({ "mode": "NONE" })
            }
        };
        json!({
            "systemInstruction": { "parts": [{ "text": SYSTEM_PROMPT }] },
            "contents": contents,
            "tools": self.registry.declarations(),
            "toolConfig": { "functionCallingConfig": function_calling },
            "generationConfig": generation,
        })
    }

    /// `countTokens` do pedido inteiro como ele sairia agora (sistema, ferramentas e conversa).
    pub async fn count_tokens(&self, model: &str, contents: &[Value]) -> Result<u64> {
        let mut request = self.body(contents, Mode::Tools { opened: false });
        request["model"] = json!(format!("models/{model}"));
        let body = json!({ "generateContentRequest": request });
        let reply = self
            .client
            .post(model, "countTokens", &body, Some(&self.cancel))
            .await?;
        if reply.status != 200 {
            return Err(stop_error(
                model,
                &reply,
                &classify(reply.status, &reply.body),
            ));
        }
        reply
            .json()
            .get("totalTokens")
            .and_then(Value::as_u64)
            .ok_or_else(|| Error::BadResponse {
                reason: "countTokens sem totalTokens".into(),
            })
    }

    /// Faz uma pergunta (novo turno) e roda o laço até a resposta final conferida.
    pub async fn ask(&self, conversation: &mut Conversation, question: String) -> Result<()> {
        push_question(conversation, question);
        self.observer.changed(conversation).await;
        self.run(conversation).await
    }

    /// Roda (ou retoma, depois de um erro) a última pergunta. O erro fica gravado na pergunta
    /// e também volta para quem chamou.
    pub async fn run(&self, conversation: &mut Conversation) -> Result<()> {
        if let Some(turn) = conversation.turns.last_mut() {
            turn.failure = None;
        }
        let result = self.run_turn(conversation).await;
        if let Err(error) = &result {
            if let Some(turn) = conversation.turns.last_mut() {
                turn.failure = Some(failure_of(error));
            }
            conversation.updated_at_ms = now_ms();
            self.observer.changed(conversation).await;
        }
        result
    }

    /// Rodadas de ferramenta já feitas nesta pergunta e se a reserva já foi pedida.
    fn progress(conversation: &Conversation) -> (u32, bool) {
        let question = conversation
            .turns
            .last()
            .map_or("", |turn| turn.question.as_str());
        let start = conversation
            .contents
            .iter()
            .rposition(|content| user_text(content) == Some(question))
            .unwrap_or(0);
        let rounds = conversation.contents[start..]
            .iter()
            .filter(|content| {
                content.get("role").and_then(Value::as_str) == Some("model")
                    && parts(content)
                        .iter()
                        .any(|part| part.get("functionCall").is_some())
            })
            .count();
        let limit_sent = conversation
            .contents
            .last()
            .and_then(user_text)
            .is_some_and(|text| text == ROUND_LIMIT_MESSAGE);
        (u32::try_from(rounds).unwrap_or(u32::MAX), limit_sent)
    }

    fn opened(&self, conversation: &Conversation) -> bool {
        conversation
            .turns
            .iter()
            .flat_map(|turn| &turn.steps)
            .any(|step| self.registry.opens_suspects(&step.name))
    }

    #[allow(clippy::too_many_lines)] // o laço inteiro num lugar só, como no spike
    async fn run_turn(&self, conversation: &mut Conversation) -> Result<()> {
        let (mut rounds, mut fallback) = Self::progress(conversation);
        let mut model_retries = 0;
        let mut round_limit = fallback;
        loop {
            if self.cancel.is_cancelled() {
                return Err(Error::Cancelled);
            }
            let over = rounds >= self.limits.max_rounds
                || conversation.usage.total() >= self.limits.max_tokens;
            if over && !fallback {
                // Estourou as rodadas ou o teto de tokens: o Warden avisa e pede a resposta sem
                // ferramentas.
                conversation
                    .contents
                    .push(json!({ "role": "user", "parts": [{ "text": ROUND_LIMIT_MESSAGE }] }));
                fallback = true;
                round_limit = true;
            }
            let mode = if fallback {
                Mode::Final
            } else {
                Mode::Tools {
                    opened: self.opened(conversation),
                }
            };
            let body = self.body(&conversation.contents, mode);
            let Ok200 { body: reply } = self.send(conversation, &body).await?;

            let candidate = reply
                .pointer("/candidates/0")
                .cloned()
                .unwrap_or(Value::Null);
            let finish = candidate
                .get("finishReason")
                .and_then(Value::as_str)
                .unwrap_or("");
            let content = candidate.get("content").cloned();
            let bad_finish = matches!(
                finish,
                "MALFORMED_FUNCTION_CALL" | "UNEXPECTED_TOOL_CALL" | "MALFORMED_RESPONSE"
            );
            let Some(content) = content.filter(|_| !bad_finish) else {
                // Erro recuperável do modelo: nova tentativa sem tocar no histórico.
                model_retries += 1;
                if model_retries > self.limits.model_retries {
                    return Err(Error::BadResponse {
                        reason: if finish.is_empty() {
                            "resposta sem conteúdo".into()
                        } else {
                            finish.to_owned()
                        },
                    });
                }
                tracing::info!(finish, "resposta do modelo com problema; nova tentativa");
                continue;
            };
            let calls: Vec<Value> = parts(&content)
                .iter()
                .filter_map(|part| part.get("functionCall").cloned())
                .collect();

            if fallback {
                // Reserva: o JSON vem no texto.
                let text = visible_text(&content);
                let parsed = parse_json_text(&text);
                let proposed = proposals::made_in_last_turn(conversation);
                match parsed {
                    Some(answer)
                        if calls.is_empty() && !schema::degenerate_given(&answer, proposed) =>
                    {
                        conversation.contents.push(content);
                        self.finish(conversation, &answer, AnswerVia::Fallback, round_limit)
                            .await;
                        return Ok(());
                    }
                    _ => {
                        model_retries += 1;
                        if model_retries > self.limits.model_retries {
                            return Err(if round_limit {
                                Error::ToolLimit {
                                    rounds: self.limits.max_rounds,
                                }
                            } else {
                                Error::BadResponse {
                                    reason: "a resposta final não veio em JSON válido".into(),
                                }
                            });
                        }
                        continue;
                    }
                }
            }

            if let Some(respond) = calls
                .iter()
                .find(|call| call.get("name").and_then(Value::as_str) == Some(RESPOND))
            {
                let answer = respond.get("args").cloned().unwrap_or_else(|| json!({}));
                if schema::degenerate_given(&answer, proposals::made_in_last_turn(conversation)) {
                    model_retries += 1;
                    if model_retries > self.limits.model_retries {
                        return Err(Error::BadResponse {
                            reason: "resposta final vazia".into(),
                        });
                    }
                    continue;
                }
                // A chamada e uma resposta curta entram no histórico, para a conversa poder
                // continuar com outra pergunta.
                let responses: Vec<Value> = calls
                    .iter()
                    .map(|call| {
                        let name = call.get("name").and_then(Value::as_str).unwrap_or("");
                        let response = if name == RESPOND {
                            json!({ "ok": true })
                        } else {
                            error_response("não executada: a resposta final já foi dada")
                        };
                        function_response(call, response)
                    })
                    .collect();
                conversation.contents.push(content);
                conversation
                    .contents
                    .push(json!({ "role": "user", "parts": responses }));
                self.finish(conversation, &answer, AnswerVia::Respond, round_limit)
                    .await;
                return Ok(());
            }

            if calls.is_empty() {
                if visible_text(&content).trim().is_empty() {
                    // Visto no gemini-3.5-flash: STOP com texto vazio.
                    model_retries += 1;
                    if model_retries > self.limits.model_retries {
                        return Err(Error::BadResponse {
                            reason: "resposta vazia".into(),
                        });
                    }
                    continue;
                }
                // Respondeu em texto: a reserva pede o JSON sem ferramentas (o texto não entra
                // no histórico).
                fallback = true;
                continue;
            }

            // Rodada de ferramentas: o content do modelo volta exatamente como veio.
            conversation.contents.push(content);
            let mut responses = Vec::with_capacity(calls.len());
            for call in &calls {
                let (part, step) = self.execute(conversation, call).await;
                responses.push(part);
                if let Some(turn) = conversation.turns.last_mut() {
                    turn.steps.push(step);
                }
            }
            // Todas as respostas numa só mensagem, depois de todas as chamadas.
            conversation
                .contents
                .push(json!({ "role": "user", "parts": responses }));
            conversation.updated_at_ms = now_ms();
            self.observer.changed(conversation).await;
            rounds += 1;
            model_retries = 0;
        }
    }

    /// Executa uma chamada e devolve a parte `functionResponse` e o bloco "Enviado à IA". Uma
    /// proposta (`propose_*`, D-14) não tem efeito: vira um cartão pendente na conversa.
    async fn execute(&self, conversation: &mut Conversation, call: &Value) -> (Value, ToolStep) {
        let name = call
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned();
        let args = call.get("args").cloned().unwrap_or_else(|| json!({}));
        let mut ok = false;
        let mut denied = false;
        let mut external = None;
        let response = match self.registry.get(&name) {
            None => error_response(&format!("Ferramenta desconhecida: {name}")),
            Some(spec) if !permitted(spec, conversation.permissions) => {
                denied = true;
                error_response(NOT_ALLOWED)
            }
            Some(_) if proposals::is_proposal(&name) => {
                match proposals::record(conversation, &name, &args, self.source).await {
                    Ok(response) => {
                        ok = true;
                        response
                    }
                    Err(message) => error_response(&message),
                }
            }
            Some(_) => match self.source.call(&name, &args).await {
                Ok(output) => {
                    ok = true;
                    conversation.sent.extend(output.items.iter().cloned());
                    external.clone_from(&output.external);
                    response_of(&output)
                }
                Err(message) => error_response(&message),
            },
        };
        let part = function_response(call, response);
        let step = ToolStep {
            call_id: call
                .get("id")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
            name,
            args: args.to_string(),
            sent: part.to_string(),
            ok,
            denied,
            external,
        };
        (part, step)
    }

    async fn finish(
        &self,
        conversation: &mut Conversation,
        answer: &Value,
        via: AnswerVia,
        round_limit: bool,
    ) {
        let mods = self.source.known_mods().await;
        let checked = check_answer(answer, &conversation.sent, &mods);
        if let Some(turn) = conversation.turns.last_mut() {
            turn.answer = Some(FinalAnswer {
                via,
                round_limit,
                checked,
                raw: answer.to_string(),
                at_ms: now_ms(),
            });
        }
        conversation.updated_at_ms = now_ms();
        self.observer.changed(conversation).await;
    }

    /// Manda uma requisição com as novas tentativas de rede e de sobrecarga e as esperas do
    /// limite por minuto. Devolve só respostas 200.
    async fn send(&self, conversation: &mut Conversation, body: &Value) -> Result<Ok200> {
        let model = conversation.model.clone();
        let mut transient = 0;
        let mut waits = 0;
        loop {
            self.observer.live(LiveStatus::Thinking);
            conversation.requests += 1;
            let outcome = self
                .client
                .post(&model, "generateContent", body, Some(&self.cancel))
                .await;
            match outcome {
                Ok(reply) if reply.status == 200 => {
                    let json = reply.json();
                    if json.is_null() {
                        return Err(Error::BadResponse {
                            reason: "resposta 200 sem JSON".into(),
                        });
                    }
                    if let Some(metadata) = json.get("usageMetadata") {
                        conversation.usage.add(TokenUsage::of(metadata));
                    }
                    return Ok(Ok200 { body: json });
                }
                Ok(reply) => match classify(reply.status, &reply.body) {
                    Problem::Transient { status } => {
                        if transient >= self.limits.transient_retries {
                            return Err(Error::Unavailable { status });
                        }
                        transient += 1;
                        self.backoff(transient).await?;
                    }
                    Problem::PerMinute { wait } => {
                        if waits >= self.limits.rate_waits {
                            return Err(Error::RateLimited { waits });
                        }
                        waits += 1;
                        self.observer.live(LiveStatus::WaitingPerMinute {
                            seconds: wait.as_secs() + 1,
                        });
                        self.sleep(wait + Duration::from_secs(1)).await?;
                    }
                    problem => return Err(stop_error(&model, &reply, &problem)),
                },
                Err(Error::Http(error)) if transient_http(&error) => {
                    if transient >= self.limits.transient_retries {
                        return Err(match error {
                            warden_http::Error::Timeout { .. } => Error::Timeout {
                                seconds: self.client.timeout().as_secs(),
                            },
                            other => Error::Http(other),
                        });
                    }
                    transient += 1;
                    self.backoff(transient).await?;
                }
                Err(error) => return Err(error),
            }
        }
    }

    /// Espera exponencial: 2, 4, 8, 16 s.
    async fn backoff(&self, attempt: u32) -> Result<()> {
        self.observer.live(LiveStatus::Retrying {
            attempt,
            of: self.limits.transient_retries,
        });
        let seconds = 1u64 << attempt.min(6);
        self.sleep(Duration::from_secs(seconds)).await
    }

    async fn sleep(&self, duration: Duration) -> Result<()> {
        tokio::select! {
            () = self.cancel.cancelled() => Err(Error::Cancelled),
            () = self.timer.sleep(duration) => Ok(()),
        }
    }
}

fn transient_http(error: &warden_http::Error) -> bool {
    matches!(
        error,
        warden_http::Error::Timeout { .. } | warden_http::Error::Network { .. }
    )
}

/// O erro de uma resposta que não adianta repetir.
fn stop_error(model: &str, reply: &Reply, problem: &Problem) -> Error {
    match problem {
        Problem::Quota { free_tier } => Error::QuotaExceeded {
            model: model.to_owned(),
            free_tier: *free_tier,
        },
        Problem::Billing => Error::Billing {
            detail: api_message(reply),
        },
        Problem::KeyInvalid => Error::KeyInvalid {
            status: reply.status,
        },
        Problem::Transient { status } => Error::Unavailable { status: *status },
        Problem::PerMinute { .. } => Error::RateLimited { waits: 0 },
        Problem::Other => Error::Rejected {
            status: reply.status,
            detail: api_message(reply),
        },
    }
}

#[allow(clippy::needless_pass_by_value)] // a resposta é montada na hora por quem chama
fn function_response(call: &Value, response: Value) -> Value {
    let mut inner = json!({
        "name": call.get("name").cloned().unwrap_or_else(|| json!("")),
        "response": response,
    });
    if let Some(id) = call.get("id") {
        inner["id"] = id.clone();
    }
    json!({ "functionResponse": inner })
}

/// O texto das partes que não são pensamento.
fn visible_text(content: &Value) -> String {
    parts(content)
        .iter()
        .filter(|part| {
            !part
                .get("thought")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        })
        .filter_map(|part| part.get("text").and_then(Value::as_str))
        .collect()
}

/// O JSON da resposta de reserva (aceita a cerca de código `json` de alguns modelos).
fn parse_json_text(text: &str) -> Option<Value> {
    let trimmed = text.trim();
    let inner = trimmed
        .strip_prefix("```json")
        .or_else(|| trimmed.strip_prefix("```"))
        .and_then(|rest| rest.strip_suffix("```"))
        .unwrap_or(trimmed)
        .trim();
    serde_json::from_str::<Value>(inner)
        .ok()
        .filter(Value::is_object)
}

/// O erro como fica gravado na pergunta (o mesmo formato do `AppError`).
#[must_use]
pub fn failure_of(error: &Error) -> TurnFailure {
    let (domain, code) = match error.code() {
        DomainCode::Core(code) => ("core", serde_json::to_value(code)),
        DomainCode::Domain(code) => ("ai", serde_json::to_value(code)),
    };
    TurnFailure {
        domain: domain.to_owned(),
        code: code
            .ok()
            .and_then(|value| value.as_str().map(ToOwned::to_owned))
            .unwrap_or_else(|| "INTERNAL".to_owned()),
        params: error.params(),
        detail: error.detail(),
        retryable: error.retryable(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_da_reserva() {
        assert!(parse_json_text("```json\n{\"a\":1}\n```").is_some());
        assert!(parse_json_text(" {\"a\":1} ").is_some());
        assert!(parse_json_text("Aqui está: {\"a\":1}").is_none());
        assert!(parse_json_text("[1]").is_none());
    }

    #[test]
    fn erro_gravado_como_o_app_error() {
        let failure = failure_of(&Error::QuotaExceeded {
            model: "m".into(),
            free_tier: true,
        });
        assert_eq!(failure.domain, "ai");
        assert_eq!(failure.code, "QUOTA_EXCEEDED");
        assert_eq!(failure.params["model"], "m");
        let cancelled = failure_of(&Error::Cancelled);
        assert_eq!(
            (cancelled.domain.as_str(), cancelled.code.as_str()),
            ("core", "CANCELLED")
        );
    }
}
