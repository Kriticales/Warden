//! Testes da conversa sem pack (D-19) com o mclo.gs, o pastebin e o Gemini simulados
//! (`wiremock`): os links aceitos e recusados sem rede (CA-T14-25), a redação, o teto e o
//! tempo-limite (CA-T14-26), o que vai à rede antes do consentimento (CA-T14-27), as
//! ferramentas sem pack (CA-T14-28) e a conversa guardada em `ai/_sem-pack/` (CA-T14-29).

use std::fmt::Write as _;
use std::time::Duration;

use serde_json::{Value, json};
use warden_ai::conversation::{AiSymptom, ConversationOrigin, PasteSite};
use warden_ai::evidence::ClaimStatus;
use warden_ai::tools::{AiPermissions, ToolSource as _};
use warden_core::CancellationToken;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::{
    LINK_TIMEOUT, PasteBases, PasteLink, STANDALONE_DIR, StandaloneTools, TEST_BASES, fetch,
    parse_link, registry,
};
use crate::ai_tools::tests::{Setup, calls, setup};
use crate::commands::ai::{
    AiStandalonePreviewRequest, AiStandaloneSource, remember_pick, standalone_delete_impl,
    standalone_get_impl, standalone_list_impl, standalone_preview_impl, standalone_run_impl,
    standalone_start_impl,
};
use crate::error::{AppError, AppErrorCode, ErrorCode};
use crate::state::tests::test_state;

/// Um crash com dados pessoais (os mesmos do CA-T14-04).
const LOG: &str = "[21:10:02] [main/INFO]: --gameDir C:\\Users\\Maria\\AppData\\Roaming\\x --username Jogador\n\
[21:11:41] [Server thread/INFO]: Jogador joined the game from 192.168.0.42\n\
[21:12:00] [main/INFO]: chave $2a$10$abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0\n\
[21:14:07] [Server thread/ERROR]: java.lang.NoSuchMethodError: getPositionVec em C:\\Users\\Maria\\mods\n";

const PERSONAL: [&str; 4] = ["Maria", "192.168.0.42", "$2a$10$abc", "Jogador"];

fn code(error: &AppError) -> AppErrorCode {
    match error.code {
        ErrorCode::App(code) => code,
        other => panic!("código inesperado: {other}"),
    }
}

/// Os sites simulados: o mclo.gs em `/1/raw/<id>` e o pastebin em `/raw/<id>` do mesmo servidor.
async fn paste_server() -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/1/raw/abc123"))
        .respond_with(ResponseTemplate::new(200).set_body_string(LOG))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/raw/Xy12Ab34"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string("[main/ERROR]: linha do pastebin\n"),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/1/raw/sumiu"))
        .respond_with(
            ResponseTemplate::new(404)
                .set_body_json(json!({"success": false, "error": "Log not found."})),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/1/raw/devagar"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(LOG)
                .set_delay(Duration::from_secs(5)),
        )
        .mount(&server)
        .await;
    server
}

fn bases(server: &MockServer) -> PasteBases {
    PasteBases {
        mclogs: format!("{}/1/raw/", server.uri()),
        pastebin: format!("{}/raw/", server.uri()),
    }
}

/// Aponta os sites para o simulado (nesta thread de teste).
fn use_bases(server: &MockServer) {
    TEST_BASES.with(|slot| *slot.borrow_mut() = Some(bases(server)));
}

fn request(url: &str) -> AiStandalonePreviewRequest {
    AiStandalonePreviewRequest {
        symptom: AiSymptom::CrashInWorld,
        source: AiStandaloneSource::Link {
            url: url.to_owned(),
        },
        note: None,
    }
}

fn generated(setup: &Setup) -> Vec<String> {
    setup
        .seen
        .lock()
        .unwrap()
        .iter()
        .filter(|(url, _)| url.ends_with(":generateContent"))
        .map(|(_, body)| body.clone())
        .collect()
}

fn everything_sent(setup: &Setup) -> String {
    setup
        .seen
        .lock()
        .unwrap()
        .iter()
        .map(|(_, body)| body.clone())
        .collect()
}

