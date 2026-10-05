//! Testes contra a API real do Modrinth (`cargo xtask test-network`; QUALITY §4.1). Marcados
//! `#[ignore = "rede"]`, com nome `rede_*`. O cache fica numa pasta temporária.

// Nos auxiliares dos testes, `unwrap`, `expect` e `panic!` são a forma idiomática de falhar
// (o `clippy.toml` só os libera dentro das funções `#[test]`).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use warden_core::{AppPaths, NoProgress};
use warden_http::{DownloadRequest, HttpClient, HttpConfig, parse_url};
use warden_modrinth::{
    Environment, Facets, HashAlgorithm, MetadataCache, ModrinthClient, ProjectType, SearchQuery,
    UpdateFilter, VersionFilter,
};
use warden_packwiz::HashFormat;
use warden_packwiz::hash::{curseforge_fingerprint, hash_bytes};

const SODIUM: &str = "AANobbMI";
const FABRIC_API: &str = "P7dR8mSH";

fn client(dir: &std::path::Path) -> ModrinthClient {
    let paths = AppPaths::from_dev_root(dir).unwrap();
    let cache = MetadataCache::open(&paths.metadata_db_file()).unwrap();
    let http = HttpClient::new(HttpConfig::default()).unwrap();
    ModrinthClient::with_base_url(http, warden_modrinth::DEFAULT_BASE_URL, Some(cache)).unwrap()
}

/// CA-3 da P1-03: busca "sodium" com facets de Fabric 1.21.1 retorna o Sodium.
#[tokio::test]
#[ignore = "rede"]
async fn rede_p1_03_ca3_busca_sodium_fabric_1_21_1() {
    let dir = tempfile::tempdir().unwrap();
    let client = client(dir.path());
    let query = SearchQuery::new("sodium").facets(
        Facets::new()
            .loaders(["fabric"])
            .game_versions(["1.21.1"])
            .project_type(ProjectType::Mod),
    );
    let results = client.search(&query, None).await.unwrap();
    assert!(results.total_hits > 0);
    let sodium = results
        .hits
        .iter()
        .find(|hit| hit.project_id == SODIUM)
        .expect("o Sodium não veio na busca");
    assert_eq!(sodium.slug, "sodium");
    assert!(sodium.categories.contains(&"fabric".to_owned()));
    assert!(sodium.versions.contains(&"1.21.1".to_owned()));
}

/// CA-3 da P1-03: `version_files` identifica um jar real. O jar é baixado pela `warden-http`
/// (que confere o sha512 da API e calcula os quatro hashes) e identificado pelo sha1.
#[tokio::test]
#[ignore = "rede"]
async fn rede_p1_03_ca3_version_files_identifica_um_jar_real() {
    let dir = tempfile::tempdir().unwrap();
    let client = client(dir.path());
    let filter = VersionFilter {
        loaders: vec!["fabric".into()],
        game_versions: vec!["1.21.1".into()],
        ..VersionFilter::default()
    };
    let versions = client
        .project_versions(SODIUM, &filter, None)
        .await
        .unwrap();
    let version = &versions[0];
    // R7 §9 item 4: a versão traz o próprio `environment`.
    assert_eq!(version.environment, Some(Environment::ClientOnly));
    let file = version.primary_file().unwrap();

    let http = HttpClient::new(HttpConfig::default()).unwrap();
    let destination = dir.path().join("downloads").join(&file.filename);
    let request = DownloadRequest::new(parse_url(&file.url).unwrap(), &destination)
        .expect_hash(HashFormat::Sha512, &file.hashes.sha512)
        .expect_size(file.size);
    let downloaded = http.download(&request, &NoProgress, None).await.unwrap();
    let bytes = std::fs::read(&destination).unwrap();
    assert_eq!(downloaded.hashes.sha1, hash_bytes(HashFormat::Sha1, &bytes));
    assert_eq!(
        downloaded.hashes.sha256,
        hash_bytes(HashFormat::Sha256, &bytes)
    );
    assert_eq!(downloaded.hashes.murmur2, curseforge_fingerprint(&bytes));

    let identified = client
        .version_files(
            std::slice::from_ref(&downloaded.hashes.sha1),
            HashAlgorithm::Sha1,
            None,
        )
        .await
        .unwrap();
    let found = &identified[&downloaded.hashes.sha1];
    assert_eq!(found.project_id, SODIUM);
    assert_eq!(found.id, version.id);
    // Também pelo sha512.
    let by_sha512 = client
        .version_files(
            std::slice::from_ref(&downloaded.hashes.sha512),
            HashAlgorithm::Sha512,
            None,
        )
        .await
        .unwrap();
    assert_eq!(by_sha512[&downloaded.hashes.sha512].id, version.id);
}

/// CA-4 da P1-03 contra a API real: 200 hashes de versões reais da Fabric API, uma requisição.
#[tokio::test]
#[ignore = "rede"]
async fn rede_p1_03_ca4_atualizacoes_de_200_hashes_numa_requisicao() {
    let dir = tempfile::tempdir().unwrap();
    let client = client(dir.path());
    let versions = client
        .project_versions(
            FABRIC_API,
            &VersionFilter {
                loaders: vec!["fabric".into()],
                ..VersionFilter::default()
            },
            None,
        )
        .await
        .unwrap();
    let hashes: Vec<String> = versions
        .iter()
        .filter_map(|v| v.primary_file().map(|f| f.hashes.sha1.clone()))
        .take(200)
        .collect();
    assert_eq!(hashes.len(), 200, "a Fabric API tem menos de 200 versões?");
    let before = client.request_count();
    let updates = client
        .version_files_update(
            &hashes,
            HashAlgorithm::Sha1,
            &UpdateFilter {
                loaders: vec!["fabric".into()],
                game_versions: vec!["1.21.1".into()],
                version_types: Vec::new(),
            },
            None,
        )
        .await
        .unwrap();
    assert_eq!(client.request_count() - before, 1);
    assert!(!updates.is_empty());
    for version in updates.values() {
        assert_eq!(version.project_id, FABRIC_API);
        assert!(version.game_versions.contains(&"1.21.1".to_owned()));
    }
}

#[tokio::test]
#[ignore = "rede"]
async fn rede_projetos_versoes_e_listas() {
    let dir = tempfile::tempdir().unwrap();
    let client = client(dir.path());
    let project = client.project("sodium", None).await.unwrap();
    assert_eq!(project.id, SODIUM);
    assert!(project.environment.contains(&Environment::ClientOnly));
    let summary = client
        .cache()
        .unwrap()
        .project_summary(SODIUM)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(summary.game_versions, project.game_versions);

    let projects = client
        .projects(&[SODIUM.into(), FABRIC_API.into()], None)
        .await
        .unwrap();
    assert_eq!(projects.len(), 2);

    let version_id = project.versions.last().unwrap().clone();
    let version = client.version(&version_id, None).await.unwrap();
    assert_eq!(version.project_id, SODIUM);
    let batch = client.versions(&project.versions[..3], None).await.unwrap();
    assert_eq!(batch.len(), 3);

    let loaders = client.loaders(None).await.unwrap();
    for name in ["fabric", "forge", "neoforge", "quilt"] {
        assert!(loaders.iter().any(|l| l.name == name), "{name}");
    }
    let game_versions = client.game_versions(None).await.unwrap();
    assert!(game_versions.iter().any(|v| v.version == "1.7.10"));
    let categories = client.categories(None).await.unwrap();
    assert!(categories.iter().any(|c| c.name == "optimization"));
}
