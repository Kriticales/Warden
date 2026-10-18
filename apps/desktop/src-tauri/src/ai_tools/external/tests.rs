//! As consultas da D-14 sobre um pack de verdade, com o Modrinth e o GitHub simulados
//! (`wiremock`) e o packwiz de `binaries/`: a busca de issues acha o repositório pelo link do
//! projeto, manda só repositório e termos, usa o token quando existe e volta com o pedido exato
//! (CA-T14-03 e CA-T14-13); o changelog vem entre a versão instalada e a mais nova, pelo
//! "Trocar de versão"; a busca do culpado e o raio-x sem dados dizem isso.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde_json::{Value, json};
use warden_ai::tools::ToolSource as _;
use warden_core::PackId;
use warden_project::registry::PackRecord;
use warden_project::transaction::PackTransaction;
use warden_secrets::{SecretKind, SecretString};
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::super::PackTools;
use crate::commands::inventory::tests::{packwiz_binary, test_packwiz};
use crate::state::AppState;
use crate::state::tests::test_state;

const SODIUM: &str = "AANobbMI";

fn version(id: &str, number: &str, date: &str, notes: &str) -> Value {
    json!({
        "id": id, "project_id": SODIUM, "version_number": number, "version_type": "release",
        "game_versions": ["1.20.1"], "loaders": ["fabric"], "date_published": date,
        "changelog": notes,
        "files": [{ "url": format!("https://cdn.modrinth.com/data/{SODIUM}/{id}.jar"),
                    "filename": format!("sodium-{number}.jar"), "primary": true,
                    "hashes": { "sha1": "a".repeat(40), "sha512": "b".repeat(128) } }]
    })
}

fn versions() -> Value {
    json!([
        version(
            "v3",
            "0.5.3",
            "2026-09-20T10:00:00Z",
            "Corrige o travamento ao renderizar entidades em C:\\Users\\Maria\\x."
        ),
        version(
            "v2",
            "0.5.2",
            "2026-09-10T10:00:00Z",
            "Melhora o desempenho."
        ),
        version("v1", "0.5.1", "2026-09-01T10:00:00Z", "Instalada."),
    ])
}

/// Modrinth simulado: o projeto do Sodium (gravação real, com os links do GitHub) e três
/// versões com as notas.
async fn modrinth() -> MockServer {
    let server = MockServer::start().await;
    let project: Value = serde_json::from_str(include_str!(
        "../../../../../../crates/warden-modrinth/tests/fixtures/http/2026-10-04-project-sodium.json"
    ))
    .unwrap();
    Mock::given(method("GET"))
        .and(path(format!("/v2/project/{SODIUM}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(project.clone()))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v2/projects"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([project])))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("/v2/project/{SODIUM}/version")))
        .respond_with(ResponseTemplate::new(200).set_body_json(versions()))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v2/versions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(versions()))
        .mount(&server)
        .await;
    server
}

/// Um pack Fabric 1.20.1 com o Sodium 0.5.1 do Modrinth e um mod só com link.
async fn pack(state: &AppState, dir: &Path) -> Option<(PackId, std::path::PathBuf)> {
    let cli = test_packwiz(state, packwiz_binary()?);
    let root = dir.join("pack");
    fs::create_dir_all(root.join("mods")).unwrap();
    let mut manifest = warden_packwiz::PackManifest::new("Vale D14", "1.20.1");
    manifest
        .versions
        .get_or_insert_with(BTreeMap::new)
        .insert("fabric".into(), "0.16.10".into());
    fs::write(root.join("pack.toml"), manifest.to_toml_string()).unwrap();
    fs::write(
        root.join("index.toml"),
        warden_packwiz::PackIndex::default().to_toml_string(),
    )
    .unwrap();
    let sodium = warden_packwiz::ModrinthFile {
        title: "Sodium".into(),
        project_id: SODIUM.into(),
        version_id: "v1".into(),
        filename: "sodium-0.5.1.jar".into(),
        url: format!("https://cdn.modrinth.com/data/{SODIUM}/v1.jar"),
        hash_format: warden_packwiz::HashFormat::Sha512,
        hash: "b".repeat(128),
    };
    fs::write(
        root.join("mods/sodium.pw.toml"),
        warden_packwiz::Metafile::modrinth(&sodium, warden_packwiz::Side::Client).to_toml_string(),
    )
    .unwrap();
    fs::write(
        root.join("mods/avulso.pw.toml"),
        "name = \"Avulso\"\nfilename = \"avulso.jar\"\nside = \"both\"\n\n[download]\nurl = \"https://example.com/avulso.jar\"\nhash-format = \"sha1\"\nhash = \"0123456789abcdef0123456789abcdef01234567\"\n",
    )
    .unwrap();
    PackTransaction::new(root.clone())
        .commit(&cli, &warden_core::CancellationToken::new())
        .await
        .unwrap();
    let record = PackRecord {
        id: PackId::new(),
        name: "Vale D14".into(),
        path: root.clone(),
        last_test: None,
        extra: BTreeMap::new(),
    };
    let id = record.id;
    state.packs.insert(record).unwrap();
    Some((id, root))
}

