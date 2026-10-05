//! Catálogo contra um servidor simulado que devolve respostas reais gravadas em 05/10/2026
//! (`tests/fixtures/http/`, origem em `FIXTURES.md`).

// Nos auxiliares dos testes, `unwrap`, `expect` e `panic!` são a forma idiomática de falhar
// (o `clippy.toml` só os libera dentro das funções `#[test]`).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use warden_catalog::{
    Catalog, CatalogCache, CatalogErrorCode, Endpoints, Error, Loader, LoaderVersions,
    MinecraftVersionKind, Refresh, Source, WallClock,
};
use warden_core::{CancellationToken, CoreErrorCode, DomainCode, DomainError};
use warden_http::{HttpClient, HttpConfig, ManualTimer};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

macro_rules! fixture {
    ($name:literal) => {
        include_bytes!(concat!("fixtures/http/2026-10-05-", $name))
    };
}

const MANIFEST: &str = "/mc/game/version_manifest_v2.json";
const FABRIC_GAME: &str = "/v2/versions/game";
const FABRIC_LOADER: &str = "/v2/versions/loader";
const FORGE_METADATA: &str = "/net/minecraftforge/forge/maven-metadata.xml";
const FORGE_PROMOTIONS: &str = "/net/minecraftforge/forge/promotions_slim.json";
const NEOFORGE: &str = "/api/maven/versions/releases/net/neoforged/neoforge";
const NEOFORGE_LEGACY: &str = "/api/maven/versions/releases/net/neoforged/forge";
const VERSION_1_7_10: &str = "/v1/packages/ed5d8789ed29872ea2ef1c348302b0c55e3f3468/1.7.10.json";

/// 2026-10-05 00:00 UTC, em milissegundos.
const NOW: u64 = 1_791_158_400_000;
const HOUR_MS: u64 = 60 * 60 * 1000;

fn body(bytes: &'static [u8]) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_bytes(bytes)
}

struct Setup {
    server: MockServer,
    catalog: Catalog,
    cache: CatalogCache,
    timer: ManualTimer,
    now: Arc<AtomicU64>,
}

impl Setup {
    /// Outro catálogo com o mesmo cache, apontando para um endereço sem servidor (sem rede).
    fn offline_catalog(&self) -> Catalog {
        let http =
            HttpClient::with_timer(HttpConfig::for_version("1"), Arc::new(self.timer.clone()))
                .unwrap();
        Catalog::with_endpoints(
            http,
            Endpoints::single("http://127.0.0.1:1").unwrap(),
            self.cache.clone(),
        )
    }

    fn advance_hours(&self, hours: u64) {
        self.now.fetch_add(hours * HOUR_MS, Ordering::SeqCst);
    }
}

async fn setup() -> Setup {
    let server = MockServer::start().await;
    let timer = ManualTimer::new();
    let http =
        HttpClient::with_timer(HttpConfig::for_version("1"), Arc::new(timer.clone())).unwrap();
    let now = Arc::new(AtomicU64::new(NOW));
    let reader = Arc::clone(&now);
    let clock: WallClock = Arc::new(move || reader.load(Ordering::SeqCst));
    let cache = CatalogCache::in_memory(Some(clock)).unwrap();
    let catalog = Catalog::with_endpoints(
        http,
        Endpoints::single(&server.uri()).unwrap(),
        cache.clone(),
    );
    Setup {
        server,
        catalog,
        cache,
        timer,
        now,
    }
}

/// Servidor com todas as respostas gravadas.
async fn setup_with_fixtures() -> Setup {
    let s = setup().await;
    for (route, bytes) in [
        (MANIFEST, &fixture!("mojang-version_manifest_v2.json")[..]),
        (FABRIC_GAME, &fixture!("fabric-versions-game.json")[..]),
        (FABRIC_LOADER, &fixture!("fabric-versions-loader.json")[..]),
        (FORGE_METADATA, &fixture!("forge-maven-metadata.xml")[..]),
        (
            FORGE_PROMOTIONS,
            &fixture!("forge-promotions_slim.json")[..],
        ),
        (NEOFORGE, &fixture!("neoforge-versions-neoforge.json")[..]),
        (
            NEOFORGE_LEGACY,
            &fixture!("neoforge-versions-forge.json")[..],
        ),
        (VERSION_1_7_10, &fixture!("mojang-1.7.10.json")[..]),
    ] {
        Mock::given(method("GET"))
            .and(path(route))
            .respond_with(body(bytes))
            .mount(&s.server)
            .await;
    }
    s
}

