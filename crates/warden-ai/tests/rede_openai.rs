//! Teste contra a API real da OpenAI (`cargo xtask test-network`; QUALITY §4.1; ROADMAP D-16).
//! Marcado `#[ignore = "rede"]`, com nome `rede_*`. A chave vem de `OPENAI_API_KEY` (o `xtask`
//! carrega o `.env` sem imprimir valores); sem ela, o teste se declara pulado. Usa o modelo "Mais
//! econômico" da regra (`gpt-N-luna`) e o pack fictício do spike, com poucas rodadas para sair
//! barato.
//!
//! 5xx e 429 por minuto acontecem: o laço já tenta de novo, e se mesmo assim a OpenAI não
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
use warden_ai::providers::openai::OpenAiClient;
use warden_ai::providers::{AiProvider, Provider, SearchMode, http_config};
use warden_ai::tools::{AiPermissions, ToolRegistry};
use warden_core::{CancellationToken, DomainCode, DomainError};
use warden_http::{HttpClient, HttpConfig, SystemTimer};

#[tokio::test]
#[ignore = "rede"]
async fn rede_openai_conversa_curta_com_o_modelo_mais_economico() {
    let Some(key) = std::env::var("OPENAI_API_KEY")
        .ok()
        .filter(|key| !key.trim().is_empty())
    else {
        eprintln!("rede_openai: pulado (OPENAI_API_KEY ausente)");
        return;
    };
    let http = HttpClient::new(http_config(HttpConfig::for_version("0.0.0"))).unwrap();
    let client = OpenAiClient::new(http, SecretString::from(key)).unwrap();
    let choices = client.models().await.unwrap();
    let (model, mode) = choices.pick(SearchMode::Quick).expect("nenhum modelo");
    assert_eq!(mode, SearchMode::Quick);
    assert_eq!(AiProvider::Openai.mode_of(model), Some(SearchMode::Quick));
    eprintln!("rede_openai: modelo {model}");

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
    conversation.provider = AiProvider::Openai;
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
        eprintln!("rede_openai: inconclusivo, a OpenAI não atendeu agora ({error})");
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
        "rede_openai: {} requisições, {} consultas, {} tokens ({} em cache, {} de raciocínio), resposta por {:?}",
        conversation.requests,
        turn.steps.len(),
        conversation.usage.total(),
        conversation.usage.cached,
        conversation.usage.thoughts,
        answer.via
    );
}
