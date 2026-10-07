//! Aceite de "Adicionar" (P1-09; SPEC T08 e T09): busca, plano e gravação contra um Modrinth
//! simulado que devolve respostas reais gravadas (`tests/fixtures/http/FIXTURES.md` e as da
//! `warden-modrinth`), com o sidecar real do packwiz quando disponível. O pack resultante passa
//! na conformidade (`check_conformance`: um `refresh` não mudaria nada).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::Value;
use warden_core::CancellationToken;
use warden_http::{HttpClient, HttpConfig};
use warden_modrinth::{MetadataCache, ModrinthClient};
use warden_packwiz::{
    CurseForgeFile, HashFormat, Loader, Metafile, ModrinthFile, PackIndex, PackManifest, Side,
};
use warden_packwiz_cli::{Packwiz, RunContext, check_conformance};
use warden_project::ProjectErrorCode as Code;
use warden_project::add::modrinth::ModrinthAdd;
use warden_project::add::plan::{AddPlan, AddPlanRequest, DuplicateMatch, NodeRole, add_plan};
use warden_project::add::{
    AddApplyRequest, AddChoice, AddSources, ApplyItem, ChannelPolicy, add_apply,
};
use warden_project::search::modrinth::ModrinthSearch;
use warden_project::search::{
    ActiveSource, InstalledKeys, PackTarget, ProjectKind, SearchRequest, SearchSort, SourceFilter,
    SourceId, project_versions, search,
};
use warden_project::side::SideChoice;
use wiremock::matchers::{method, path, path_regex};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

const MODRINTH_FIXTURES: &str = "../warden-modrinth/tests/fixtures/http";
const FIXTURES: &str = "tests/fixtures/http";

const SODIUM: &str = "AANobbMI";
const FABRIC_API: &str = "P7dR8mSH";
const PLACEHOLDER: &str = "eXts2L7r";
const MODMENU: &str = "mOgUt4GM";
const APPLESKIN: &str = "EsAfCjCV";
const IRIS: &str = "YL57xq9U";
const EMBEDDIUM: &str = "sk9rgfiA";
const RUBIDIUM: &str = "4ZqxOvjD";

fn fixture(dir: &str, name: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(dir).join(name);
    serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap()
}

fn recorded(name: &str) -> Value {
    fixture(FIXTURES, &format!("2026-10-07-{name}.json"))
}

fn recorded_modrinth(name: &str) -> Value {
    fixture(MODRINTH_FIXTURES, &format!("2026-10-04-{name}.json"))
}

fn binary() -> Option<PathBuf> {
    let name = if cfg!(windows) {
        "packwiz-x86_64-pc-windows-msvc.exe"
    } else {
        "packwiz-x86_64-unknown-linux-gnu"
    };
    let path = std::env::var_os("WARDEN_PACKWIZ_BIN").map_or_else(
        || {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../apps/desktop/src-tauri/binaries")
                .join(name)
        },
        PathBuf::from,
    );
    if path.is_file() {
        Some(path)
    } else {
        assert_ne!(
            std::env::var_os("WARDEN_REQUIRE_EXTERNALS").as_deref(),
            Some(std::ffi::OsStr::new("1")),
            "sidecar packwiz ausente"
        );
        None
    }
}

fn packwiz(root: &Path) -> Option<Packwiz> {
    let binary = binary()?;
    Some(Packwiz::new(
        binary,
        root.join(".packwiz-cache"),
        root.join(".packwiz-config.toml"),
    ))
}

/// Responde `GET /projects?ids=` e `GET /versions?ids=` com os itens pedidos de um conjunto
/// de respostas gravadas (a API omite os que não existem).
struct ByIds(Vec<Value>);

impl Respond for ByIds {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let ids: Vec<String> = request
            .url
            .query_pairs()
            .find(|(key, _)| key == "ids")
            .map(|(_, value)| serde_json::from_str(&value).unwrap())
            .unwrap_or_default();
        let found: Vec<&Value> = self
            .0
            .iter()
            .filter(|item| {
                let id = item["id"].as_str().unwrap_or_default();
                let slug = item["slug"].as_str().unwrap_or_default();
                ids.iter().any(|wanted| wanted == id || wanted == slug)
            })
            .collect();
        ResponseTemplate::new(200).set_body_json(found)
    }
}

/// O Modrinth simulado, com as respostas reais gravadas.
struct Api {
    server: MockServer,
    client: ModrinthClient,
}

impl Api {
    async fn start() -> Self {
        Self::with_versions(Vec::new()).await
    }