async fn loader(s: &Setup, loader: Loader, minecraft: &str) -> LoaderVersions {
    s.catalog
        .loader_versions(loader, minecraft, Refresh::IfStale, None)
        .await
        .unwrap()
}

/// Critério 1 da P1-05: `26.3` aparece antes de `1.21.11`, que aparece antes de `1.21.2`
/// (ordem do manifesto, nunca semver).
#[tokio::test]
async fn p1_05_ca1_ordem_oficial_do_manifesto() {
    let s = setup_with_fixtures().await;
    let list = s
        .catalog
        .minecraft_versions(Refresh::IfStale, None)
        .await
        .unwrap();
    let releases: Vec<&str> = list.releases().map(|v| v.id.as_str()).collect();
    let at = |id: &str| releases.iter().position(|r| *r == id).unwrap();
    assert!(at("26.3") < at("1.21.11"));
    assert!(at("1.21.11") < at("1.21.2"));
    assert_eq!(releases[0], "26.3");
    assert_eq!(list.latest_release.as_deref(), Some("26.3"));
    assert_eq!(list.latest_snapshot.as_deref(), Some("26.4-snapshot-2"));
    assert_eq!(
        list.compare("26.3", "1.21.11"),
        Some(std::cmp::Ordering::Greater)
    );
    // A lista inteira é a do manifesto, na mesma ordem (917 versões em 05/10/2026).
    assert_eq!(list.versions.len(), 917);
    assert_eq!(list.versions[0].id, "26.4-snapshot-2");
    assert_eq!(list.versions[0].kind, MinecraftVersionKind::Snapshot);
    // "Melhor esforço": tudo o que vem depois da 1.7.10 no manifesto.
    let v1_7_10 = list.position("1.7.10").unwrap();
    assert!(!list.get("1.7.10").unwrap().best_effort);
    assert!(list.versions[..=v1_7_10].iter().all(|v| !v.best_effort));
    assert!(list.versions[v1_7_10 + 1..].iter().all(|v| v.best_effort));
    assert!(list.get("1.7.2").unwrap().best_effort);
    assert!(list.get("b1.7.3").unwrap().best_effort);
    assert_eq!(list.freshness.fetched_at_ms, NOW);
    assert!(!list.freshness.offline);
}

/// Critério 2 da P1-05: Forge 1.7.10 lista `10.13.4.1614` (sem o sufixo `-1.7.10`) e marca a
/// recomendada.
#[tokio::test]
async fn p1_05_ca2_forge_1_7_10_sem_sufixo_e_recomendada() {
    let s = setup_with_fixtures().await;
    let list = loader(&s, Loader::Forge, "1.7.10").await;
    assert!(list.available());
    let v1614 = list.get("10.13.4.1614").unwrap();
    assert!(v1614.recommended);
    assert_eq!(
        v1614.maven,
        "net.minecraftforge:forge:1.7.10-10.13.4.1614-1.7.10"
    );
    assert_eq!(list.preselected.as_deref(), Some("10.13.4.1614"));
    assert_eq!(list.versions[0].version, "10.13.4.1614");
    assert!(list.versions.iter().all(|v| !v.version.contains('-')));
    assert_eq!(list.versions.iter().filter(|v| v.recommended).count(), 1);
}

/// Critério 2 da P1-05 e CA-T03-03: NeoForge não existe para 1.19.2; Fabric não existe para
/// 1.12.2.
#[tokio::test]
async fn ca_t03_03_neoforge_sem_1_19_2_e_fabric_sem_1_12_2() {
    let s = setup_with_fixtures().await;
    let neo = loader(&s, Loader::NeoForge, "1.19.2").await;
    assert!(!neo.available());
    assert!(neo.preselected.is_none());
    let fabric = loader(&s, Loader::Fabric, "1.12.2").await;
    assert!(!fabric.available());
    // E existem onde devem.
    assert!(loader(&s, Loader::NeoForge, "1.20.1").await.available());
    assert!(loader(&s, Loader::Fabric, "1.14").await.available());
    assert!(loader(&s, Loader::Forge, "1.12.2").await.available());
}

