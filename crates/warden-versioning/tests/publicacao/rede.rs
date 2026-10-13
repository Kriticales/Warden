//! CA-T18-01 com uma conta de teste real no GitHub, sob demanda (`#[ignore = "rede"]`).
//!
//! Como rodar (o orquestrador, com o dono):
//!
//! ```text
//! WARDEN_TEST_GITHUB_TOKEN='<token da conta descartável>' \
//!   cargo nextest run -p warden-versioning --run-ignored only -E 'test(rede_)'
//! ```
//!
//! O token precisa das permissões do passo a passo de Configurações (fine-grained: acesso a
//! todos os repositórios; Administration e Contents com leitura e escrita). O teste cria um
//! repositório **público** `warden-ca-t18-01-<data>` na conta, publica a versão 1.0.0 e confere
//! a `main` (exatamente `pack.toml`, `index.toml`, os arquivos do índice, `CHANGELOG.md` e
//! `.gitattributes`), a tag `v1.0.0` e a Release com as notas. O repositório fica na conta para
//! conferência; apague pelo site depois. Sem a variável (como no Noturno), o teste se declara
//! pulado e passa.

#![allow(clippy::print_stderr)] // o aviso de teste pulado

use std::time::{SystemTime, UNIX_EPOCH};

use secrecy::SecretString;
use warden_core::CancellationToken;
use warden_http::{HttpClient, HttpConfig};
use warden_versioning::github::{GithubClient, GithubEndpoints, RepoRef};
use warden_versioning::publish::{PublishRequest, publish};

use super::common::{TestPack, at, identity};
use super::{save, tree_of};

#[tokio::test]
#[ignore = "rede"]
async fn rede_ca_t18_01_primeira_publicacao_numa_conta_de_teste() {
    let Ok(token) = std::env::var("WARDEN_TEST_GITHUB_TOKEN") else {
        eprintln!(
            "AVISO: WARDEN_TEST_GITHUB_TOKEN ausente; teste pulado. Para rodar, defina a \
             variável com o token da conta de teste e rode `cargo nextest run -p \
             warden-versioning --run-ignored only -E 'test(rede_)'`."
        );
        return;
    };
    let client = GithubClient::new(
        HttpClient::new(HttpConfig::default()).unwrap(),
        GithubEndpoints::official(),
        SecretString::from(token),
    );
    let login = client.user(None).await.unwrap();
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let name = format!("warden-ca-t18-01-{stamp}");
    let created = client
        .create_repo(&login, &name, false, "Teste do Warden (CA-T18-01)")
        .await
        .unwrap();
    assert!(!created.private);
    let repo_ref = RepoRef { owner: login, name };

    let (pack, repo) = TestPack::versioned();
    drop(repo);
    pack.write("config/a.toml", b"a = 1\r\n");
    let repo = warden_versioning::PackRepo::open(&pack.root).unwrap();
    save(&repo, &pack, "1.0.0", 10, true);
    let tree = tree_of(&repo, "1.0.0");
    let id = identity();
    let published = publish(
        &client,
        &pack.root,
        &repo_ref,
        &PublishRequest {
            version: "1.0.0",
            tree: &tree,
            notes: "Notas da primeira versão.",
            pack_name: "Vale Sereno",
            identity: &id,
            when: at(1),
        },
        None,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert!(published.release.id > 0);
    let release = client
        .release_by_tag(&repo_ref, "v1.0.0")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(release.id, published.release.id);

    // A main pelo próprio link do pack e pela API de conteúdo: pack.toml e o resto da árvore.
    let http = HttpClient::new(HttpConfig::default()).unwrap();
    let raw = http
        .get(warden_http::parse_url(&published.pack_link).unwrap())
        .bytes()
        .await
        .unwrap();
    assert_eq!(raw, tree["pack.toml"]);
    let mut expected: Vec<String> = tree.keys().cloned().collect();
    expected.extend(["CHANGELOG.md".into(), ".gitattributes".into()]);
    expected.sort();
    let url = format!(
        "https://api.github.com/repos/{}/git/trees/main?recursive=1",
        repo_ref.full_name()
    );
    let listing: serde_json::Value = http
        .get(warden_http::parse_url(&url).unwrap())
        .json()
        .await
        .unwrap();
    let mut files: Vec<String> = listing["tree"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|entry| entry["type"] == "blob")
        .map(|entry| entry["path"].as_str().unwrap().to_owned())
        .collect();
    files.sort();
    assert_eq!(files, expected);
}
