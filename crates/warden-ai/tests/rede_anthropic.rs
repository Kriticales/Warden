//! Teste contra a API real da Anthropic (`cargo xtask test-network`; QUALITY §4.1; ROADMAP D-17).
//! Marcado `#[ignore = "rede"]`, com nome `rede_*`. A chave vem de `ANTHROPIC_API_KEY` (o `xtask`
//! carrega o `.env` sem imprimir valores); sem ela, o teste se declara pulado. Usa o modelo "Mais
//! econômico" da regra (o Sonnet mais novo) e o pack fictício do spike, com poucas rodadas para sair
//! barato.
//!
//! 529, 5xx e 429 por minuto acontecem: o laço já tenta de novo, e se mesmo assim a Anthropic não
//! atender, o teste se declara inconclusivo em vez de falhar.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr
)]

mod support;

use std::sync::Arc;

use secrecy::SecretString;
use support::ValeSombrio;
use warden_ai::AiErrorCode;
use warden_ai::conversation::AnswerVia;
use warden_ai::engine::{Engine, Limits, NoObserver};
use warden_ai::providers::anthropic::AnthropicClient;
use warden_ai::providers::{AiProvider, ModelTier, Provider};
use warden_ai::tools::{AiPermissions, ToolRegistry};
use warden_core::{CancellationToken, DomainCode, DomainError};
use warden_http::{HttpClient, HttpConfig, SystemTimer};

#[tokio::test]
#[ignore = "rede"]
async fn rede_anthropic_conversa_curta_com_o_modelo_mais_economico() {
    let Some(key) = std::env::var("ANTHROPIC_API_KEY")
        .ok()
        .filter(|key| !key.trim().is_empty())
    else {
        eprintln!("rede_anthropic: pulado (ANTHROPIC_API_KEY ausente)");
        return;
    };
    let http = HttpClient::new(HttpConfig::for_version("0.0.0")).unwrap();
    let client = AnthropicClient::new(http, SecretString::from(key)).unwrap();
    let choices = client.models().await.unwrap();
    let (model, tier) = choices.pick(ModelTier::Economical).expect("nenhum modelo");
    assert_eq!(tier, ModelTier::Economical);
    assert_eq!(
        AiProvider::Anthropic.tier_of(model),
        Some(ModelTier::Economical)
    );
    eprintln!("rede_anthropic: modelo {model}");

    let registry = ToolRegistry::new();
    let source = ValeSombrio::default();
    let engine = Engine {
        provider: &client,
        registry: &registry,
        source: &source,
        observer: &NoObserver,
        timer: Arc::new(SystemTimer::new()),
        cancel: CancellationToken::new(),
        // Poucas rodadas: barato, e o estouro ainda cai na reserva.
        limits: Limits {
            max_rounds: 3,
            ..Limits::default()
        },
    };
    let mut conversation = support::conversation(AiPermissions::default());
    conversation.provider = AiProvider::Anthropic;
    conversation.model = model.to_owned();
    let result = engine
        .ask(
            &mut conversation,
            "O pack travou no último teste (sessão s3). Leia o crash report dessa sessão e responda pela ferramenta responder.".into(),
        )
        .await;
    if let Err(error) = &result
        && matches!(
            error.code(),
            DomainCode::Domain(AiErrorCode::Unavailable | AiErrorCode::RateLimited)
        )
    {
        eprintln!("rede_anthropic: inconclusivo, a Anthropic não atendeu agora ({error})");
        return;
    }
    result.unwrap();
    let turn = &conversation.turns[0];
    assert!(!turn.steps.is_empty(), "a IA não chamou nenhuma ferramenta");
    let answer = turn.answer.as_ref().unwrap();
    assert!(matches!(
        answer.via,
        AnswerVia::Respond | AnswerVia::Fallback
    ));
    let _: serde_json::Value = serde_json::from_str(&answer.raw).unwrap();
    eprintln!(
        "rede_anthropic: {} requisições, {} consultas, {} tokens ({} em cache), resposta por {:?}",
        conversation.requests,
        turn.steps.len(),
        conversation.usage.total(),
        conversation.usage.cached,
        answer.via
    );
}
