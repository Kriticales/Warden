//! O Gemini migrado para o trait de provedor manda **o mesmo corpo, byte a byte**, que o laço
//! da D-04 mandava para a mesma conversa (ROADMAP D-15; CA-T14-22), salvo o modo das rodadas de
//! ferramenta (`ANY` desde a D-15b, [`as_in_d04`]), e as conversas do formato 1 abrem iguais e
//! continuam com o Gemini.
//!
//! `tests/fixtures/corpos-gemini-d04.json` tem o SHA-256 de cada corpo que o código da D-04
//! (commit 04f2d21) mandou ao servidor simulado, gravado antes de mexer no laço, nos dois jeitos
//! de o `serde_json` ordenar as chaves: o padrão (chaves ordenadas, como neste `cargo test`) e o
//! `preserve_order` (ordem de inserção, como no app, que o liga pelo `schemars`). Para provar os
//! dois: `cargo nextest run -p warden-ai --test corpo_identico` e
//! `cargo nextest run -p warden-ai --features serde_json/preserve_order --test corpo_identico`.

// Nos auxiliares dos testes, `unwrap`, `expect` e `panic!` são a forma idiomática de falhar.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use std::sync::Arc;

use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};
use support::{Gemini, ValeSombrio, conversation, external_tools, fixture};
use warden_ai::conversation::{Conversation, ConversationStore, ConversationTurn, SCHEMA_VERSION};
use warden_ai::engine::{Engine, Limits, NoObserver, push_question};
use warden_ai::providers::{AiProvider, Role};
use warden_ai::tools::{AiPermissions, ToolRegistry};
use warden_core::CancellationToken;
use warden_http::ManualTimer;

const RECORDINGS: [&str; 4] = [
    "travamento-gemini-3.8-flash-ferramenta.json",
    "travamento-gemini-3.8-flash.json",
    "seguimento-gemini-3.1-flash-lite-preview.json",
    "injecao-gemini-3.5-flash-lite.json",
];

const V1: &str = "conversa-v1-seguimento.json";

/// O corpo como a D-04 o mandaria. A única mudança desde então é o modo das rodadas de
/// ferramenta (`VALIDATED` → `ANY`, D-15b); o resto continua igual, byte a byte. Dentro de um
/// texto as aspas vão escapadas, então o trecho só aparece como a chave do modo.
fn as_in_d04(body: &str) -> String {
    body.replace(r#""mode":"ANY""#, r#""mode":"VALIDATED""#)
}

fn sha256(text: &str) -> String {
    use std::fmt::Write as _;
    Sha256::digest(text.as_bytes())
        .iter()
        .fold(String::new(), |mut hex, byte| {
            let _ = write!(hex, "{byte:02x}");
            hex
        })
}

/// Os resumos gravados no jeito de ordenar as chaves deste build.
fn golden() -> Value {
    let order = serde_json::to_string(&json!({ "b": 1, "a": 2 })).unwrap();
    let mode = if order == r#"{"b":1,"a":2}"# {
        "ordemDeInsercao"
    } else {
        "chavesOrdenadas"
    };
    fixture("corpos-gemini-d04.json")[mode].clone()
}

fn engine<'a>(
    client: &'a warden_ai::providers::gemini::GeminiClient,
    registry: &'a ToolRegistry,
    source: &'a ValeSombrio,
    timer: &ManualTimer,
) -> Engine<'a> {
    Engine {
        provider: client,
        registry,
        source,
        observer: &NoObserver,
        timer: Arc::new(timer.clone()),
        cancel: CancellationToken::new(),
        limits: Limits::default(),
    }
}

#[tokio::test]
async fn ca_t14_22_corpo_do_gemini_identico_ao_da_d04_nas_gravacoes_reais() {
    let golden = golden();
    for name in RECORDINGS {
        let recorded = fixture(name);
        let responses: Vec<Value> = recorded["respostas"].as_array().unwrap().clone();
        let gemini = Gemini::start(Box::new(move |n, _| {
            (200, responses.get(n).cloned().unwrap_or(Value::Null))
        }))
        .await;
        let timer = ManualTimer::new();
        let client = gemini.client(&timer);
        let registry = ToolRegistry::new().with_extra(external_tools());
        let source = ValeSombrio::default();
        let engine = engine(&client, &registry, &source, &timer);
        let mut conv = conversation(AiPermissions::default());
        // Como o app: a primeira pergunta é contada antes de rodar.
        let questions = recorded["perguntas"].as_array().unwrap();
        push_question(&mut conv, questions[0].as_str().unwrap().to_owned());
        engine
            .count_tokens(&conv.model, &conv.messages)
            .await
            .unwrap()
            .unwrap();
        engine.run(&mut conv).await.unwrap();
        for question in &questions[1..] {
            engine
                .ask(&mut conv, question.as_str().unwrap().to_owned())
                .await
                .unwrap();
        }
        assert!(
            gemini.refusals().is_empty(),
            "{name}: {:?}",
            gemini.refusals()
        );

        let state = gemini.state.lock().unwrap();
        let sent: Vec<String> = state
            .raw
            .iter()
            .map(|body| sha256(&as_in_d04(body)))
            .collect();
        let expected: Vec<String> =
            serde_json::from_value(golden[name]["generateContent"].clone()).unwrap();
        assert_eq!(sent, expected, "{name}: generateContent");
        let counted: Vec<String> = state
            .count_tokens_raw
            .iter()
            .map(|b| sha256(&as_in_d04(b)))
            .collect();
        let expected: Vec<String> =
            serde_json::from_value(golden[name]["countTokens"].clone()).unwrap();
        assert_eq!(counted, expected, "{name}: countTokens");

        // O corpo de cada rodada fica na conversa, byte a byte (repetições iguais uma vez só).
        let mut distinct = state.raw.clone();
        distinct.dedup();
        let rounds: Vec<&str> = conv.rounds.iter().map(|r| r.body.as_str()).collect();
        assert_eq!(rounds, distinct, "{name}: corpos guardados");
    }
}