#[test]
fn ca_t14_25_links_aceitos_e_recusados() {
    let link = parse_link("  https://mclo.gs/abc123  ").unwrap();
    assert_eq!(
        link,
        PasteLink {
            site: PasteSite::Mclogs,
            id: "abc123".into(),
            url: "https://mclo.gs/abc123".into(),
        }
    );
    // A âncora de linha e o endereço da própria API também valem; sem esquema, vale https.
    assert_eq!(
        parse_link("https://mclo.gs/abc123#L12").unwrap().id,
        "abc123"
    );
    assert_eq!(
        parse_link("https://api.mclo.gs/1/raw/abc123").unwrap().id,
        "abc123"
    );
    assert_eq!(
        parse_link("mclo.gs/abc123").unwrap().site,
        PasteSite::Mclogs
    );
    let paste = parse_link("https://pastebin.com/Xy12Ab34").unwrap();
    assert_eq!(
        (paste.site, paste.id.as_str()),
        (PasteSite::Pastebin, "Xy12Ab34")
    );
    assert_eq!(
        parse_link("https://www.pastebin.com/raw/Xy12Ab34")
            .unwrap()
            .id,
        "Xy12Ab34"
    );
    let official = PasteBases {
        mclogs: "https://api.mclo.gs/1/raw/".into(),
        pastebin: "https://pastebin.com/raw/".into(),
    };
    assert_eq!(
        official.raw_url(&link).unwrap().as_str(),
        "https://api.mclo.gs/1/raw/abc123"
    );
    assert_eq!(
        official.raw_url(&paste).unwrap().as_str(),
        "https://pastebin.com/raw/Xy12Ab34"
    );

    let refused = |text: &str| code(&parse_link(text).unwrap_err());
    assert_eq!(
        refused("http://mclo.gs/abc123"),
        AppErrorCode::AiLinkNotHttps
    );
    assert_eq!(
        refused("http://pastebin.com/Xy12Ab34"),
        AppErrorCode::AiLinkNotHttps
    );
    assert_eq!(
        refused("https://hastebin.com/abcdef"),
        AppErrorCode::AiLinkNeedsKey
    );
    assert_eq!(
        refused("https://www.toptal.com/developers/hastebin/abcdef"),
        AppErrorCode::AiLinkNeedsKey
    );
    for other in [
        "https://example.com/abc123",
        "https://mclo.gs.example.com/abc123",
        "https://gist.github.com/x/y",
        "não é link",
        "",
    ] {
        assert_eq!(refused(other), AppErrorCode::AiLinkUnsupported, "{other}");
    }
    assert_eq!(refused("https://mclo.gs/"), AppErrorCode::AiLinkInvalid);
    assert_eq!(
        refused("https://pastebin.com/u/alguem"),
        AppErrorCode::AiLinkInvalid
    );
    assert_eq!(
        refused("https://mclo.gs/abc%2F.."),
        AppErrorCode::AiLinkInvalid
    );
}

#[tokio::test]
async fn ca_t14_25_site_recusado_nao_faz_requisicao() {
    let setup = setup(Box::new(|_, _| unreachable!("nada pode ir à IA"))).await;
    let paste = paste_server().await;
    use_bases(&paste);
    for url in [
        "https://example.com/abc123",
        "http://mclo.gs/abc123",
        "https://hastebin.com/abcdef",
    ] {
        let error = standalone_preview_impl(&setup.state, request(url))
            .await
            .unwrap_err();
        assert!(
            matches!(
                code(&error),
                AppErrorCode::AiLinkUnsupported
                    | AppErrorCode::AiLinkNotHttps
                    | AppErrorCode::AiLinkNeedsKey
            ),
            "{error:?}"
        );
    }
    // Nem o site do link, nem a IA receberam nada.
    assert!(paste.received_requests().await.unwrap().is_empty());
    assert!(setup.seen.lock().unwrap().is_empty());
}