#[tokio::test]
async fn forge_1_12_2_sem_a_quebrada_e_com_a_recomendada() {
    let s = setup_with_fixtures().await;
    let list = loader(&s, Loader::Forge, "1.12.2").await;
    assert!(list.get("14.23.5.2851").is_none(), "denylist");
    assert!(list.get("14.23.5.2847").is_some());
    assert_eq!(list.preselected.as_deref(), Some("14.23.5.2859"));
    assert!(list.get("14.23.5.2859").unwrap().recommended);
    // Ordem numérica: 2864 (a mais nova) antes de 2860, que o Maven lista primeiro.
    assert_eq!(list.versions[0].version, "14.23.5.2864");
    // Builds de ramos de teste (`-4627`) ficam de fora.
    assert!(list.versions.iter().all(|v| v.version != "14.23.4.2720"));
}

#[tokio::test]
async fn forge_versoes_por_ano_e_sem_recomendada() {
    let s = setup_with_fixtures().await;
    let list = loader(&s, Loader::Forge, "26.3").await;
    assert!(list.available());
    // 26.3 só tem "latest" nas promoções: ela é a pré-selecionada.
    assert_eq!(list.preselected.as_deref(), Some("66.0.9"));
    assert!(list.versions.iter().all(|v| !v.recommended));
    assert_eq!(
        loader(&s, Loader::Forge, "1.20.1")
            .await
            .preselected
            .as_deref(),
        Some("47.4.10")
    );
}

/// Toda promoção do Forge para uma versão final do Minecraft aponta para uma versão da lista
/// (confere o tratamento dos sufixos contra os dados reais).
#[tokio::test]
async fn toda_promocao_do_forge_esta_na_lista() {
    let s = setup_with_fixtures().await;
    let promotions: serde_json::Value =
        serde_json::from_slice(fixture!("forge-promotions_slim.json")).unwrap();
    let minecraft = s
        .catalog
        .minecraft_versions(Refresh::IfStale, None)
        .await
        .unwrap();
    let mut checked = 0;
    for (key, version) in promotions["promos"].as_object().unwrap() {
        let (mc, _) = key.rsplit_once('-').unwrap();
        if minecraft.get(mc).map(|v| v.kind) != Some(MinecraftVersionKind::Release) {
            continue;
        }
        let list = loader(&s, Loader::Forge, mc).await;
        assert!(
            list.get(version.as_str().unwrap()).is_some(),
            "{key} = {version} não está na lista de {mc}"
        );
        checked += 1;
    }
    assert!(checked > 100, "{checked}");
}

#[tokio::test]
async fn neoforge_1_20_1_vem_de_net_neoforged_forge() {
    let s = setup_with_fixtures().await;
    let list = loader(&s, Loader::NeoForge, "1.20.1").await;
    assert_eq!(list.versions[0].version, "47.1.106");
    assert_eq!(
        list.versions[0].maven,
        "net.neoforged:forge:1.20.1-47.1.106"
    );
    assert_eq!(list.preselected.as_deref(), Some("47.1.106"));
    // O `47.1.82` sem prefixo não tem instalador: fica de fora.
    assert!(list.get("47.1.82").is_none());
    assert_eq!(list.versions.len(), 62);
}

/// O filtro com ponto final: `21.1.x` é a 1.21.1 e `21.11.x` é a 1.21.11.
#[tokio::test]
async fn neoforge_1_21_1_e_1_21_11_nao_se_misturam() {
    let s = setup_with_fixtures().await;
    let v1_21_1 = loader(&s, Loader::NeoForge, "1.21.1").await;
    assert!(
        v1_21_1
            .versions
            .iter()
            .all(|v| v.version.starts_with("21.1."))
    );
    assert!(v1_21_1.get("21.1.252").is_some());
    let v1_21_11 = loader(&s, Loader::NeoForge, "1.21.11").await;
    assert!(
        v1_21_11
            .versions
            .iter()
            .all(|v| v.version.starts_with("21.11."))
    );
    assert!(v1_21_11.available());
    let v1_21 = loader(&s, Loader::NeoForge, "1.21").await;
    assert!(
        v1_21
            .versions
            .iter()
            .all(|v| v.version.starts_with("21.0."))
    );
    // A pré-selecionada é estável.
    let preselected = v1_21_1.preselected.as_deref().unwrap();
    assert!(v1_21_1.get(preselected).unwrap().stable);
    assert_eq!(preselected, v1_21_1.versions[0].version);
}

