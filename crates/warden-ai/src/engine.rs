//! O laço sem estado (ARCHITECTURE §9.5 "Laço"; ADR-0055 e ADR-0060; S-R5-4 §16 itens 2 a 6),
//! a partir do `laco.rs` do spike. Fala só com o trait [`Provider`], no formato comum: não
//! conhece a API de nenhum provedor.
//!
//! - O histórico inteiro vai em cada requisição; o campo opaco de cada resposta (no Gemini, o
//!   `content` inteiro, com as *thought signatures*) é guardado e reenviado **intacto**
//!   (CA-T14-14).
//! - Declarações fixas por conversa; entre as rodadas muda só a lista de nomes permitidos
//!   ([`ToolRegistry::allowed`], [`ToolChoice::Required`]).
//! - Todos os resultados de uma rodada numa só mensagem, cada um com o `id` da chamada.
//! - Resposta final pela ferramenta `responder`. Reserva: quando o modelo responde em texto, ou
//!   ao passar de 8 rodadas ou do teto de tokens da conversa, uma requisição sem ferramentas
//!   ([`ToolChoice::None`]) com a resposta em JSON pelo esquema (CA-T14-18).
//! - Novas tentativas (S-R5-4 §13), pela classificação comum ([`Failure`]): temporário
//!   (sobrecarga, falta de conexão, tempo esgotado) até 4 vezes com espera exponencial (2, 4, 8,
//!   16 s); resposta malformada, texto vazio e resposta final vazia até 2 vezes, **sem mexer no
//!   histórico**; limite por minuto espera o tempo pedido (até 60 s), com a espera visível
//!   ([`LiveStatus::WaitingPerMinute`]); cota esgotada para na hora, sem horário; cobrança e
//!   chave recusada param tudo.
//! - Cota diária do modelo da conversa esgotada, com um modelo mais leve conhecido
//!   ([`Conversation::fallback_model`], no Gemini o Flash-Lite): a mesma requisição vai de novo
//!   com o mais leve, só naquela pergunta, e a pergunta guarda a troca
//!   ([`ConversationTurn::fallback_model`]) para a tela avisar (ADR-0060, complemento de
//!   19/10/2026). A próxima pergunta volta ao modelo da conversa.

use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use serde::Serialize;
use serde_json::{Value, json};
use warden_core::{CancellationToken, DomainCode, DomainError};
use warden_http::Timer;

