//! Aceite da descoberta (P1-16; SPEC T08, CA-T08-10 e CA-T08-13): início com no máximo 3
//! requisições por fonte, categorias que filtram só a fonte que as tem e a pré-visualização
//! completa (galeria, notas sob demanda, dependências), contra um Modrinth simulado com
//! respostas reais gravadas e uma segunda fonte simulada que conta as chamadas (a CurseForge
//! entra no app com a P1-10).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use serde_json::{Value, json};
use warden_core::CancellationToken;
use warden_discovery::home::{HOME_ITEMS, home};
use warden_discovery::preview::{
    DetailSources, ModrinthDetails, gallery, version_dependencies, version_notes,
};
use warden_http::{HttpClient, HttpConfig};
use warden_modrinth::{MetadataCache, ModrinthClient};
use warden_packwiz::Loader;
use warden_project::add::AddSources;
use warden_project::add::DependencyKind;
use warden_project::add::modrinth::ModrinthAdd;
use warden_project::search::modrinth::ModrinthSearch;
use warden_project::search::{
    ActiveSource, CategoryFilter, InstalledKeys, PackTarget, ProjectKind, ProjectPreview,
    SearchRequest, SearchSort, SearchSource, SourceFailure, SourceFilter, SourceHit, SourceId,
    SourcePage, SourceQuery, SourceRef, SourceWarningReason, search_in_category,
};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const MODRINTH_FIXTURES: &str = "../warden-modrinth/tests/fixtures/http";
const PROJECT_FIXTURES: &str = "../warden-project/tests/fixtures/http";
const SODIUM: &str = "AANobbMI";

fn fixture(dir: &str, name: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(dir).join(name);
    serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap()
}

fn target() -> PackTarget {
    PackTarget {
        minecraft: "1.21.1".into(),
        game_versions: vec!["1.21.1".into()],
        loaders: vec![Loader::Fabric],
    }
}

struct Api {
    server: MockServer,
    client: ModrinthClient,
}

impl Api {
    async fn start() -> Self {
        let server = MockServer::start().await;
        let mut config = HttpConfig::for_version("1");
        config.max_retries = 0;
        let http = HttpClient::new(config).unwrap();
        let cache = MetadataCache::in_memory(None).unwrap();
        let client =
            ModrinthClient::with_base_url(http, &format!("{}/v2", server.uri()), Some(cache))
                .unwrap();
        Self { server, client }
    }

    async fn with_search() -> Self {
        let api = Self::start().await;
        Mock::given(method("GET"))
            .and(path("/v2/search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(fixture(
                MODRINTH_FIXTURES,
                "2026-10-04-search-sodium-fabric-1.21.1.json",
            )))
            .mount(&api.server)
            .await;
        api
    }

    async fn requests(&self, prefix: &str) -> Vec<wiremock::Request> {
        self.server
            .received_requests()
            .await
            .unwrap_or_default()
            .into_iter()
            .filter(|request| request.url.path().starts_with(prefix))
            .collect()
    }

    fn modrinth_source(&self) -> ActiveSource {
        ActiveSource::Ready(Arc::new(ModrinthSearch::new(self.client.clone())))
    }
}

/// Outra fonte (faz o papel da CurseForge): conta as chamadas e guarda a última consulta.
struct Counting {
    calls: AtomicUsize,
    last: std::sync::Mutex<Option<SourceQuery>>,
    fail: bool,
}

impl Counting {
    fn new(fail: bool) -> Arc<Self> {
        Arc::new(Self {
            calls: AtomicUsize::new(0),
            last: std::sync::Mutex::new(None),
            fail,
        })
    }
}

#[async_trait::async_trait]
impl SearchSource for Counting {
    fn id(&self) -> SourceId {
        SourceId::Curseforge
    }

    async fn preview(
        &self,
        _project_id: &str,
        _cancel: &CancellationToken,
    ) -> warden_project::Result<ProjectPreview> {
        unreachable!("o início não pede pré-visualização")
    }

    async fn search(
        &self,
        query: &SourceQuery,
        _cancel: &CancellationToken,
    ) -> Result<SourcePage, SourceFailure> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        *self.last.lock().unwrap() = Some(query.clone());
        if self.fail {
            return Err(SourceFailure::unavailable("simulado: erro 500"));
        }
        let hits = (0..12)
            .map(|index| SourceHit {
                reference: SourceRef {
                    source: SourceId::Curseforge,
                    project_id: format!("cf{index}"),
                    slug: format!("cf-mod-{index}"),
                    downloads: 1000.0 - f64::from(index),
                },
                title: format!("Mod da CurseForge {index}"),
                author: format!("autor{index}"),
                summary: "resumo".into(),
                icon_url: None,
                updated: format!("2026-10-{:02}T00:00:00Z", 20 - index),
                created: "2026-01-01T00:00:00Z".into(),
                compatible: true,
                manual_download: false,
            })
            .collect();
        Ok(SourcePage { hits, total: 12 })
    }
}

fn param(request: &wiremock::Request, name: &str) -> String {
    request
        .url
        .query_pairs()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.into_owned())
        .unwrap_or_default()
}