#[tokio::test]
async fn ca_t14_26_teto_fica_com_o_fim_e_tempo_limite() {
    let paste = MockServer::start().await;
    let mut big = String::new();
    for n in 1..=500 {
        let _ = writeln!(big, "linha {n:04}");
    }
    Mock::given(method("GET"))
        .and(path("/1/raw/grande"))
        .respond_with(ResponseTemplate::new(200).set_body_string(big.clone()))
        .mount(&paste)
        .await;
    Mock::given(method("GET"))
        .and(path("/1/raw/devagar"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string("x")
                .set_delay(Duration::from_secs(5)),
        )
        .mount(&paste)
        .await;
    let http = warden_http::HttpClient::new(warden_http::HttpConfig::for_version("0.0.0")).unwrap();
    let cancel = CancellationToken::new();

    let link = parse_link("https://mclo.gs/grande").unwrap();
    let fetched = fetch(&http, &link, &bases(&paste), 1000, LINK_TIMEOUT, &cancel)
        .await
        .unwrap();
    assert!(fetched.truncated);
    assert!(fetched.text.len() <= 1000);
    assert!(fetched.text.ends_with("linha 0500\n"));
    // A primeira linha cortada pela metade sai.
    assert!(
        fetched.text.starts_with("linha "),
        "{}",
        &fetched.text[..20]
    );
    let whole = fetch(&http, &link, &bases(&paste), 1 << 20, LINK_TIMEOUT, &cancel)
        .await
        .unwrap();
    assert!(!whole.truncated);
    assert_eq!(whole.text, big);

    let slow = parse_link("https://mclo.gs/devagar").unwrap();
    let started = std::time::Instant::now();
    let error = fetch(
        &http,
        &slow,
        &bases(&paste),
        1 << 20,
        Duration::from_millis(300),
        &cancel,
    )
    .await
    .unwrap_err();
    assert_eq!(code(&error), AppErrorCode::AiLinkTimeout);
    assert!(error.retryable);
    assert_eq!(
        error.params.get("site").map(String::as_str),
        Some("mclo.gs")
    );
    assert!(
        started.elapsed() < Duration::from_secs(4),
        "uma tentativa só"
    );
}

#[tokio::test]
async fn ca_t14_26_link_inexistente_nao_cria_conversa() {
    let setup = setup(Box::new(|_, _| unreachable!("nada pode ir à IA"))).await;
    let paste = paste_server().await;
    use_bases(&paste);
    let error = standalone_preview_impl(&setup.state, request("https://mclo.gs/sumiu"))
        .await
        .unwrap_err();
    assert_eq!(code(&error), AppErrorCode::AiLinkNotFound);
    assert!(standalone_list_impl(&setup.state).unwrap().is_empty());
    assert!(generated(&setup).is_empty());
}

#[tokio::test]
async fn ca_t14_26_e_27_link_redigido_e_so_o_texto_pedido_ao_site() {
    let setup = setup(Box::new(|_, _| {
        unreachable!("cancelado: nada pode ir à IA")
    }))
    .await;
    let paste = paste_server().await;
    use_bases(&paste);
    let preview = standalone_preview_impl(&setup.state, request("https://mclo.gs/abc123"))
        .await
        .unwrap();
    // CA-T14-26: nada pessoal no texto mostrado; o erro continua lá.
    for secret in PERSONAL {
        assert!(
            !preview.text.contains(secret),
            "{secret} vazou: {}",
            preview.text
        );
    }
    assert!(preview.text.contains("NoSuchMethodError"));
    assert!(preview.text.contains("link do mclo.gs"));
    assert!(preview.text.contains("session_id \"link\""));
    // CA-T14-27: o link não está no texto, e o único pedido ao site foi o do texto puro.
    assert!(!preview.text.contains("abc123"));
    let asked: Vec<String> = paste
        .received_requests()
        .await
        .unwrap()
        .iter()
        .map(|request| format!("{} {}", request.method, request.url.path()))
        .collect();
    assert_eq!(asked, ["GET /1/raw/abc123"]);
    // Cancelar = não chamar o start: nada do log foi à IA (só a lista de modelos e a contagem
    // do system prompt).
    assert!(generated(&setup).is_empty());
    let sent = everything_sent(&setup);
    assert!(!sent.contains("NoSuchMethodError"), "{sent}");
    assert!(!sent.contains("abc123"), "{sent}");
    assert!(standalone_list_impl(&setup.state).unwrap().is_empty());
}

#[tokio::test]
async fn ca_t14_28_so_ferramentas_sem_pack() {
    let names: Vec<&str> = registry().names().collect();
    assert_eq!(names, ["search_log", "read_log", "search_mod_issues"]);
    // Nenhuma ferramenta do pack nem de proposta é declarada.
    let declared: Vec<String> = registry()
        .declarations()
        .into_iter()
        .map(|declaration| declaration.name)
        .collect();
    assert!(!declared.iter().any(|name| name.starts_with("propose_")));
    assert!(
        !declared
            .iter()
            .any(|name| name == "list_mods" || name == "get_findings")
    );

    // Uma ferramenta do pack pedida mesmo assim não lê nada.
    let setup = setup(Box::new(|_, _| unreachable!())).await;
    let tools = StandaloneTools::new(&setup.state, None, CancellationToken::new());
    let error = tools.call("list_mods", &json!({})).await.unwrap_err();
    assert!(error.contains("sem pack"), "{error}");
    // O slug da busca de issues vai no caminho da API do Modrinth: nada de `/` nem `..`.
    let error = tools
        .call(
            "search_mod_issues",
            &json!({ "mod_id": "../v2/user", "query": "NoSuchMethodError" }),
        )
        .await
        .unwrap_err();
    assert!(error.contains("slug do Modrinth"), "{error}");
    assert!(tools.known_mods().await.get("create").is_none());
}

/// A última `functionResponse` da requisição: o id e o texto do primeiro item.
fn first_item(request: &Value) -> (String, String) {
    let contents = request["contents"].as_array().unwrap();
    let item = &contents.last().unwrap()["parts"][0]["functionResponse"]["response"]["itens"][0];
    (
        item["id"].as_str().unwrap().to_owned(),
        item["texto"].as_str().unwrap().to_owned(),
    )
}

#[tokio::test]
async fn ca_t14_29_conversa_sem_pack_guardada_reaberta_e_apagada() {
    let setup = setup(Box::new(|n, request| {
        if n == 0 {
            calls(&[(
                "search_log",
                json!({ "session_id": "link", "query": "NoSuchMethodError" }),
            )])
        } else {
            let (id, text) = first_item(request);
            calls(&[(
                "responder",
                json!({ "resumo": "Um mod chama um método que não existe.", "semConclusao": false, "propostas": [],
                    "achados": [{ "afirmacao": "O jogo travou num NoSuchMethodError.", "mods": [], "confianca": "media",
                        "evidencias": [{ "id": id, "citacao": text }] }] }),
            )])
        }
    }))
    .await;
    let paste = paste_server().await;
    use_bases(&paste);
    let packs_before = setup.state.packs.list().len();
    let preview = standalone_preview_impl(&setup.state, request("https://mclo.gs/abc123"))
        .await
        .unwrap();
    let id =
        standalone_start_impl(&setup.state, &preview.preview_id, AiPermissions::default()).unwrap();
    // A prévia é de uso único.
    assert!(
        standalone_start_impl(&setup.state, &preview.preview_id, AiPermissions::default()).is_err()
    );
    standalone_run_impl(&setup.state, &id, std::sync::Arc::new(|_| {}))
        .await
        .unwrap();

    // O primeiro pedido à IA é o texto mostrado, só com as ferramentas sem pack.
    let sent = generated(&setup);
    let first: Value = serde_json::from_str(&sent[0]).unwrap();
    assert_eq!(first["contents"][0]["parts"][0]["text"], preview.text);
    let tools: Vec<&str> = first["tools"][0]["functionDeclarations"]
        .as_array()
        .unwrap()
        .iter()
        .map(|declaration| declaration["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        tools,
        ["search_log", "read_log", "search_mod_issues", "responder"]
    );
    let everything = everything_sent(&setup);
    for secret in PERSONAL {
        assert!(!everything.contains(secret), "{secret} foi enviado");
    }
    assert!(!everything.contains("abc123"), "o link não vai à IA");

    // Guardada em ai/_sem-pack/, com a cópia redigida do log ao lado; nenhum pack novo.
    let dir = setup.state.paths.data_dir().join("ai").join(STANDALONE_DIR);
    assert!(dir.starts_with(&setup.data));
    assert!(dir.join(format!("{id}.json")).is_file());
    let log = std::fs::read_to_string(dir.join(format!("{id}.log.txt"))).unwrap();
    assert!(log.contains("NoSuchMethodError") && !log.contains("Maria"));
    assert_eq!(setup.state.packs.list().len(), packs_before);

    // "Reiniciar o app": outro estado na mesma pasta de dados.
    let reopened = test_state(&setup.data);
    let list = standalone_list_impl(&reopened).unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, id);
    assert!(matches!(
        &list[0].origin,
        ConversationOrigin::PasteLink { site: PasteSite::Mclogs, url } if url == "https://mclo.gs/abc123"
    ));
    assert_eq!(
        list[0].title.as_deref(),
        Some("O jogo travou num NoSuchMethodError.")
    );
    let view = standalone_get_impl(&reopened, &id).unwrap();
    assert_eq!(view.turns[0].question, preview.text);
    assert_eq!(view.turns[0].steps[0].name, "search_log");
    assert_eq!(
        view.turns[0].answer.as_ref().unwrap().checked.claims[0].status,
        ClaimStatus::Verified
    );
    assert!(view.proposals.is_empty());

    standalone_delete_impl(&reopened, &id).unwrap();
    assert!(!dir.join(format!("{id}.json")).exists());
    assert!(!dir.join(format!("{id}.log.txt")).exists());
    assert!(standalone_list_impl(&reopened).unwrap().is_empty());
}

#[tokio::test]
async fn ca_t14_25_pastebin_baixa_o_texto_puro() {
    // O link do pastebin vira o pedido a `pastebin.com/raw/<id>` (no simulado, `/raw/<id>`).
    let setup = setup(Box::new(|_, _| unreachable!("nada pode ir à IA"))).await;
    let paste = paste_server().await;
    use_bases(&paste);
    let preview = standalone_preview_impl(&setup.state, request("https://pastebin.com/Xy12Ab34"))
        .await
        .unwrap();
    assert!(
        preview.text.contains("link do pastebin"),
        "{}",
        preview.text
    );
    assert!(
        preview.text.contains("linha do pastebin"),
        "{}",
        preview.text
    );
    assert!(!preview.text.contains("Xy12Ab34"), "o link não vai à IA");
    let asked: Vec<String> = paste
        .received_requests()
        .await
        .unwrap()
        .iter()
        .map(|request| format!("{} {}", request.method, request.url.path()))
        .collect();
    assert_eq!(asked, ["GET /raw/Xy12Ab34"]);
    assert!(generated(&setup).is_empty());
}

#[tokio::test]
async fn ca_t14_28_busca_de_issues_so_com_a_permissao() {
    // Sem a permissão do GitHub, a busca de issues pedida pela IA é recusada sem rede.
    let setup = setup(Box::new(|n, _| {
        if n == 0 {
            calls(&[(
                "search_mod_issues",
                json!({ "mod_id": "sodium", "query": "NoSuchMethodError" }),
            )])
        } else {
            calls(&[(
                "responder",
                json!({ "resumo": "Não deu para concluir.", "semConclusao": true, "propostas": [],
                    "achados": [] }),
            )])
        }
    }))
    .await;
    let paste = paste_server().await;
    use_bases(&paste);
    let preview = standalone_preview_impl(&setup.state, request("https://mclo.gs/abc123"))
        .await
        .unwrap();
    let permissions = AiPermissions {
        configs: false,
        github: false,
    };
    let id = standalone_start_impl(&setup.state, &preview.preview_id, permissions).unwrap();
    standalone_run_impl(&setup.state, &id, std::sync::Arc::new(|_| {}))
        .await
        .unwrap();
    let view = standalone_get_impl(&setup.state, &id).unwrap();
    let step = &view.turns[0].steps[0];
    assert_eq!(step.name, "search_mod_issues");
    assert!(step.denied, "{step:?}");
    assert!(step.external.is_none(), "nada foi pedido ao GitHub");
    // Fora a IA, só o pedido do texto do link saiu.
    assert_eq!(paste.received_requests().await.unwrap().len(), 1);
}

#[tokio::test]
async fn arquivo_do_computador_sem_pack() {
    let setup = setup(Box::new(|_, _| unreachable!("nada pode ir à IA"))).await;
    let file = setup.dir.path().join("crash-2026.txt");
    std::fs::write(&file, LOG).unwrap();
    let pick = remember_pick(&setup.state, file).unwrap();
    let preview = standalone_preview_impl(
        &setup.state,
        AiStandalonePreviewRequest {
            symptom: AiSymptom::CrashOnStart,
            source: AiStandaloneSource::LocalFile { token: pick.token },
            note: Some("abre e fecha".into()),
        },
    )
    .await
    .unwrap();
    assert!(preview.text.contains("\"crash-2026.txt\""));
    assert!(preview.text.contains("session_id \"arquivo\""));
    assert!(preview.text.contains("abre e fecha"));
    for secret in PERSONAL {
        assert!(!preview.text.contains(secret), "{secret} vazou");
    }
    let id =
        standalone_start_impl(&setup.state, &preview.preview_id, AiPermissions::default()).unwrap();
    let list = standalone_list_impl(&setup.state).unwrap();
    assert!(matches!(
        &list[0].origin,
        ConversationOrigin::LocalFile { file_name } if file_name == "crash-2026.txt"
    ));
    assert_eq!(list[0].id, id);
}
