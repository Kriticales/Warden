//! As propostas e as consultas externas da D-14 no laço, contra os servidores simulados do
//! Gemini (o da D-04, que exige as assinaturas) e do GitHub (`wiremock`), sem rede (ROADMAP D-14
//! "Verificação"). CA-T14-03 (bloco "Enviado ao GitHub"), CA-T14-11 (proposta sem evidência não
//! aparece), CA-T14-12 (a proposta não muda nada no laço) e CA-T14-13 (GitHub só com a caixa
//! marcada, só repositório e termos).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde_json::{Value, json};
use support::{Gemini, ValeSombrio, calls, conversation, respond, simple_answer};
use warden_ai::conversation::Conversation;
use warden_ai::engine::{Engine, Limits, NoObserver};
use warden_ai::external::{self, GithubIssues, GithubRepo, search_terms};
use warden_ai::proposals::{self, ProposalKind, ProposalState, ProposalSummary};
use warden_ai::tools::{
    AiPermissions, KnownMods, NOT_ALLOWED, ToolOutput, ToolRegistry, ToolSource,
};
use warden_core::CancellationToken;
use warden_diagnostics::{RedactionProfile, Redactor};
use warden_http::{HttpConfig, ManualTimer};
use wiremock::matchers::path;
use wiremock::{Mock, MockServer, ResponseTemplate};

/// O pack fictício do spike com as consultas da D-14: a busca de issues vai ao GitHub simulado
/// pela [`GithubIssues`] de verdade; as propostas são conferidas sem gravar nada.
struct Pack {
    inner: ValeSombrio,
    github: GithubIssues,
    redactor: Redactor,
    calls: Mutex<Vec<String>>,
}

impl Pack {
    fn new(github: &MockServer) -> Self {
        let mut config = HttpConfig::for_version("0.0.0");
        config.max_retries = 0;
        Self {
            inner: ValeSombrio::default(),
            github: GithubIssues::with_api(&config, &github.uri()).unwrap(),
            redactor: Redactor::new(RedactionProfile::new().with_player("Steve_42")),
            calls: Mutex::new(Vec::new()),
        }
    }
}

#[async_trait]
impl ToolSource for Pack {
    async fn call(&self, name: &str, args: &Value) -> Result<ToolOutput, String> {
        self.calls.lock().unwrap().push(name.to_owned());
        if name != external::SEARCH_MOD_ISSUES {
            return self.inner.call(name, args).await;
        }
        let repository =
            GithubRepo::from_links(None, Some("https://github.com/Layers-of-Railways/Railway"))
                .ok_or("sem repositório")?;
        let query = args.get("query").and_then(Value::as_str).unwrap_or("");
        let terms = search_terms(query, &self.redactor).map_err(|error| error.to_string())?;
        let search = self
            .github
            .search(&repository, &terms, None, &self.redactor, None)
            .await
            .map_err(|error| error.to_string())?;
        Ok(ToolOutput {
            items: search.items(),
            third_party: true,
            external: Some(search.request),
            ..ToolOutput::default()
        })
    }

    async fn known_mods(&self) -> KnownMods {
        self.inner.known_mods().await
    }

    async fn check_proposal(&self, change: &ProposalKind) -> Result<ProposalSummary, String> {
        Ok(match change {
            ProposalKind::UpdateMod { path } => ProposalSummary {
                target: Some(path.clone()),
                from: Some("1.6.4".into()),
                to: Some("1.6.7".into()),
            },
            _ => ProposalSummary::default(),
        })
    }
}

fn registry() -> ToolRegistry {
    ToolRegistry::new()
        .with_extra(external::tools())
        .with_extra(proposals::tools())
}