#[tokio::test]
async fn neoforge_por_ano_so_beta_preseleciona_a_mais_nova() {
    let s = setup_with_fixtures().await;
    let list = loader(&s, Loader::NeoForge, "26.3").await;
    assert_eq!(list.versions[0].version, "26.3.0.48-beta");
    assert!(!list.versions[0].stable);
    assert_eq!(list.preselected.as_deref(), Some("26.3.0.48-beta"));
    let v26_1_2 = loader(&s, Loader::NeoForge, "26.1.2").await;
    assert!(
        v26_1_2
            .versions
            .iter()
            .all(|v| v.version.starts_with("26.1.2."))
    );
    // Snapshots (`+snapshot-1`) e brincadeiras não aparecem em versão nenhuma.
    let v26_1 = loader(&s, Loader::NeoForge, "26.1").await;
    assert!(v26_1.versions.iter().all(|v| !v.version.contains('+')));
}

#[tokio::test]
async fn fabric_lista_o_loader_e_preseleciona_o_estavel() {
    let s = setup_with_fixtures().await;
    let list = loader(&s, Loader::Fabric, "1.21.1").await;
    assert_eq!(list.versions[0].version, "0.19.5");
    assert!(list.versions[0].stable);
    assert_eq!(list.preselected.as_deref(), Some("0.19.5"));
    assert_eq!(list.versions[0].maven, "net.fabricmc:fabric-loader:0.19.5");
    assert!(loader(&s, Loader::Fabric, "26.3").await.available());
}

/// Critério 3 da P1-05: sem rede, usa o cache e informa a data.
#[tokio::test]
async fn p1_05_ca3_sem_rede_usa_o_cache_e_informa_a_data() {
    let s = setup_with_fixtures().await;
    s.catalog
        .minecraft_versions(Refresh::IfStale, None)
        .await
        .unwrap();
    for (l, mc) in [
        (Loader::Forge, "1.7.10"),
        (Loader::NeoForge, "1.21.1"),
        (Loader::Fabric, "1.21.1"),
    ] {
        loader(&s, l, mc).await;
    }
    // Um dia depois, sem rede.
    s.advance_hours(24);
    let offline = s.offline_catalog();
    let list = offline
        .minecraft_versions(Refresh::IfStale, None)
        .await
        .unwrap();
    assert!(list.freshness.offline);
    assert_eq!(list.freshness.fetched_at_ms, NOW);
    assert!(list.get("26.3").is_some());
    for (l, mc) in [
        (Loader::Forge, "1.7.10"),
        (Loader::NeoForge, "1.21.1"),
        (Loader::Fabric, "1.21.1"),
    ] {
        let list = offline
            .loader_versions(l, mc, Refresh::Always, None)
            .await
            .unwrap();
        assert!(list.available(), "{l:?}");
        assert!(list.freshness.offline, "{l:?}");
        assert_eq!(list.freshness.fetched_at_ms, NOW, "{l:?}");
    }
    assert!(offline.request_count() >= 4);
}

/// Sem rede e sem cache: o erro comum `NETWORK_UNAVAILABLE`, que vale tentar de novo.
#[tokio::test]
async fn sem_rede_e_sem_cache_e_erro_de_rede() {
    let s = setup().await;
    let error = s
        .offline_catalog()
        .minecraft_versions(Refresh::IfStale, None)
        .await
        .unwrap_err();
    assert_eq!(
        error.code(),
        DomainCode::Core(CoreErrorCode::NetworkUnavailable)
    );
    assert!(error.retryable());
    assert_eq!(error.params()["source"], "Mojang");
}

#[tokio::test]
async fn cache_valido_nao_pergunta_a_fonte_e_vencido_pergunta() {
    let s = setup_with_fixtures().await;
    let first = s
        .catalog
        .minecraft_versions(Refresh::IfStale, None)
        .await
        .unwrap();
    assert_eq!(s.catalog.request_count(), 1);
    s.advance_hours(5);
    let second = s
        .catalog
        .minecraft_versions(Refresh::IfStale, None)
        .await
        .unwrap();
    assert_eq!(s.catalog.request_count(), 1);
    assert_eq!(second.freshness, first.freshness);
    // "Tentar de novo" pede mesmo com o cache válido.
    s.catalog
        .minecraft_versions(Refresh::Always, None)
        .await
        .unwrap();
    assert_eq!(s.catalog.request_count(), 2);
    // Vencido (6 h depois da última busca): pede de novo e a data muda.
    s.advance_hours(6);
    let third = s
        .catalog
        .minecraft_versions(Refresh::IfStale, None)
        .await
        .unwrap();
    assert_eq!(s.catalog.request_count(), 3);
    assert_eq!(third.freshness.fetched_at_ms, NOW + 11 * HOUR_MS);
}

