//! Testes contra as fontes reais (Mojang, Fabric, Forge, NeoForge): `cargo xtask
//! test-network` (QUALITY §4.1). Marcados `#[ignore = "rede"]`, com nome `rede_*`. O cache fica
//! numa pasta temporária, no layout do app (`AppPaths::metadata_db_file`).

// Nos auxiliares dos testes, `unwrap`, `expect` e `panic!` são a forma idiomática de falhar
// (o `clippy.toml` só os libera dentro das funções `#[test]`).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use warden_catalog::{Catalog, CatalogCache, Endpoints, Loader, MinecraftVersionKind, Refresh};
use warden_core::AppPaths;
use warden_http::{HttpClient, HttpConfig};

fn catalog(dir: &std::path::Path) -> Catalog {
    let paths = AppPaths::from_dev_root(dir).unwrap();
    let cache = CatalogCache::open(&paths.metadata_db_file()).unwrap();
    let http = HttpClient::new(HttpConfig::default()).unwrap();
    Catalog::with_endpoints(http, Endpoints::official().unwrap(), cache)
}

/// Critério 1 da P1-05 com o manifesto de hoje: as versões por ano vêm antes das `1.x`, e a
/// ordem é a do manifesto, não a do texto.
#[tokio::test]
#[ignore = "rede"]
async fn rede_p1_05_ca1_ordem_do_manifesto_real() {
    let dir = tempfile::tempdir().unwrap();
    let list = catalog(dir.path())
        .minecraft_versions(Refresh::IfStale, None)
        .await
        .unwrap();
    assert!(list.versions.len() >= 917, "{}", list.versions.len());
    let releases: Vec<&str> = list.releases().map(|v| v.id.as_str()).collect();
    let at = |id: &str| releases.iter().position(|r| *r == id).unwrap();
    assert!(at("26.3") < at("1.21.11"));
    assert!(at("1.21.11") < at("1.21.2"));
    assert!(at("1.21.2") < at("1.7.10"));
    let latest = list.latest_release.clone().unwrap();
    assert_eq!(releases[0], latest);
    assert_eq!(
        list.get(&latest).unwrap().kind,
        MinecraftVersionKind::Release
    );
    assert!(list.get("1.6.4").unwrap().best_effort);
    assert!(!list.get("1.7.10").unwrap().best_effort);
    assert!(!list.freshness.offline);
}

/// Critério 2 da P1-05 contra as fontes reais.
#[tokio::test]
#[ignore = "rede"]
async fn rede_p1_05_ca2_loaders_reais() {
    let dir = tempfile::tempdir().unwrap();
    let catalog = catalog(dir.path());
    let get = |loader, mc: &'static str| {
        let catalog = catalog.clone();
        async move {
            catalog
                .loader_versions(loader, mc, Refresh::IfStale, None)
                .await
                .unwrap()
        }
    };
    let forge = get(Loader::Forge, "1.7.10").await;
    let v1614 = forge.get("10.13.4.1614").unwrap();
    assert!(v1614.recommended);
    assert_eq!(
        v1614.maven,
        "net.minecraftforge:forge:1.7.10-10.13.4.1614-1.7.10"
    );
    assert_eq!(forge.preselected.as_deref(), Some("10.13.4.1614"));

    let forge = get(Loader::Forge, "1.12.2").await;
    assert!(forge.get("14.23.5.2851").is_none());
    assert!(forge.versions.iter().any(|v| v.recommended));

    assert!(!get(Loader::NeoForge, "1.19.2").await.available());
    assert!(!get(Loader::Fabric, "1.12.2").await.available());

    let neo = get(Loader::NeoForge, "1.20.1").await;
    assert!(neo.get("47.1.106").is_some());
    let neo = get(Loader::NeoForge, "1.21.1").await;
    assert!(neo.versions.iter().all(|v| v.version.starts_with("21.1.")));
    assert!(neo.get("21.1.252").is_some());
    let fabric = get(Loader::Fabric, "1.21.1").await;
    assert!(fabric.versions.iter().any(|v| v.stable));
    assert!(fabric.get("0.19.5").is_some());
}

/// O JSON da 1.7.10 confere com o `sha1` do manifesto, e o critério 3 da P1-05 com o cache de
/// verdade em disco: um catálogo sem rede (endereço sem servidor) usa o que o primeiro
/// guardou.
#[tokio::test]
#[ignore = "rede"]
async fn rede_p1_05_ca3_json_da_versao_e_cache_em_disco_sem_rede() {
    let dir = tempfile::tempdir().unwrap();
    let online = catalog(dir.path());
    let json = online.version_json("1.7.10", None).await.unwrap();
    assert_eq!(json.json().unwrap()["id"], "1.7.10");
    let fetched = online
        .loader_versions(Loader::Forge, "1.20.1", Refresh::IfStale, None)
        .await
        .unwrap();
    drop(online);

    let paths = AppPaths::from_dev_root(dir.path()).unwrap();
    let cache = CatalogCache::open(&paths.metadata_db_file()).unwrap();
    let http = HttpClient::new(HttpConfig::default()).unwrap();
    let offline = Catalog::with_endpoints(
        http,
        Endpoints::single("http://127.0.0.1:1").unwrap(),
        cache,
    );
    let list = offline
        .loader_versions(Loader::Forge, "1.20.1", Refresh::Always, None)
        .await
        .unwrap();
    assert!(list.freshness.offline);
    assert_eq!(
        list.freshness.fetched_at_ms,
        fetched.freshness.fetched_at_ms
    );
    assert_eq!(list.versions, fetched.versions);
    assert_eq!(offline.version_json("1.7.10", None).await.unwrap(), json);
}