use crate::conversation::{
    AnswerVia, Conversation, ConversationTurn, FinalAnswer, ToolStep, TurnFailure,
};
use crate::error::{Error, Result};
use crate::evidence::check_answer;
use crate::proposals;
use crate::providers::{
    ChatRequest, ChatResponse, Failure, Message, Provider, Role, ToolCall, ToolChoice, ToolResult,
};
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
    /// O provedor ficou sobrecarregado ou não respondeu; nova tentativa.
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
    /// O provedor da conversa.
    pub provider: &'a dyn Provider,
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
    conversation.messages.push(Message::user(question.clone()));
    conversation.turns.push(ConversationTurn {
        question,
        asked_at_ms: now_ms(),
        steps: Vec::new(),
        answer: None,
        failure: None,
        fallback_model: None,
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

/// O que a requisição pede.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// Rodada de ferramentas (só os nomes liberados nesta etapa).
    Tools {
        /// A IA já abriu um suspeito.
        opened: bool,
    },
    /// Reserva: sem ferramentas, resposta em JSON pelo esquema.
    Final,
}

/// O que voltou de uma requisição que o laço não repete sozinho.
enum Sent {
    /// Uma resposta que dá para usar.
    Reply(ChatResponse),
    /// Uma resposta malformada (o laço decide repetir, sem mexer no histórico).
    Malformed(String),
}

impl Engine<'_> {
    /// O pedido com o histórico dado, como ele sai agora.
    fn request(&self, model: &str, messages: &[Message], mode: Mode) -> ChatRequest {
        let (choice, response_schema) = match mode {
            Mode::Tools { opened } => (
                ToolChoice::Required(
                    self.registry
                        .allowed(opened)
                        .into_iter()
                        .map(ToOwned::to_owned)
                        .collect(),
                ),
                None,
            ),
            Mode::Final => (ToolChoice::None, Some(schema::answer_schema())),
        };
        let mut all = Vec::with_capacity(messages.len() + 1);
        all.push(Message::system(SYSTEM_PROMPT));
        all.extend(messages.iter().cloned());
        ChatRequest {
            model: model.to_owned(),
            messages: all,
            tools: self.registry.declarations(),
            choice,
            response_schema,
        }
    }

    /// O tamanho do pedido inteiro como ele sairia agora (sistema, ferramentas e conversa),
    /// contado pelo provedor (`None`: o provedor não conta).
    pub async fn count_tokens(&self, model: &str, messages: &[Message]) -> Result<Option<u64>> {
        let request = self.request(model, messages, Mode::Tools { opened: false });
        self.provider
            .count_tokens(&request, &self.cancel)
            .await
            .map_err(|failure| failure.into_error(model))
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
            .messages
            .iter()
            .rposition(|message| message.user_text() == Some(question))
            .unwrap_or(0);
        let rounds = conversation.messages[start..]
            .iter()
            .filter(|message| message.role == Role::Assistant && !message.calls.is_empty())
            .count();
        let limit_sent = conversation
            .messages
            .last()
            .and_then(Message::user_text)
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
        // A pergunta que já trocou para o modelo mais leve (retomada depois de um erro) continua
        // nele.
        let mut model = conversation
            .turns
            .last()
            .and_then(|turn| turn.fallback_model.clone())
            .unwrap_or_else(|| conversation.model.clone());
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
                    .messages
                    .push(Message::user(ROUND_LIMIT_MESSAGE));
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
            let request = self.request(&model, &conversation.messages, mode);
            let sent = match self.send(conversation, &request).await {
                Ok(sent) => sent,
                Err(error @ Error::QuotaExceeded { .. }) => {
                    let Some(lighter) = conversation
                        .fallback_model
                        .clone()
                        .filter(|lighter| *lighter != model)
                    else {
                        return Err(error);
                    };
                    // A cota diária do modelo acabou: esta pergunta segue no mais leve.
                    tracing::info!(%model, %lighter, "cota diária esgotada; troca para o modelo mais leve");
                    if let Some(turn) = conversation.turns.last_mut() {
                        turn.fallback_model = Some(lighter.clone());
                    }
                    model = lighter;
                    conversation.updated_at_ms = now_ms();
                    self.observer.changed(conversation).await;
                    continue;
                }
                Err(error) => return Err(error),
            };
            let content = match sent {
                Sent::Reply(reply) => reply.message,
                Sent::Malformed(reason) => {
                    // Erro recuperável do modelo: nova tentativa sem tocar no histórico.
                    model_retries += 1;
                    if model_retries > self.limits.model_retries {
                        return Err(Error::BadResponse { reason });
                    }
                    tracing::info!(reason, "resposta do modelo com problema; nova tentativa");
                    continue;
                }
            };
            let calls = content.calls.clone();

            if fallback {
                // Reserva: o JSON vem no texto.
                let parsed = parse_json_text(&content.text);
                let proposed = proposals::made_in_last_turn(conversation);
                match parsed {
                    Some(answer)
                        if calls.is_empty() && !schema::degenerate_given(&answer, proposed) =>
                    {
                        conversation.messages.push(content);
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

            if let Some(respond) = calls.iter().find(|call| call.name == RESPOND) {
                let answer = respond.args.clone();
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
                let responses: Vec<ToolResult> = calls
                    .iter()
                    .map(|call| {
                        let response = if call.name == RESPOND {
                            json!({ "ok": true })
                        } else {
                            error_response("não executada: a resposta final já foi dada")
                        };
                        tool_result(call, response)
                    })
                    .collect();
                conversation.messages.push(content);
                conversation.messages.push(Message::tool_results(responses));
                self.finish(conversation, &answer, AnswerVia::Respond, round_limit)
                    .await;
                return Ok(());
            }

            if calls.is_empty() {
                if content.text.trim().is_empty() {
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

            // Rodada de ferramentas: a resposta do modelo volta exatamente como veio.
            conversation.messages.push(content);
            let mut responses = Vec::with_capacity(calls.len());
            for call in &calls {
                let (part, step) = self.execute(conversation, call).await;
                responses.push(part);
                if let Some(turn) = conversation.turns.last_mut() {
                    turn.steps.push(step);
                }
            }
            // Todas as respostas numa só mensagem, depois de todas as chamadas.
            conversation.messages.push(Message::tool_results(responses));
            conversation.updated_at_ms = now_ms();
            self.observer.changed(conversation).await;
            rounds += 1;
            model_retries = 0;
        }
    }

    /// Executa uma chamada e devolve o resultado e o bloco "Enviado à IA". Uma proposta
    /// (`propose_*`, D-14) não tem efeito: vira um cartão pendente na conversa.
    async fn execute(
        &self,
        conversation: &mut Conversation,
        call: &ToolCall,
    ) -> (ToolResult, ToolStep) {
        let name = call.name.clone();
        let args = call.args.clone();
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
        let result = tool_result(call, response);
        let step = ToolStep {
            call_id: call.id.clone(),
            name,
            args: args.to_string(),
            sent: self.provider.tool_result_text(&result),
            ok,
            denied,
            external,
        };
        (result, step)
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

    /// Manda uma requisição com as novas tentativas dos erros temporários e as esperas do
    /// limite por minuto. Devolve as respostas que o provedor leu (usáveis ou malformadas).
    async fn send(&self, conversation: &mut Conversation, request: &ChatRequest) -> Result<Sent> {
        let model = request.model.clone();
        let mut transient = 0;
        let mut waits = 0;
        loop {
            self.observer.live(LiveStatus::Thinking);
            conversation.requests += 1;
            match self.provider.generate(request, &self.cancel).await {
                Ok(reply) => {
                    conversation.usage.add(reply.usage);
                    conversation.record_round(now_ms(), &reply.sent);
                    return Ok(Sent::Reply(reply));
                }
                Err(Failure::Malformed { reason, usage }) => {
                    conversation.usage.add(usage);
                    return Ok(Sent::Malformed(reason));
                }
                Err(failure) if failure.is_temporary() => {
                    if transient >= self.limits.transient_retries {
                        return Err(match failure {
                            Failure::Network(warden_http::Error::Timeout { .. }) => {
                                Error::Timeout {
                                    seconds: self.provider.timeout().as_secs(),
                                }
                            }
                            other => other.into_error(&model),
                        });
                    }
                    transient += 1;
                    self.backoff(transient).await?;
                }
                Err(Failure::PerMinute { wait }) => {
                    if waits >= self.limits.rate_waits {
                        return Err(Error::RateLimited { waits });
                    }
                    waits += 1;
                    self.observer.live(LiveStatus::WaitingPerMinute {
                        seconds: wait.as_secs() + 1,
                    });
                    self.sleep(wait + Duration::from_secs(1)).await?;
                }
                Err(failure) => return Err(failure.into_error(&model)),
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

/// O resultado de uma chamada, com o `id` dela.
fn tool_result(call: &ToolCall, content: Value) -> ToolResult {
    ToolResult {
        id: call.id.clone(),
        name: call.name.clone(),
        content,
    }
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