fn modrinth_client(server: &MockServer) -> warden_modrinth::ModrinthClient {
    let http = warden_http::HttpClient::new(warden_http::HttpConfig::for_version("0.0.0")).unwrap();
    warden_modrinth::ModrinthClient::with_base_url(http, &format!("{}/v2", server.uri()), None)
        .unwrap()
}

#[tokio::test]
async fn ca_t14_13_issues_pelo_link_do_projeto_so_repositorio_e_termos_com_token() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = test_state(&dir.path().join("dados"));
    let api = modrinth().await;
    state.modrinth = modrinth_client(&api);
    let Some((pack_id, root)) = pack(&state, dir.path()).await else {
        return;
    };
    let github = MockServer::start().await;
    Mock::given(path("/search/issues"))
        .and(query_param(
            "q",
            "repo:CaffeineMC/sodium is:issue NullPointerException EntityRenderer",
        ))
        .and(wiremock::matchers::header(
            "authorization",
            "Bearer ghp-token-de-teste",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "items": [
            { "number": 2210, "title": "NPE em EntityRenderer", "state": "closed",
              "html_url": "https://github.com/CaffeineMC/sodium/issues/2210" }
        ] })))
        .expect(1)
        .mount(&github)
        .await;
    *state.ai.github.api.lock().unwrap() = Some(github.uri());
    state
        .secrets
        .set(
            SecretKind::Github,
            &SecretString::from("ghp-token-de-teste"),
        )
        .unwrap();

    let tools = PackTools::new(&state, pack_id, root, None);
    let output = tools
        .call(
            "search_mod_issues",
            &json!({ "mod_id": "sodium", "query": "NullPointerException at EntityRenderer" }),
        )
        .await
        .unwrap();
    assert_eq!(output.items[0].id, "issue:CaffeineMC/sodium#2210");
    assert!(output.third_party);
    let request = output.external.unwrap();
    let received = github.received_requests().await.unwrap();
    assert_eq!(
        request.request,
        format!(
            "GET {}{}?{}",
            github.uri(),
            received[0].url.path(),
            received[0].url.query().unwrap()
        )
    );
    assert_eq!(request.repository, "CaffeineMC/sodium");
    assert!(
        !request.request.contains("ghp-token"),
        "o token nunca aparece no bloco"
    );

    // Um mod que não vem do Modrinth nem da CurseForge não tem repositório conhecido.
    let error = tools
        .call(
            "search_mod_issues",
            &json!({ "mod_id": "avulso", "query": "NullPointerException" }),
        )
        .await
        .unwrap_err();
    assert!(
        error.contains("não vem do Modrinth nem da CurseForge"),
        "{error}"
    );
    assert_eq!(github.received_requests().await.unwrap().len(), 1);
}

#[tokio::test]
async fn busca_que_falha_no_github_ainda_mostra_o_pedido_enviado() {
    // O GitHub recusa (repositório renomeado): o pedido saiu, então o bloco "Enviado ao GitHub"
    // aparece igual, e a IA recebe o motivo.
    let dir = tempfile::tempdir().unwrap();
    let mut state = test_state(&dir.path().join("dados"));
    let api = modrinth().await;
    state.modrinth = modrinth_client(&api);
    let Some((pack_id, root)) = pack(&state, dir.path()).await else {
        return;
    };
    let github = MockServer::start().await;
    Mock::given(path("/search/issues"))
        .respond_with(ResponseTemplate::new(422))
        .expect(1)
        .mount(&github)
        .await;
    *state.ai.github.api.lock().unwrap() = Some(github.uri());

    let tools = PackTools::new(&state, pack_id, root, None);
    let output = tools
        .call(
            "search_mod_issues",
            &json!({ "mod_id": "sodium", "query": "NullPointerException EntityRenderer" }),
        )
        .await
        .unwrap();
    let request = output.external.expect("o pedido que saiu aparece");
    let received = github.received_requests().await.unwrap();
    assert_eq!(
        request.request,
        format!(
            "GET {}{}?{}",
            github.uri(),
            received[0].url.path(),
            received[0].url.query().unwrap()
        )
    );
    assert_eq!(output.items[0].id, "issue:CaffeineMC/sodium");
    assert!(
        output.items[0].text.contains("falhou"),
        "{}",
        output.items[0].text
    );
}