#[tokio::test]
async fn fonte_fora_do_ar_usa_o_cache_vencido() {
    let s = setup().await;
    Mock::given(path(MANIFEST))
        .respond_with(body(fixture!("mojang-version_manifest_v2.json")))
        .up_to_n_times(1)
        .mount(&s.server)
        .await;
    Mock::given(path(MANIFEST))
        .respond_with(ResponseTemplate::new(503))
        .mount(&s.server)
        .await;
    s.catalog
        .minecraft_versions(Refresh::IfStale, None)
        .await
        .unwrap();
    s.advance_hours(7);
    let list = s
        .catalog
        .minecraft_versions(Refresh::IfStale, None)
        .await
        .unwrap();
    assert!(list.freshness.offline);
    assert_eq!(list.freshness.fetched_at_ms, NOW);
    // Tentou de novo (3 vezes) antes de cair no cache.
    assert_eq!(s.timer.sleeps().len(), 3);
}

#[tokio::test]
async fn fonte_fora_do_ar_sem_cache_e_erro_da_fonte() {
    let s = setup().await;
    Mock::given(path(FABRIC_GAME))
        .respond_with(ResponseTemplate::new(503))
        .mount(&s.server)
        .await;
    Mock::given(path(FABRIC_LOADER))
        .respond_with(body(fixture!("fabric-versions-loader.json")))
        .mount(&s.server)
        .await;
    let error = s
        .catalog
        .loader_versions(Loader::Fabric, "1.21.1", Refresh::IfStale, None)
        .await
        .unwrap_err();
    assert_eq!(
        error.code(),
        DomainCode::Domain(CatalogErrorCode::SourceUnavailable)
    );
    assert_eq!(error.params()["source"], "Fabric");
}

#[tokio::test]
async fn resposta_estragada_nao_substitui_o_cache() {
    let s = setup().await;
    Mock::given(path(NEOFORGE))
        .respond_with(body(fixture!("neoforge-versions-neoforge.json")))
        .up_to_n_times(1)
        .mount(&s.server)
        .await;
    Mock::given(path(NEOFORGE))
        .respond_with(ResponseTemplate::new(200).set_body_string("<html>manutenção</html>"))
        .mount(&s.server)
        .await;
    loader(&s, Loader::NeoForge, "1.21.1").await;
    let list = s
        .catalog
        .loader_versions(Loader::NeoForge, "1.21.1", Refresh::Always, None)
        .await
        .unwrap();
    assert!(list.freshness.offline);
    assert!(list.available());
    // E continua lá: a resposta estragada não foi guardada.
    s.advance_hours(1);
    let list = s
        .catalog
        .loader_versions(Loader::NeoForge, "1.21.1", Refresh::IfStale, None)
        .await
        .unwrap();
    assert!(list.available());
}

#[tokio::test]
async fn resposta_estragada_sem_cache_e_erro() {
    let s = setup().await;
    Mock::given(path(FORGE_METADATA))
        .respond_with(ResponseTemplate::new(200).set_body_string("<metadata>"))
        .mount(&s.server)
        .await;
    Mock::given(path(FORGE_PROMOTIONS))
        .respond_with(body(fixture!("forge-promotions_slim.json")))
        .mount(&s.server)
        .await;
    let error = s
        .catalog
        .loader_versions(Loader::Forge, "1.20.1", Refresh::IfStale, None)
        .await
        .unwrap_err();
    assert!(
        matches!(
            &error,
            Error::InvalidResponse {
                origin: Source::Forge,
                ..
            }
        ),
        "{error}"
    );
    assert_eq!(
        error.code(),
        DomainCode::Domain(CatalogErrorCode::InvalidResponse)
    );
}

