//! Teste contra a API real do Gemini (`cargo xtask test-network`; QUALITY §4.1; S-R5-4 §16 item
//! 11). Marcado `#[ignore = "rede"]`, com nome `rede_*`. A chave vem de `GEMINI_API_KEY` (o
//! `xtask` carrega o `.env` do repositório principal sem imprimir valores); sem ela, o teste se
//! declara pulado. Usa o modelo padrão resolvido pelo `models.list` e o pack fictício do spike.
//!
//! 503 ("high demand") e 429 por minuto são normais no plano gratuito: o laço já tenta de novo, e
//! se mesmo assim o Google não atender, o teste se declara inconclusivo em vez de falhar.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr
)]

mod support;

use std::sync::Arc;

use secrecy::SecretString;
use serde_json::Value;
use support::ValeSombrio;
use warden_ai::conversation::{AnswerVia, FallbackReason};
use warden_ai::engine::{Engine, Limits, NoObserver};
use warden_ai::providers::gemini::GeminiClient;
use warden_ai::providers::gemini::models::fetch_choices;
use warden_ai::providers::{SearchMode, http_config};
use warden_ai::schema::answer_schema;
use warden_ai::tools::{AiPermissions, ToolRegistry};
use warden_ai::{AiErrorCode, Error};
use warden_core::{CancellationToken, DomainCode, DomainError};
use warden_http::{HttpClient, HttpConfig, SystemTimer};

#[tokio::test]
#[ignore = "rede"]
async fn rede_d04_conversa_completa_com_o_modelo_padrao() {
    let Some(key) = std::env::var("GEMINI_API_KEY")
        .ok()
        .filter(|key| !key.trim().is_empty())
    else {
        eprintln!("rede_d04: pulado (GEMINI_API_KEY ausente)");
        return;
    };
    let http = HttpClient::new(http_config(HttpConfig::for_version("0.0.0"))).unwrap();
    let client = GeminiClient::new(http, SecretString::from(key)).unwrap();
    let choices = fetch_choices(&client).await.unwrap();
    let (model, mode) = choices.pick(SearchMode::Quick).expect("nenhum modelo");
    eprintln!("rede_d04: modelo {model} ({mode:?})");

    let registry = ToolRegistry::new();
    let source = ValeSombrio::default();
    let engine = Engine {
        provider: &client,
        registry: &registry,
        source: &source,
        observer: &NoObserver,
        timer: Arc::new(SystemTimer::new()),
        cancel: CancellationToken::new(),
        limits: Limits::default(),
    };
    let mut conversation = support::conversation(AiPermissions::default());
    conversation.model = model.to_owned();
    conversation.fallback_model = choices.lighter.clone();
    let result = engine
        .ask(
            &mut conversation,
            "O pack travou no último teste (sessão s3). Por que travou e como resolvo?".into(),
        )
        .await;
    if let Err(error) = &result
        && matches!(
            error.code(),
            DomainCode::Domain(AiErrorCode::Unavailable | AiErrorCode::RateLimited)
        )
    {
        eprintln!("rede_d04: inconclusivo, o Google não atendeu agora ({error})");
        return;
    }
    if let Err(Error::QuotaExceeded { .. }) = &result {
        eprintln!("rede_d04: inconclusivo, cota diária do modelo esgotada");
        return;
    }
    result.unwrap();
    let turn = &conversation.turns[0];
    // O modelo padrão tem de responder no tempo-limite (D-15b): a troca para o mais leve por
    // tempo esgotado salva a pessoa no app, mas aqui esconderia um Flash lento ou um corte
    // antes do prazo do provedor. A troca pela cota diária continua aceita.
    assert_ne!(
        turn.fallback_reason,
        Some(FallbackReason::Timeout),
        "o modelo padrão estourou o tempo-limite e a resposta veio do mais leve ({:?})",
        turn.fallback_model
    );
    assert!(!turn.steps.is_empty(), "a IA não chamou nenhuma ferramenta");
    let answer = turn.answer.as_ref().unwrap();
    // As rodadas usam `ANY` (D-15b): o modelo não tem como responder em texto, então a resposta
    // vem pela `responder`. A reserva só vale no caminho que a ADR-0055 prevê para ela: o
    // limite de 8 rodadas ou o teto de tokens da conversa (`round_limit`).
    assert!(
        answer.via == AnswerVia::Respond || answer.round_limit,
        "a resposta não veio pela responder nem pelo limite de rodadas: {:?}",
        answer.via
    );
    // Válida no esquema: os campos obrigatórios com os tipos certos.
    let raw: Value = serde_json::from_str(&answer.raw).unwrap();
    let schema = answer_schema();
    for field in schema["required"].as_array().unwrap() {
        let field = field.as_str().unwrap();
        let kind = schema["properties"][field]["type"].as_str().unwrap();
        let ok = match kind {
            "string" => raw[field].is_string(),
            "array" => raw[field].is_array(),
            "boolean" => raw[field].is_boolean(),
            _ => true,
        };
        assert!(ok, "campo {field} fora do esquema: {raw}");
    }
    eprintln!(
        "rede_d04: {} requisições, {} consultas, {} tokens, confiança {:?}, resposta por {:?}, troca {:?} ({:?})",
        conversation.requests,
        turn.steps.len(),
        conversation.usage.total(),
        answer.checked.confidence,
        answer.via,
        turn.fallback_model,
        turn.fallback_reason
    );
}