#[tokio::test]
async fn inicio_usa_no_maximo_tres_requisicoes_por_fonte() {
    let api = Api::with_search().await;
    let counting = Counting::new(false);
    let sources = [
        api.modrinth_source(),
        ActiveSource::Ready(counting.clone() as Arc<dyn SearchSource>),
    ];
    let page = home(
        &sources,
        &target(),
        &InstalledKeys::new(),
        ProjectKind::Mod,
        &CancellationToken::new(),
    )
    .await
    .unwrap();

    let searches = api.requests("/v2/search").await;
    assert!(searches.len() <= 3, "Modrinth recebeu {}", searches.len());
    assert!(counting.calls.load(Ordering::SeqCst) <= 3);
    // Nada além da busca: sem a lista de categorias, que é o mapeamento curado.
    assert_eq!(
        api.server.received_requests().await.unwrap().len(),
        searches.len()
    );

    let indexes: Vec<String> = searches.iter().map(|r| param(r, "index")).collect();
    assert!(indexes.contains(&"downloads".to_owned()) && indexes.contains(&"updated".to_owned()));
    let facets = param(&searches[0], "facets");
    assert!(facets.contains("versions:1.21.1") && facets.contains("categories:fabric"));
    assert!(param(&searches[0], "query").is_empty());

    assert!(!page.popular.is_empty() && !page.updated.is_empty());
    assert!(page.popular.len() <= HOME_ITEMS && page.updated.len() <= HOME_ITEMS);
    assert_eq!(page.sources, [SourceId::Modrinth, SourceId::Curseforge]);
    assert!(page.warnings.is_empty());
    // Os dois lados aparecem na lista combinada.
    assert!(
        page.popular
            .iter()
            .any(|item| item.sources[0].source == SourceId::Curseforge)
    );
}