#[tokio::test]
async fn sem_promocoes_a_lista_do_forge_continua() {
    let s = setup().await;
    Mock::given(path(FORGE_METADATA))
        .respond_with(body(fixture!("forge-maven-metadata.xml")))
        .mount(&s.server)
        .await;
    Mock::given(path(FORGE_PROMOTIONS))
        .respond_with(ResponseTemplate::new(404))
        .mount(&s.server)
        .await;
    let list = loader(&s, Loader::Forge, "1.12.2").await;
    assert!(list.available());
    assert!(list.versions.iter().all(|v| !v.recommended));
    assert_eq!(list.preselected.as_deref(), Some("14.23.5.2864"));
    assert!(list.freshness.offline);
}

#[tokio::test]
async fn cancelamento_nao_cai_no_cache() {
    let s = setup_with_fixtures().await;
    s.catalog
        .minecraft_versions(Refresh::IfStale, None)
        .await
        .unwrap();
    let cancel = CancellationToken::new();
    cancel.cancel();
    let error = s
        .catalog
        .minecraft_versions(Refresh::Always, Some(&cancel))
        .await
        .unwrap_err();
    assert!(error.is_cancelled());
    assert_eq!(error.code(), DomainCode::Core(CoreErrorCode::Cancelled));
    let error = s
        .catalog
        .loader_versions(Loader::Forge, "1.7.10", Refresh::Always, Some(&cancel))
        .await
        .unwrap_err();
    assert!(error.is_cancelled());
}

#[tokio::test]
async fn json_da_versao_conferido_e_guardado_pelo_sha1() {
    let s = setup_with_fixtures().await;
    let json = s.catalog.version_json("1.7.10", None).await.unwrap();
    assert_eq!(json.sha1, "ed5d8789ed29872ea2ef1c348302b0c55e3f3468");
    assert_eq!(json.body, fixture!("mojang-1.7.10.json"));
    assert_eq!(json.json().unwrap()["id"], "1.7.10");
    let requests = s.catalog.request_count();
    // Do cache, mesmo sem rede e com o manifesto vencido.
    s.advance_hours(48);
    let offline = s.offline_catalog();
    let again = offline.version_json("1.7.10", None).await.unwrap();
    assert_eq!(again, json);
    assert_eq!(s.catalog.request_count(), requests);
}

#[tokio::test]
async fn json_da_versao_com_outro_conteudo_e_recusado() {
    let s = setup().await;
    Mock::given(path(MANIFEST))
        .respond_with(body(fixture!("mojang-version_manifest_v2.json")))
        .mount(&s.server)
        .await;
    Mock::given(path(VERSION_1_7_10))
        .respond_with(ResponseTemplate::new(200).set_body_string("{\"id\":\"1.7.10\"}"))
        .mount(&s.server)
        .await;
    let error = s.catalog.version_json("1.7.10", None).await.unwrap_err();
    assert!(
        matches!(&error, Error::VersionJsonCorrupted { .. }),
        "{error}"
    );
    assert_eq!(error.params()["id"], "1.7.10");
    // Nada foi guardado.
    assert!(
        s.cache
            .version_json("ed5d8789ed29872ea2ef1c348302b0c55e3f3468")
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn versao_que_nao_existe_pede_o_manifesto_de_novo_antes_de_desistir() {
    let s = setup_with_fixtures().await;
    s.catalog
        .minecraft_versions(Refresh::IfStale, None)
        .await
        .unwrap();
    let error = s.catalog.version_json("99.9", None).await.unwrap_err();
    assert!(matches!(&error, Error::VersionNotFound { id } if id == "99.9"));
    assert_eq!(
        error.code(),
        DomainCode::Domain(CatalogErrorCode::VersionNotFound)
    );
    // Uma do começo, uma para conferir o manifesto mais novo.
    assert_eq!(s.catalog.request_count(), 2);
}

#[tokio::test]
async fn catalogo_com_enderecos_do_ambiente() {
    let http = HttpClient::new(HttpConfig::default()).unwrap();
    let cache = CatalogCache::in_memory(None).unwrap();
    let catalog = Catalog::new(http, cache).unwrap();
    let vars_set = warden_catalog::BASE_URL_ENVS
        .iter()
        .any(|(name, _)| std::env::var_os(name).is_some());
    if !vars_set {
        assert_eq!(*catalog.endpoints(), Endpoints::official().unwrap());
    }
    assert!(catalog.cache().path().is_none());
}