    /// `extra`: versões a mais (simuladas, marcadas no teste) servidas em `/versions`.
    async fn with_versions(extra: Vec<Value>) -> Self {
        let server = MockServer::start().await;
        let mut projects: Vec<Value> = as_list(&recorded("projects-add"));
        projects.extend(as_list(&recorded_modrinth("projects-3")));
        let mut versions = Vec::new();
        let by_project = [
            (
                SODIUM,
                recorded_modrinth("project-sodium-versions-fabric-1.21.1"),
            ),
            (FABRIC_API, recorded("versions-fabric-api-fabric-1.21.1")),
            (
                PLACEHOLDER,
                recorded("versions-placeholder-api-fabric-1.21.1"),
            ),
            (MODMENU, recorded("versions-modmenu-fabric-1.21.1")),
            (APPLESKIN, recorded("versions-appleskin-fabric-1.21.1")),
            (IRIS, recorded("versions-iris-fabric-1.21.1")),
            (EMBEDDIUM, recorded_modrinth("project-embeddium-versions")),
        ];
        for (id, list) in &by_project {
            versions.extend(as_list(list));
            Mock::given(method("GET"))
                .and(path(format!("/v2/project/{id}/version")))
                .respond_with(ResponseTemplate::new(200).set_body_json(list))
                .mount(&server)
                .await;
        }
        versions.extend(as_list(&recorded("versions-sodium-neoforge-1.21.1")));
        versions.extend(as_list(&recorded("versions-ids-s7adptIg")));
        versions.extend(extra);
        Mock::given(method("GET"))
            .and(path("/v2/projects"))
            .respond_with(ByIds(projects))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/v2/versions"))
            .respond_with(ByIds(versions.clone()))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/v2/search"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(recorded_modrinth("search-sodium-fabric-1.21.1")),
            )
            .mount(&server)
            .await;
        // Projeto sem versão para o pack.
        Mock::given(method("GET"))
            .and(path_regex(r"^/v2/project/[^/]+/version$"))
            .respond_with(ResponseTemplate::new(200).set_body_json(Vec::<Value>::new()))
            .with_priority(10)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/v2/version_files"))
            .respond_with(HashLookup(versions))
            .mount(&server)
            .await;
        let mut config = HttpConfig::for_version("1");
        config.max_retries = 0;
        let http = HttpClient::new(config).unwrap();
        let cache = MetadataCache::in_memory(None).unwrap();
        let client =
            ModrinthClient::with_base_url(http, &format!("{}/v2", server.uri()), Some(cache))
                .unwrap();
        Self { server, client }
    }

    fn sources(&self) -> AddSources {
        AddSources::new().with(Arc::new(ModrinthAdd::new(self.client.clone())))
    }

    async fn requests(&self, path_prefix: &str) -> usize {
        self.server
            .received_requests()
            .await
            .unwrap_or_default()
            .iter()
            .filter(|request| request.url.path().starts_with(path_prefix))
            .count()
    }
}

/// `POST /version_files` com SHA-1: o hash de cada arquivo conhecido → a versão.
struct HashLookup(Vec<Value>);

impl Respond for HashLookup {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let body: Value = serde_json::from_slice(&request.body).unwrap();
        let hashes: HashSet<String> = body["hashes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|hash| hash.as_str().unwrap().to_owned())
            .collect();
        let mut found = serde_json::Map::new();
        for version in &self.0 {
            for file in version["files"].as_array().into_iter().flatten() {
                let sha1 = file["hashes"]["sha1"].as_str().unwrap_or_default();
                if hashes.contains(sha1) {
                    found.insert(sha1.to_owned(), version.clone());
                }
            }
        }
        ResponseTemplate::new(200).set_body_json(Value::Object(found))
    }
}

fn as_list(value: &Value) -> Vec<Value> {
    value.as_array().cloned().unwrap()
}

/// Uma versão gravada pelo ID, de qualquer resposta.
fn version_json(list: &Value, id: &str) -> Value {
    as_list(&list.clone())
        .into_iter()
        .find(|version| version["id"] == id)
        .unwrap()
}

fn write(root: &Path, path: &str, text: &str) {
    let full = root.join(path);
    fs::create_dir_all(full.parent().unwrap()).unwrap();
    fs::write(full, text).unwrap();
}