#[tokio::test]
async fn inicio_deixa_de_fora_o_que_ja_esta_no_pack_e_cita_os_nomes() {
    let api = Api::with_search().await;
    let installed = InstalledKeys::from([format!("modrinth:{SODIUM}")]);
    let page = home(
        &[api.modrinth_source()],
        &target(),
        &installed,
        ProjectKind::Mod,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert!(
        page.popular
            .iter()
            .chain(&page.updated)
            .all(|item| !item.in_pack && item.key != format!("modrinth:{SODIUM}"))
    );
    assert_eq!(page.popular_in_pack, ["Sodium"]);
}

#[tokio::test]
async fn inicio_com_uma_fonte_fora_traz_a_outra_e_o_aviso() {
    let api = Api::with_search().await;
    let counting = Counting::new(true);
    let sources = [
        api.modrinth_source(),
        ActiveSource::Ready(counting.clone() as Arc<dyn SearchSource>),
    ];
    let page = home(
        &sources,
        &target(),
        &InstalledKeys::new(),
        ProjectKind::Mod,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert!(!page.popular.is_empty());
    assert_eq!(page.warnings.len(), 1);
    assert_eq!(page.warnings[0].source, SourceId::Curseforge);
    assert_eq!(page.warnings[0].reason, SourceWarningReason::Unavailable);
}

#[tokio::test]
async fn sem_chave_o_inicio_nao_consulta_a_curseforge() {
    let api = Api::with_search().await;
    let sources = [
        api.modrinth_source(),
        ActiveSource::Off(SourceId::Curseforge, SourceWarningReason::KeyMissing),
    ];
    let page = home(
        &sources,
        &target(),
        &InstalledKeys::new(),
        ProjectKind::Mod,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert_eq!(page.sources, [SourceId::Modrinth]);
    assert_eq!(page.warnings.len(), 1);
    assert_eq!(page.warnings[0].reason, SourceWarningReason::KeyMissing);
}

fn request() -> SearchRequest {
    SearchRequest {
        query: String::new(),
        kind: ProjectKind::Mod,
        sort: SearchSort::Relevance,
        source: SourceFilter::All,
        environment: None,
        include_incompatible: false,
        category: Some("tecnologia".into()),
        cursor: None,
    }
}

#[tokio::test]
async fn categoria_filtra_as_duas_fontes_pelas_categorias_mapeadas() {
    let api = Api::with_search().await;
    let counting = Counting::new(false);
    let sources = [
        api.modrinth_source(),
        ActiveSource::Ready(counting.clone() as Arc<dyn SearchSource>),
    ];
    let filter = warden_discovery::categories::resolve("tecnologia", ProjectKind::Mod).unwrap();
    search_in_category(
        &sources,
        &target(),
        &InstalledKeys::new(),
        &request(),
        Some(&filter),
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    let searches = api.requests("/v2/search").await;
    assert_eq!(searches.len(), 1);
    assert!(param(&searches[0], "facets").contains(r#"["categories:technology"]"#));
    let seen = counting.last.lock().unwrap().clone().unwrap();
    assert_eq!(seen.category.unwrap().curseforge, [412]);
}

#[tokio::test]
async fn categoria_sem_par_filtra_so_a_fonte_que_a_tem() {
    let api = Api::with_search().await;
    let counting = Counting::new(false);
    let sources = [
        api.modrinth_source(),
        ActiveSource::Ready(counting.clone() as Arc<dyn SearchSource>),
    ];
    // "Desempenho" só existe no Modrinth.
    let filter = warden_discovery::categories::resolve("desempenho", ProjectKind::Mod).unwrap();
    let page = search_in_category(
        &sources,
        &target(),
        &InstalledKeys::new(),
        &request(),
        Some(&filter),
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert_eq!(counting.calls.load(Ordering::SeqCst), 0);
    assert_eq!(page.sources, [SourceId::Modrinth]);
    assert!(page.warnings.is_empty());
    assert_eq!(CategoryFilter::default().curseforge.len(), 0);
}

// ---------- pré-visualização completa ----------

async fn preview_api() -> Api {
    let api = Api::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/v2/project/{SODIUM}")))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(fixture(MODRINTH_FIXTURES, "2026-10-04-project-sodium.json")),
        )
        .mount(&api.server)
        .await;
    api
}

#[tokio::test]
async fn galeria_vem_do_projeto_com_miniatura_e_imagem_inteira() {
    let api = preview_api().await;
    let sources = DetailSources::new().with(Arc::new(ModrinthDetails::new(api.client.clone())));
    let items = gallery(
        &sources,
        SourceId::Modrinth,
        SODIUM,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert_eq!(items.len(), 6);
    assert!(items[0].thumb_url.contains("_350.webp"));
    assert!(
        !items[0].url.contains("_350") && items[0].url.starts_with("https://cdn.modrinth.com/")
    );
    assert!(items.iter().any(|item| item.title.is_some()));
    // Pedir de novo (a pré-visualização já trouxe o projeto) não faz outra requisição: cache.
    gallery(
        &sources,
        SourceId::Modrinth,
        SODIUM,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert_eq!(api.requests("/v2/project/").await.len(), 1);
}

#[tokio::test]
async fn fonte_sem_detalhes_ligados_da_erro_de_fonte_indisponivel() {
    let error = gallery(
        &DetailSources::new(),
        SourceId::Curseforge,
        "1",
        &CancellationToken::new(),
    )
    .await
    .unwrap_err();
    assert_eq!(
        error.code,
        warden_project::ProjectErrorCode::SearchSourceUnavailable
    );
}

#[tokio::test]
async fn notas_so_sao_pedidas_ao_abrir_a_linha() {
    let api = Api::start().await;
    let mut version = fixture(MODRINTH_FIXTURES, "2026-10-04-version-SMxNOGZ6.json");
    version["changelog"] = json!("- Corrige a mochila sumindo");
    let id = version["id"].as_str().unwrap().to_owned();
    Mock::given(method("GET"))
        .and(path(format!("/v2/version/{id}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(&version))
        .mount(&api.server)
        .await;
    let sources = DetailSources::new().with(Arc::new(ModrinthDetails::new(api.client.clone())));
    // Montar a lista de versões não pede nota nenhuma.
    assert!(api.requests("/v2/version/").await.is_empty());
    let notes = version_notes(
        &sources,
        SourceId::Modrinth,
        SODIUM,
        &id,
        &CancellationToken::new(),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(notes.body, "- Corrige a mochila sumindo");
    assert_eq!(api.requests("/v2/version/").await.len(), 1);
}

#[tokio::test]
async fn dependencias_marcam_ja_no_pack_e_o_que_sera_adicionado() {
    let api = Api::start().await;
    // O ModMenu exige a Fabric API e a Placeholder API; a Embeddium declara incompatíveis.
    let modmenu = fixture(
        PROJECT_FIXTURES,
        "2026-10-07-versions-modmenu-fabric-1.21.1.json",
    );
    let version = modmenu[0].clone();
    let version_id = version["id"].as_str().unwrap().to_owned();
    Mock::given(method("GET"))
        .and(path("/v2/versions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([version])))
        .mount(&api.server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v2/projects"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(fixture(PROJECT_FIXTURES, "2026-10-07-projects-add.json")),
        )
        .mount(&api.server)
        .await;
    let sources = AddSources::new().with(Arc::new(ModrinthAdd::new(api.client.clone())));
    let installed = InstalledKeys::from(["modrinth:P7dR8mSH".to_owned()]);
    let deps = version_dependencies(
        &sources,
        &installed,
        SourceId::Modrinth,
        &version_id,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert_eq!(deps.items.len(), 2);
    assert!(
        deps.items
            .iter()
            .all(|dep| dep.kind == DependencyKind::Required)
    );
    let fabric = deps
        .items
        .iter()
        .find(|dep| dep.project_id == "P7dR8mSH")
        .unwrap();
    let placeholder = deps
        .items
        .iter()
        .find(|dep| dep.project_id == "eXts2L7r")
        .unwrap();
    assert!(fabric.in_pack, "Fabric API já está no pack");
    assert!(!placeholder.in_pack, "Placeholder API será adicionada");
    assert_ne!(fabric.title, fabric.project_id, "o nome vem do projeto");
}