/// A conversa do formato 1 numa pasta de conversas (o arquivo como a D-04 gravou).
fn v1_store() -> (tempfile::TempDir, ConversationStore, String, Value) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(V1);
    let bytes = std::fs::read(path).unwrap();
    let raw: Value = serde_json::from_slice(&bytes).unwrap();
    let id = raw["id"].as_str().unwrap().to_owned();
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join(format!("{id}.json")), &bytes).unwrap();
    let store = ConversationStore::new(dir.path().to_path_buf());
    (dir, store, id, raw)
}

#[test]
fn ca_t14_22_conversa_do_formato_1_abre_igual() {
    let (_dir, store, id, raw) = v1_store();
    assert_eq!(raw["schemaVersion"], 1);
    let conv = store.load(&id).unwrap();
    assert_eq!(conv.schema_version, SCHEMA_VERSION);
    assert_eq!(conv.provider, AiProvider::Gemini);
    assert_eq!(conv.model, raw["model"].as_str().unwrap());
    // O que a tela mostra (perguntas, consultas, respostas, uso) é o que estava gravado.
    let turns: Vec<ConversationTurn> = serde_json::from_value(raw["turns"].clone()).unwrap();
    assert_eq!(conv.turns, turns);
    assert_eq!(
        conv.usage,
        serde_json::from_value(raw["usage"].clone()).unwrap()
    );
    assert_eq!(conv.requests, 6);
    assert_eq!(conv.turns.len(), 2);
    // O histórico convertido volta para o Gemini exatamente como estava no arquivo.
    let contents = raw["contents"].as_array().unwrap();
    assert_eq!(conv.messages.len(), contents.len());
    let back: Vec<Value> = conv
        .messages
        .iter()
        .filter_map(warden_ai::providers::gemini::translate::content)
        .collect();
    assert_eq!(
        serde_json::to_string(&back).unwrap(),
        serde_json::to_string(contents).unwrap()
    );
    assert_eq!(conv.messages[0].role, Role::User);
    assert!(conv.messages.iter().any(|m| m.role == Role::Tool));
    // Gravar de novo escreve o formato 2, sem o `contents`.
    store.save(&conv).unwrap();
    let saved: Value =
        serde_json::from_slice(&std::fs::read(store.dir().join(format!("{id}.json"))).unwrap())
            .unwrap();
    assert_eq!(saved["schemaVersion"], 2);
    assert_eq!(saved["provider"], "gemini");
    assert!(saved.get("contents").is_none());
    let again: Conversation = store.load(&id).unwrap();
    assert_eq!(again.messages, conv.messages);
}

#[tokio::test]
async fn ca_t14_22_conversa_do_formato_1_continua_com_o_gemini_e_o_mesmo_corpo() {
    let (_dir, store, id, _) = v1_store();
    let mut conv = store.load(&id).unwrap();
    let gemini = Gemini::start(Box::new(|_, _| support::respond(&support::simple_answer()))).await;
    let timer = ManualTimer::new();
    let client = gemini.client(&timer);
    let registry = ToolRegistry::new().with_extra(external_tools());
    let source = ValeSombrio::default();
    let engine = engine(&client, &registry, &source, &timer);
    engine
        .ask(&mut conv, "E se eu atualizar o Steam 'n' Rails?".to_owned())
        .await
        .unwrap();
    // O simulado recusa histórico alterado e assinatura ausente, como a API real.
    assert!(gemini.refusals().is_empty(), "{:?}", gemini.refusals());
    assert_eq!(conv.turns.len(), 3);
    assert!(conv.turns[2].answer.is_some());
    let raw = gemini.raw();
    assert_eq!(
        sha256(&as_in_d04(&raw[0])),
        golden()["continuacao"].as_str().unwrap(),
        "o primeiro corpo da pergunta nova é o que a D-04 mandaria"
    );
}