/// Pack vazio (índice vazio) com o loader dado, mais os metafiles dados.
fn pack(root: &Path, minecraft: &str, loader: Loader, files: &[(&str, Metafile)]) {
    let mut manifest = PackManifest::new("Teste", minecraft);
    manifest.set_loader_version(loader, "1.0.0");
    write(root, "pack.toml", &manifest.to_toml_string());
    let index = PackIndex {
        hash_format: "sha256".into(),
        files: files
            .iter()
            .map(|(path, _)| warden_packwiz::IndexEntry::new(path, "00"))
            .collect(),
    };
    write(root, "index.toml", &index.to_toml_string());
    for (path, metafile) in files {
        write(root, path, &metafile.to_toml_string());
    }
}

/// Deixa o pack como o packwiz o deixaria (hashes do índice), quando o sidecar existe.
async fn refresh(root: &Path) {
    if let Some(cli) = packwiz(root) {
        cli.refresh(root, false, RunContext::new(&CancellationToken::new()))
            .await
            .unwrap();
    }
}

fn modrinth_metafile(title: &str, project: &str, version: &str) -> Metafile {
    Metafile::modrinth(
        &ModrinthFile {
            title: title.into(),
            project_id: project.into(),
            version_id: version.into(),
            filename: format!("{project}.jar"),
            url: format!("https://cdn.modrinth.com/data/{project}/versions/{version}/a.jar"),
            hash_format: HashFormat::Sha512,
            hash: "ab".repeat(64),
        },
        Side::Both,
    )
}

fn choice(project: &str) -> AddChoice {
    AddChoice {
        source: SourceId::Modrinth,
        project_id: project.into(),
        version_id: None,
    }
}

async fn plan_of(api: &Api, root: &Path, projects: &[&str]) -> AddPlan {
    add_plan(
        root,
        &api.sources(),
        ChannelPolicy::default(),
        &AddPlanRequest {
            items: projects.iter().map(|p| choice(p)).collect(),
        },
        &CancellationToken::new(),
    )
    .await
    .unwrap()
}

fn node<'a>(plan: &'a AddPlan, project: &str) -> &'a warden_project::add::plan::PlanNode {
    plan.nodes
        .iter()
        .find(|node| node.project_id == project)
        .unwrap_or_else(|| panic!("{project} fora do plano: {plan:#?}"))
}

/// O que o usuário confirma por padrão: todos os nós escolhidos e obrigatórios.
fn confirm_default(plan: &AddPlan) -> AddApplyRequest {
    AddApplyRequest {
        items: plan
            .nodes
            .iter()
            .filter(|node| node.role != NodeRole::Optional)
            .map(|node| ApplyItem {
                source: node.source,
                project_id: node.project_id.clone(),
                version_id: node.version_id.clone(),
                side: None,
                replaces: None,
            })
            .collect(),
    }
}

async fn assert_conforming(root: &Path) {
    let Some(cli) = packwiz(root) else { return };
    let staging = tempfile::tempdir().unwrap();
    let report = check_conformance(
        &cli,
        root,
        &staging.path().join("copia"),
        RunContext::new(&CancellationToken::new()),
    )
    .await
    .unwrap();
    assert!(report.is_conforming(), "{:#?}", report.differences);
}

fn metafiles(root: &Path) -> Vec<String> {
    let mut found = Vec::new();
    for folder in ["mods", "resourcepacks", "shaderpacks"] {
        if let Ok(entries) = fs::read_dir(root.join(folder)) {
            for entry in entries {
                found.push(format!(
                    "{folder}/{}",
                    entry.unwrap().file_name().to_string_lossy()
                ));
            }
        }
    }
    found.sort();
    found
}