async fn run(
    script: support::Script,
    permissions: AiPermissions,
    github: &MockServer,
) -> (Gemini, Conversation, Vec<String>) {
    let gemini = Gemini::start(script).await;
    let timer = ManualTimer::new();
    let client = gemini.client(&timer);
    let registry = registry();
    let source = Pack::new(github);
    let engine = Engine {
        provider: &client,
        registry: &registry,
        source: &source,
        observer: &NoObserver,
        timer: Arc::new(timer),
        cancel: CancellationToken::new(),
        limits: Limits::default(),
    };
    let mut conv = conversation(permissions);
    engine
        .ask(&mut conv, "Trava ao entrar no mundo.".into())
        .await
        .unwrap();
    assert!(gemini.refusals().is_empty(), "{:?}", gemini.refusals());
    let calls = source.calls.lock().unwrap().clone();
    (gemini, conv, calls)
}

fn allowed(request: &Value) -> Vec<String> {
    request["toolConfig"]["functionCallingConfig"]["allowedFunctionNames"]
        .as_array()
        .unwrap()
        .iter()
        .map(|name| name.as_str().unwrap().to_owned())
        .collect()
}

#[tokio::test]
async fn ca_t14_12_proposta_no_laco_vira_cartao_sem_efeito_e_sem_evidencia_nem_aparece() {
    let github = MockServer::start().await;
    let script: support::Script = Box::new(|n, _| match n {
        0 => calls(&[("get_crash_report", json!({ "session_id": "s3" }))]),
        1 => calls(&[
            (
                "propose_update_mod",
                json!({ "mod_id": "railways", "motivo": "A versão nova acompanha o Create 6.",
                        "evidencias": [{ "id": "crash:s3", "citacao": "railways$tickBogeys" }] }),
            ),
            (
                "propose_remove_mod",
                json!({ "mod_id": "create", "motivo": "Inventado.",
                        "evidencias": [{ "id": "crash:s3", "citacao": "Create quebrou tudo" }] }),
            ),
        ]),
        // Sem achados nem propostas no campo: com o cartão da ferramenta, ainda tem conteúdo.
        _ => respond(
            &json!({ "resumo": "Atualizar o Steam 'n' Rails deve resolver.", "achados": [],
                              "propostas": [], "semConclusao": false }),
        ),
    });
    let (gemini, conv, tool_calls) = run(script, AiPermissions::default(), &github).await;

    // Uma proposta só, pendente: a da evidência inventada voltou como erro para a IA.
    assert_eq!(conv.proposals.len(), 1);
    let proposal = &conv.proposals[0];
    assert_eq!(proposal.id, "p1");
    assert_eq!(
        proposal.change,
        ProposalKind::UpdateMod {
            path: "mods/railways.pw.toml".into()
        }
    );
    assert_eq!(proposal.state, ProposalState::Pending);
    assert_eq!(proposal.summary.to.as_deref(), Some("1.6.7"));
    let steps = &conv.turns[0].steps;
    let update = steps
        .iter()
        .find(|s| s.name == "propose_update_mod")
        .unwrap();
    assert!(update.ok);
    assert!(update.sent.contains("ainda não foi aplicada"));
    let remove = steps
        .iter()
        .find(|s| s.name == "propose_remove_mod")
        .unwrap();
    assert!(!remove.ok);
    assert!(
        remove.sent.contains("AI_PROPOSAL_NO_EVIDENCE"),
        "{}",
        remove.sent
    );
    // Nenhuma proposta chega ao app como chamada de ferramenta (nada é executado).
    assert!(
        tool_calls.iter().all(|name| !proposals::is_proposal(name)),
        "{tool_calls:?}"
    );
    // A resposta final sem achados foi aceita de primeira (3 requisições, sem novas tentativas).
    assert!(conv.turns[0].answer.is_some());
    assert_eq!(gemini.requests().len(), 3);

    // Liberação por etapa: as propostas entram quando a IA já abriu um suspeito; as declarações
    // não mudam entre as rodadas.
    let requests = gemini.requests();
    assert!(
        !allowed(&requests[0])
            .iter()
            .any(|n| proposals::is_proposal(n))
    );
    assert!(allowed(&requests[0]).contains(&"get_bisect_result".to_owned()));
    assert!(!allowed(&requests[0]).contains(&"search_mod_issues".to_owned()));
    let later = allowed(&requests[1]);
    assert!(later.contains(&"propose_update_mod".to_owned()));
    assert!(later.contains(&"search_mod_issues".to_owned()));
    assert_eq!(requests[0]["tools"], requests[1]["tools"]);
}