#[tokio::test]
async fn cancelar_a_conversa_para_a_busca_no_github() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = test_state(&dir.path().join("dados"));
    let api = modrinth().await;
    state.modrinth = modrinth_client(&api);
    let Some((pack_id, root)) = pack(&state, dir.path()).await else {
        return;
    };
    let github = MockServer::start().await;
    Mock::given(path("/search/issues"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({ "items": [] }))
                .set_delay(std::time::Duration::from_secs(60)),
        )
        .mount(&github)
        .await;
    *state.ai.github.api.lock().unwrap() = Some(github.uri());

    let cancel = warden_core::CancellationToken::new();
    let tools = PackTools::new(&state, pack_id, root, None).with_cancel(cancel.clone());
    let canceller = cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        canceller.cancel();
    });
    let started = std::time::Instant::now();
    let output = tokio::time::timeout(
        std::time::Duration::from_secs(20),
        tools.call(
            "search_mod_issues",
            &json!({ "mod_id": "sodium", "query": "NullPointerException" }),
        ),
    )
    .await
    .expect("cancelar não parou a busca no GitHub")
    .unwrap();
    assert!(started.elapsed() < std::time::Duration::from_secs(20));
    assert!(output.items[0].text.contains("falhou"));
}

#[tokio::test]
async fn changelog_entre_a_instalada_e_a_mais_nova_redigido() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = test_state(&dir.path().join("dados"));
    let api = modrinth().await;
    state.modrinth = modrinth_client(&api);
    let Some((pack_id, root)) = pack(&state, dir.path()).await else {
        return;
    };
    let tools = PackTools::new(&state, pack_id, root, None);
    let output = tools
        .call("get_mod_changelog", &json!({ "mod_id": "sodium" }))
        .await
        .unwrap();
    let ids: Vec<&str> = output.items.iter().map(|item| item.id.as_str()).collect();
    assert_eq!(
        ids,
        ["changelog:sodium@0.5.3", "changelog:sodium@0.5.2"],
        "sem a instalada"
    );
    assert!(
        output.items[0]
            .text
            .starts_with("Versão 0.5.3 (2026-09-20):")
    );
    assert!(output.items[0].text.contains("Corrige o travamento"));
    assert!(
        !output.items[0].text.contains("Maria"),
        "{}",
        output.items[0].text
    );
    assert!(output.third_party);

    let only = tools
        .call(
            "get_mod_changelog",
            &json!({ "mod_id": "sodium", "from": "0.5.2", "to": "0.5.3" }),
        )
        .await
        .unwrap();
    assert_eq!(only.items.len(), 1);
    let wrong = tools
        .call(
            "get_mod_changelog",
            &json!({ "mod_id": "sodium", "from": "0.5.3", "to": "0.5.1" }),
        )
        .await
        .unwrap_err();
    assert!(wrong.contains("anterior"), "{wrong}");
}

#[tokio::test]
async fn busca_do_culpado_e_raio_x_sem_dados_dizem_isso() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = test_state(&dir.path().join("dados"));
    let api = modrinth().await;
    state.modrinth = modrinth_client(&api);
    let Some((pack_id, root)) = pack(&state, dir.path()).await else {
        return;
    };
    let tools = PackTools::new(&state, pack_id, root, None);
    let bisect = tools.call("get_bisect_result", &json!({})).await.unwrap();
    assert_eq!(bisect.items[0].id, "bisect:nenhuma");
    let xray = tools
        .call("get_mixin_report", &json!({ "mod_id": "sodium" }))
        .await
        .unwrap_err();
    assert!(xray.contains("ainda não está no computador"), "{xray}");
    let pack_wide = tools.call("get_mixin_report", &json!({})).await.unwrap();
    assert_eq!(pack_wide.items[0].id, "mixin:pack");
}