#[tokio::test]
async fn ca_t08_01_buscar_sodium_e_adicionar_grava_referencia_do_modrinth() {
    let api = Api::start().await;
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    pack(root, "1.21.1", Loader::Fabric, &[]);
    refresh(root).await;

    let target = PackTarget::read(root).unwrap();
    let sources = [ActiveSource::Ready(Arc::new(ModrinthSearch::new(
        api.client.clone(),
    )))];
    let page = search(
        &sources,
        &target,
        &InstalledKeys::new(),
        &SearchRequest {
            query: "sodium".into(),
            kind: ProjectKind::Mod,
            sort: SearchSort::Relevance,
            source: SourceFilter::All,
            environment: None,
            include_incompatible: false,
            cursor: None,
        },
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    let position = page
        .items
        .iter()
        .position(|item| item.key == format!("modrinth:{SODIUM}"))
        .unwrap();
    assert!(position < 3, "Sodium em {position}");
    let sodium = &page.items[position];
    assert_eq!(sodium.sources.len(), 1);
    assert!(sodium.compatible && !sodium.in_pack);
    // A busca pediu os filtros travados do pack.
    let requests = api.server.received_requests().await.unwrap();
    let query = requests
        .iter()
        .find(|request| request.url.path() == "/v2/search")
        .unwrap()
        .url
        .query_pairs()
        .find(|(key, _)| key == "facets")
        .unwrap()
        .1
        .into_owned();
    assert!(query.contains("versions:1.21.1") && query.contains("categories:fabric"));

    let versions = project_versions(
        &api.sources(),
        &target,
        ChannelPolicy::default(),
        SourceId::Modrinth,
        SODIUM,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    // A padrão é a estável mais nova, não a beta.
    assert_eq!(versions.default_id.as_deref(), Some("SMxNOGZ6"));
    assert!(versions.versions.len() > 1);

    let plan = plan_of(&api, root, &[SODIUM]).await;
    assert_eq!(plan.nodes.len(), 1);
    let sodium = node(&plan, SODIUM);
    assert_eq!(sodium.role, NodeRole::Chosen);
    assert_eq!(sodium.version_id, "SMxNOGZ6");
    assert_eq!(sodium.side, SideChoice::Client);
    assert!(sodium.compatible && !sodium.outside_channel);
    assert!(plan.conflicts.is_empty() && plan.duplicates.is_empty());

    let Some(cli) = packwiz(root) else { return };
    let result = add_apply(
        root,
        &api.sources(),
        &confirm_default(&plan),
        &cli,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert_eq!(result.added.len(), 1);
    assert_eq!(result.added[0].path, "mods/sodium.pw.toml");
    let text = fs::read_to_string(root.join("mods/sodium.pw.toml")).unwrap();
    assert!(text.contains("[update.modrinth]"), "{text}");
    assert!(text.contains("hash-format = \"sha512\""), "{text}");
    assert!(text.contains("side = \"client\""), "{text}");
    assert!(text.contains("mod-id = \"AANobbMI\""), "{text}");
    assert!(text.contains("version = \"SMxNOGZ6\""), "{text}");
    let index = fs::read_to_string(root.join("index.toml")).unwrap();
    assert!(index.contains("mods/sodium.pw.toml"), "{index}");
    assert_conforming(root).await;
}

#[tokio::test]
async fn ca_t09_01_dependencia_obrigatoria_entra_junto_com_um_refresh() {
    let api = Api::start().await;
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    pack(root, "1.21.1", Loader::Fabric, &[]);
    refresh(root).await;

    let plan = plan_of(&api, root, &[MODMENU]).await;
    let fabric_api = node(&plan, FABRIC_API);
    assert_eq!(fabric_api.role, NodeRole::Required);
    assert_eq!(fabric_api.required_by, [format!("modrinth:{MODMENU}")]);
    assert_eq!(node(&plan, PLACEHOLDER).role, NodeRole::Required);
    assert_eq!(node(&plan, MODMENU).side, SideChoice::Client);
    assert_eq!(plan.nodes.len(), 3);
    assert!(plan.missing.is_empty());

    let Some(cli) = packwiz(root) else { return };
    let result = add_apply(
        root,
        &api.sources(),
        &confirm_default(&plan),
        &cli,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert_eq!(result.added.len(), 3);
    assert_eq!(
        metafiles(root),
        [
            "mods/fabric-api.pw.toml",
            "mods/modmenu.pw.toml",
            "mods/placeholder-api.pw.toml"
        ]
    );
    let index = fs::read_to_string(root.join("index.toml")).unwrap();
    for path in metafiles(root) {
        assert!(index.contains(&path), "{path} fora do índice");
    }
    assert_conforming(root).await;

    // Depois de adicionar, a busca marca "Já no pack" e o plano não repete nada.
    let plan = plan_of(&api, root, &[MODMENU]).await;
    assert!(plan.nodes.is_empty());
    assert!(plan.installed.iter().any(|entry| entry.chosen));
}

#[tokio::test]
async fn ca_t09_05_biblioteca_comum_aparece_uma_vez() {
    let api = Api::start().await;
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    pack(root, "1.21.1", Loader::Fabric, &[]);

    let plan = plan_of(&api, root, &[MODMENU, APPLESKIN]).await;
    let libraries: Vec<_> = plan
        .nodes
        .iter()
        .filter(|node| node.project_id == FABRIC_API)
        .collect();
    assert_eq!(libraries.len(), 1);
    assert_eq!(
        libraries[0].required_by,
        [
            format!("modrinth:{MODMENU}"),
            format!("modrinth:{APPLESKIN}")
        ]
    );
    // Escolhidos primeiro, na ordem pedida.
    assert_eq!(plan.nodes[0].project_id, MODMENU);
    assert_eq!(plan.nodes[1].project_id, APPLESKIN);
    assert!(plan.conflicts.is_empty());
}

#[tokio::test]
async fn ca_t09_05_escolhidos_que_se_declaram_incompativeis_geram_aviso() {
    // Simulado: a resposta real do AppleSkin acrescida de `incompatible` com o Mod Menu
    // (nenhum par real de mods Fabric 1.21.1 gravado declara isso).
    let mut appleskin = recorded("versions-appleskin-fabric-1.21.1");
    for version in appleskin.as_array_mut().unwrap() {
        version["dependencies"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({
                "project_id": MODMENU, "version_id": null, "file_name": null,
                "dependency_type": "incompatible"
            }));
    }
    let api = Api::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/v2/project/{APPLESKIN}/version")))
        .respond_with(ResponseTemplate::new(200).set_body_json(appleskin))
        .with_priority(1)
        .mount(&api.server)
        .await;
    let temp = tempfile::tempdir().unwrap();
    pack(temp.path(), "1.21.1", Loader::Fabric, &[]);
    let plan = plan_of(&api, temp.path(), &[MODMENU, APPLESKIN]).await;
    assert_eq!(plan.conflicts.len(), 1);
    let conflict = &plan.conflicts[0];
    assert!(!conflict.with_pack);
    assert_eq!(conflict.declared_by, format!("modrinth:{APPLESKIN}"));
    let keys = [conflict.item.key.as_str(), conflict.other.key.as_str()];
    assert!(keys.contains(&"modrinth:mOgUt4GM") && keys.contains(&"modrinth:EsAfCjCV"));
}

#[tokio::test]
async fn ca_t09_02_incompatibilidade_declarada_com_item_do_pack() {
    // Dados reais: o Embeddium para NeoForge 1.21.1 declara `incompatible` com o Rubidium.
    let api = Api::start().await;
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    pack(
        root,
        "1.21.1",
        Loader::NeoForge,
        &[(
            "mods/rubidium.pw.toml",
            modrinth_metafile("Rubidium", RUBIDIUM, "rubidium-v"),
        )],
    );
    let plan = plan_of(&api, root, &[EMBEDDIUM]).await;
    assert_eq!(plan.conflicts.len(), 1, "{plan:#?}");
    let conflict = &plan.conflicts[0];
    assert!(conflict.with_pack);
    assert_eq!(conflict.item.key, format!("modrinth:{EMBEDDIUM}"));
    assert_eq!(conflict.other.title, "Rubidium");
    assert_eq!(
        conflict.other.path.as_deref(),
        Some("mods/rubidium.pw.toml")
    );
    assert_eq!(conflict.declared_by, format!("modrinth:{EMBEDDIUM}"));
}

#[tokio::test]
async fn ca_t09_02_pack_neoforge_com_sodium_e_embeddium_simulado() {
    // Simulado (ver FIXTURES.md): em 07/10/2026 nem o Sodium nem o Embeddium para NeoForge
    // 1.21.1 declaram um ao outro. A resposta real do Embeddium recebe `incompatible` com o
    // Sodium, e o pack tem o Sodium real para NeoForge (versão `uMOpc5uV`).
    let mut embeddium = recorded_modrinth("project-embeddium-versions");
    for version in embeddium.as_array_mut().unwrap() {
        version["dependencies"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({
                "project_id": SODIUM, "version_id": null, "file_name": null,
                "dependency_type": "incompatible"
            }));
    }
    let api = Api::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/v2/project/{EMBEDDIUM}/version")))
        .respond_with(ResponseTemplate::new(200).set_body_json(embeddium))
        .with_priority(1)
        .mount(&api.server)
        .await;
    let temp = tempfile::tempdir().unwrap();
    pack(
        temp.path(),
        "1.21.1",
        Loader::NeoForge,
        &[(
            "mods/sodium.pw.toml",
            modrinth_metafile("Sodium", SODIUM, "uMOpc5uV"),
        )],
    );
    let plan = plan_of(&api, temp.path(), &[EMBEDDIUM]).await;
    let conflict = plan
        .conflicts
        .iter()
        .find(|conflict| conflict.other.title == "Sodium")
        .unwrap_or_else(|| panic!("{plan:#?}"));
    assert!(conflict.with_pack);
}

#[tokio::test]
async fn incompatibilidade_declarada_pelo_item_do_pack_tambem_aparece() {
    // O pack tem o Embeddium (versão real que declara o Rubidium incompatível); adicionar o
    // Rubidium com uma versão escolhida (simulada para NeoForge 1.21.1) mostra o conflito.
    let embeddium = recorded_modrinth("project-embeddium-versions");
    let embeddium_version = as_list(&embeddium)[0]["id"].as_str().unwrap().to_owned();
    let mut rubidium = version_json(&recorded("versions-ids-s7adptIg"), "s7adptIg");
    rubidium["id"] = "rubidium-simulada".into();
    rubidium["project_id"] = RUBIDIUM.into();
    rubidium["loaders"] = serde_json::json!(["neoforge"]);
    let api = Api::with_versions(vec![rubidium]).await;
    let temp = tempfile::tempdir().unwrap();
    pack(
        temp.path(),
        "1.21.1",
        Loader::NeoForge,
        &[(
            "mods/embeddium.pw.toml",
            modrinth_metafile("Embeddium", EMBEDDIUM, &embeddium_version),
        )],
    );
    let plan = add_plan(
        temp.path(),
        &api.sources(),
        ChannelPolicy::default(),
        &AddPlanRequest {
            items: vec![AddChoice {
                source: SourceId::Modrinth,
                project_id: RUBIDIUM.into(),
                version_id: Some("rubidium-simulada".into()),
            }],
        },
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert_eq!(plan.conflicts.len(), 1, "{plan:#?}");
    let conflict = &plan.conflicts[0];
    assert_eq!(conflict.item.key, format!("modrinth:{RUBIDIUM}"));
    assert_eq!(conflict.declared_by, format!("modrinth:{EMBEDDIUM}"));
    assert_eq!(
        conflict.other.path.as_deref(),
        Some("mods/embeddium.pw.toml")
    );
}

#[tokio::test]
async fn ca_t09_04_mod_ja_no_pack_pela_curseforge_oferece_substituir() {
    let api = Api::start().await;
    let versions = recorded("versions-modmenu-fabric-1.21.1");
    let latest = &as_list(&versions)[0];
    let sha1 = latest["files"][0]["hashes"]["sha1"]
        .as_str()
        .unwrap()
        .to_owned();
    // O mesmo arquivo do Mod Menu, mas pela CurseForge (nome diferente no pack).
    let curseforge = Metafile::curseforge(
        &CurseForgeFile {
            name: "Menu de Mods (CF)".into(),
            project_id: 308_702,
            file_id: 5_000_000,
            filename: latest["files"][0]["filename"].as_str().unwrap().into(),
            hash_format: HashFormat::Sha1,
            hash: sha1,
        },
        Side::Client,
    );
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    pack(
        root,
        "1.21.1",
        Loader::Fabric,
        &[
            ("mods/menu-de-mods.pw.toml", curseforge),
            (
                "mods/fabric-api.pw.toml",
                modrinth_metafile("Fabric API", FABRIC_API, "Mys3P7lK"),
            ),
        ],
    );
    refresh(root).await;
    let plan = plan_of(&api, root, &[MODMENU]).await;
    assert_eq!(plan.duplicates.len(), 1, "{plan:#?}");
    let duplicate = &plan.duplicates[0];
    assert_eq!(duplicate.existing_path, "mods/menu-de-mods.pw.toml");
    assert_eq!(duplicate.existing_source, Some(SourceId::Curseforge));
    assert_eq!(duplicate.matched_by, DuplicateMatch::Project);
    // A Fabric API já está no pack: não entra de novo, aparece em "Já no pack".
    assert!(plan.nodes.iter().all(|node| node.project_id != FABRIC_API));
    let installed = plan
        .installed
        .iter()
        .find(|entry| entry.key == format!("modrinth:{FABRIC_API}"))
        .unwrap();
    assert_eq!(installed.required_by, [format!("modrinth:{MODMENU}")]);
    assert_eq!(installed.path, "mods/fabric-api.pw.toml");

    let Some(cli) = packwiz(root) else { return };
    let mut request = confirm_default(&plan);
    for item in &mut request.items {
        if item.project_id == MODMENU {
            item.replaces = Some(duplicate.existing_path.clone());
        }
    }
    let result = add_apply(
        root,
        &api.sources(),
        &request,
        &cli,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert_eq!(result.removed, ["mods/menu-de-mods.pw.toml"]);
    assert!(!root.join("mods/menu-de-mods.pw.toml").exists());
    assert!(root.join("mods/modmenu.pw.toml").exists());
    assert_conforming(root).await;
}

#[tokio::test]
async fn dependencia_com_versao_exata_e_canal() {
    let api = Api::start().await;
    let temp = tempfile::tempdir().unwrap();
    pack(temp.path(), "1.21.1", Loader::Fabric, &[]);
    // Estável: o Iris estável pede o Sodium sem versão (a mais nova estável).
    let plan = plan_of(&api, temp.path(), &[IRIS]).await;
    assert_eq!(
        node(&plan, IRIS).channel,
        warden_project::add::VersionChannel::Release
    );
    assert_eq!(node(&plan, SODIUM).version_id, "SMxNOGZ6");
    // Com beta liberada: o Iris beta pede a versão exata `s7adptIg` do Sodium.
    let plan = add_plan(
        temp.path(),
        &api.sources(),
        ChannelPolicy {
            allow_prerelease: true,
        },
        &AddPlanRequest {
            items: vec![choice(IRIS)],
        },
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert_eq!(
        node(&plan, IRIS).channel,
        warden_project::add::VersionChannel::Beta
    );
    assert_eq!(node(&plan, SODIUM).version_id, "s7adptIg");
    assert_eq!(
        node(&plan, SODIUM).required_by,
        [format!("modrinth:{IRIS}")]
    );
}

#[tokio::test]
async fn sem_versao_compativel_vai_para_faltando() {
    let api = Api::start().await;
    let temp = tempfile::tempdir().unwrap();
    // Pack Forge 1.20.1: o Mod Menu (só Fabric) não tem versão.
    pack(temp.path(), "1.20.1", Loader::Forge, &[]);
    let plan = plan_of(&api, temp.path(), &["inexistente-xyz", MODMENU]).await;
    assert!(plan.nodes.is_empty());
    assert_eq!(plan.missing.len(), 2);
    assert!(plan.missing.iter().all(|m| m.role == NodeRole::Chosen));
    assert!(plan.missing.iter().any(|m| m.title == "Mod Menu"));
    assert_eq!(plan.target.minecraft, "1.20.1");
    assert_eq!(plan.target.loader.as_deref(), Some("forge"));
}

#[tokio::test]
async fn ca_t09_03_falha_de_rede_antes_de_gravar_nao_deixa_nada() {
    let api = Api::start().await;
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    pack(root, "1.21.1", Loader::Fabric, &[]);
    let plan = plan_of(&api, root, &[MODMENU, APPLESKIN]).await;
    let before = metafiles(root);
    // O Modrinth cai entre o plano e a gravação.
    api.server.reset().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&api.server)
        .await;
    let fresh = Api {
        client: ModrinthClient::with_base_url(
            {
                let mut config = HttpConfig::for_version("1");
                config.max_retries = 0;
                HttpClient::new(config).unwrap()
            },
            &format!("{}/v2", api.server.uri()),
            None,
        )
        .unwrap(),
        server: api.server,
    };
    let cli = packwiz(root).unwrap_or_else(|| {
        Packwiz::new(
            root.join("inexistente.exe"),
            root.join("cache"),
            root.join("config.toml"),
        )
    });
    let error = add_apply(
        root,
        &fresh.sources(),
        &confirm_default(&plan),
        &cli,
        &CancellationToken::new(),
    )
    .await
    .unwrap_err();
    assert_eq!(error.code, Code::SearchSourceUnavailable);
    assert_eq!(error.params["source"], "Modrinth");
    assert_eq!(metafiles(root), before);
}

#[tokio::test]
async fn ca_t08_11_falha_no_refresh_desfaz_todos_os_itens() {
    let api = Api::start().await;
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    pack(root, "1.21.1", Loader::Fabric, &[]);
    let index_before = fs::read(root.join("index.toml")).unwrap();
    let plan = plan_of(&api, root, &[MODMENU, APPLESKIN, SODIUM]).await;
    // Três escolhidos e as duas bibliotecas: cinco arquivos numa transação só.
    assert_eq!(confirm_default(&plan).items.len(), 5);
    let broken = Packwiz::new(
        root.join("inexistente.exe"),
        root.join("cache"),
        root.join("config.toml"),
    );
    assert!(
        add_apply(
            root,
            &api.sources(),
            &confirm_default(&plan),
            &broken,
            &CancellationToken::new(),
        )
        .await
        .is_err()
    );
    assert!(metafiles(root).is_empty(), "{:?}", metafiles(root));
    assert!(!root.join("mods").exists());
    assert_eq!(fs::read(root.join("index.toml")).unwrap(), index_before);
}

#[tokio::test]
async fn gravar_respeita_nomes_unicos_e_pula_o_que_ja_esta() {
    let api = Api::start().await;
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    // Um link direto já usa o nome `sodium` (e um arquivo solto `fabric-api.pw.toml` fora do
    // índice ocupa o nome no disco).
    pack(
        root,
        "1.21.1",
        Loader::Fabric,
        &[(
            "mods/sodium.pw.toml",
            Metafile::url("Outro", "https://example.org/outro.jar", "00").unwrap(),
        )],
    );
    write(root, "mods/fabric-api.pw.toml", "name = \"solto\"\n");
    let Some(cli) = packwiz(root) else { return };
    refresh(root).await;
    let request = AddApplyRequest {
        items: vec![
            ApplyItem {
                source: SourceId::Modrinth,
                project_id: SODIUM.into(),
                version_id: "SMxNOGZ6".into(),
                side: Some(SideChoice::Both),
                replaces: None,
            },
            ApplyItem {
                source: SourceId::Modrinth,
                project_id: FABRIC_API.into(),
                version_id: "Mys3P7lK".into(),
                side: None,
                replaces: None,
            },
        ],
    };
    let result = add_apply(
        root,
        &api.sources(),
        &request,
        &cli,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    let paths: Vec<&str> = result.added.iter().map(|a| a.path.as_str()).collect();
    assert_eq!(
        paths,
        [
            "mods/sodium-modrinth.pw.toml",
            "mods/fabric-api-modrinth.pw.toml"
        ]
    );
    // O lado escolhido na tela vale mais que o sugerido.
    let text = fs::read_to_string(root.join("mods/sodium-modrinth.pw.toml")).unwrap();
    assert!(!text.contains("side = \"client\""), "{text}");
    // De novo: os dois já estão no pack pela mesma fonte e são pulados.
    let again = add_apply(
        root,
        &api.sources(),
        &request,
        &cli,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert!(again.added.is_empty());
    assert_eq!(again.skipped.len(), 2);
}

#[tokio::test]
async fn entradas_invalidas_sao_recusadas_sem_gravar() {
    let api = Api::start().await;
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    pack(root, "1.21.1", Loader::Fabric, &[]);
    let cli = Packwiz::new(
        root.join("inexistente.exe"),
        root.join("cache"),
        root.join("config.toml"),
    );
    let cancel = CancellationToken::new();
    let empty = add_apply(
        root,
        &api.sources(),
        &AddApplyRequest { items: Vec::new() },
        &cli,
        &cancel,
    )
    .await
    .unwrap_err();
    assert_eq!(empty.code, Code::InvalidInput);
    // Versão de outro projeto.
    let wrong = add_apply(
        root,
        &api.sources(),
        &AddApplyRequest {
            items: vec![ApplyItem {
                source: SourceId::Modrinth,
                project_id: MODMENU.into(),
                version_id: "SMxNOGZ6".into(),
                side: None,
                replaces: None,
            }],
        },
        &cli,
        &cancel,
    )
    .await
    .unwrap_err();
    assert_eq!(wrong.code, Code::InvalidInput);
    assert_eq!(wrong.params["field"], "versionId");
    // Substituir um caminho que não é item do pack.
    let outside = add_apply(
        root,
        &api.sources(),
        &AddApplyRequest {
            items: vec![ApplyItem {
                source: SourceId::Modrinth,
                project_id: SODIUM.into(),
                version_id: "SMxNOGZ6".into(),
                side: None,
                replaces: Some("../fora.pw.toml".into()),
            }],
        },
        &cli,
        &cancel,
    )
    .await
    .unwrap_err();
    assert_eq!(outside.code, Code::InvalidInput);
    // CurseForge não ligada nesta etapa.
    let off = add_plan(
        root,
        &api.sources(),
        ChannelPolicy::default(),
        &AddPlanRequest {
            items: vec![AddChoice {
                source: SourceId::Curseforge,
                project_id: "238222".into(),
                version_id: None,
            }],
        },
        &cancel,
    )
    .await
    .unwrap_err();
    assert_eq!(off.code, Code::SearchSourceUnavailable);
    let none = add_plan(
        root,
        &api.sources(),
        ChannelPolicy::default(),
        &AddPlanRequest { items: Vec::new() },
        &cancel,
    )
    .await
    .unwrap_err();
    assert_eq!(none.code, Code::InvalidInput);
    assert!(metafiles(root).is_empty());
}

#[tokio::test]
async fn plano_de_um_item_usa_poucas_requisicoes() {
    let api = Api::start().await;
    let temp = tempfile::tempdir().unwrap();
    pack(temp.path(), "1.21.1", Loader::Fabric, &[]);
    let _ = plan_of(&api, temp.path(), &[SODIUM]).await;
    // Projeto (lote) e versões do projeto: duas requisições.
    assert_eq!(api.requests("/v2/projects").await, 1);
    assert_eq!(api.requests("/v2/project/").await, 1);
    assert_eq!(api.requests("/v2/version_files").await, 0);
}