fn issue_search_script(query: &'static str) -> support::Script {
    Box::new(move |n, _| match n {
        0 => calls(&[("get_crash_report", json!({ "session_id": "s3" }))]),
        1 => calls(&[(
            "search_mod_issues",
            json!({ "mod_id": "railways", "query": query }),
        )]),
        _ => respond(&simple_answer()),
    })
}

#[tokio::test]
async fn ca_t14_13_com_a_caixa_marcada_so_repositorio_e_termos_vao_ao_github() {
    let github = MockServer::start().await;
    Mock::given(path("/search/issues"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "items": [
            { "number": 412, "title": "NoSuchMethodError tickBogeys with Create 6", "state": "closed",
              "html_url": "https://github.com/Layers-of-Railways/Railway/issues/412" }
        ] })))
        .expect(1)
        .mount(&github)
        .await;
    let (_gemini, conv, _) = run(
        issue_search_script("NoSuchMethodError tickBogeys"),
        AiPermissions::default(),
        &github,
    )
    .await;

    let received = github.received_requests().await.unwrap();
    assert_eq!(received.len(), 1);
    let q: String = received[0]
        .url
        .query_pairs()
        .find(|(k, _)| k == "q")
        .map(|(_, v)| v.into_owned())
        .unwrap();
    assert_eq!(
        q,
        "repo:Layers-of-Railways/Railway is:issue NoSuchMethodError tickBogeys"
    );
    let step = conv.turns[0]
        .steps
        .iter()
        .find(|s| s.name == "search_mod_issues")
        .unwrap();
    let external = step.external.as_ref().expect("bloco Enviado ao GitHub");
    // CA-T14-03: o bloco é o pedido exato.
    assert_eq!(
        external.request,
        format!(
            "GET {}{}?{}",
            github.uri(),
            received[0].url.path(),
            received[0].url.query().unwrap()
        )
    );
    assert_eq!(external.repository, "Layers-of-Railways/Railway");
    // O resultado volta marcado como texto de terceiros e citável.
    assert!(step.sent.contains("issue:Layers-of-Railways/Railway#412"));
    assert!(step.sent.contains("escrito por terceiros"));
    assert!(
        conv.sent
            .iter()
            .any(|item| item.id == "issue:Layers-of-Railways/Railway#412")
    );
}

#[tokio::test]
async fn ca_t14_13_com_a_caixa_desmarcada_nada_vai_ao_github() {
    let github = MockServer::start().await;
    let permissions = AiPermissions {
        configs: true,
        github: false,
    };
    let (_gemini, conv, tool_calls) = run(
        issue_search_script("NoSuchMethodError tickBogeys"),
        permissions,
        &github,
    )
    .await;
    assert!(github.received_requests().await.unwrap().is_empty());
    assert!(!tool_calls.contains(&"search_mod_issues".to_owned()));
    let step = conv.turns[0]
        .steps
        .iter()
        .find(|s| s.name == "search_mod_issues")
        .unwrap();
    assert!(step.denied);
    assert!(step.external.is_none());
    assert!(step.sent.contains(NOT_ALLOWED));
}

#[tokio::test]
async fn ca_t14_13_linha_de_log_na_consulta_nao_sai() {
    let github = MockServer::start().await;
    let (_gemini, conv, _) = run(
        issue_search_script("[21:13:58] [Server thread/ERROR]: Steve_42 travou"),
        AiPermissions::default(),
        &github,
    )
    .await;
    assert!(github.received_requests().await.unwrap().is_empty());
    let step = conv.turns[0]
        .steps
        .iter()
        .find(|s| s.name == "search_mod_issues")
        .unwrap();
    assert!(!step.ok);
    assert!(step.external.is_none());
    assert!(step.sent.contains("só palavras do erro"), "{}", step.sent);
}
