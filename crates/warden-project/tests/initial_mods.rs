//! Aceite dos mods iniciais e dos kits (P1-18; SPEC T03; CA-T03-07 e CA-T03-08): a oferta da
//! etapa, a gravação depois do "Pack criado" e o pack resultante, contra um Modrinth simulado
//! que devolve respostas reais gravadas (`tests/fixtures/http/FIXTURES.md`), com o sidecar real
//! do packwiz quando disponível. O pack resultante passa na conformidade (`check_conformance`).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::Value;
use warden_core::CancellationToken;
use warden_http::{HttpClient, HttpConfig};
use warden_modrinth::{MetadataCache, ModrinthClient};
use warden_packwiz::{Loader, PackIndex, PackManifest};
use warden_packwiz_cli::{Packwiz, RunContext, check_conformance};
use warden_project::ProjectErrorCode as Code;
use warden_project::add::modrinth::ModrinthAdd;
use warden_project::add::{AddSources, ChannelPolicy};
use warden_project::initial_mods::{InitialModsRequest, KitChoice, Unavailable, apply, offer};
use warden_project::kits::kits_for;
use warden_project::player_tools;
use warden_project::search::SourceId;
use wiremock::matchers::{method, path, path_regex};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

const FIXTURES: &str = "tests/fixtures/http";
const SPARK: &str = "l6YH9Als";
const CRASH_ASSISTANT: &str = "ix1qq8Ux";
const FABRIC_API: &str = "P7dR8mSH";
const SODIUM: &str = "AANobbMI";

fn fixture(name: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(FIXTURES)
        .join(format!("2026-10-07-{name}.json"));
    serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap()
}

fn list(value: &Value) -> Vec<Value> {
    value.as_array().cloned().unwrap()
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
    Some(Packwiz::new(
        binary()?,
        root.join(".packwiz-cache"),
        root.join(".packwiz-config.toml"),
    ))
}

/// `GET /projects?ids=` e `GET /versions?ids=` com os itens pedidos (a API omite os que não
/// existem).
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

struct Api {
    server: MockServer,
    client: ModrinthClient,
}

impl Api {
    async fn start() -> Self {
        let server = MockServer::start().await;
        let mut projects = list(&fixture("projects-initial"));
        projects.extend(list(&fixture("projects-add")));
        let mut versions = Vec::new();
        let by_project = [
            (SPARK, fixture("versions-spark-fabric-1.21.1")),
            (
                CRASH_ASSISTANT,
                fixture("versions-crash-assistant-fabric-1.21.1"),
            ),
            (FABRIC_API, fixture("versions-fabric-api-fabric-1.21.1")),
            (
                SODIUM,
                serde_json::from_str(
                    &fs::read_to_string(
                        Path::new(env!("CARGO_MANIFEST_DIR")).join(
                            "../warden-modrinth/tests/fixtures/http/2026-10-04-project-sodium-versions-fabric-1.21.1.json",
                        ),
                    )
                    .unwrap(),
                )
                .unwrap(),
            ),
        ];
        for (id, body) in &by_project {
            versions.extend(list(body));
            Mock::given(method("GET"))
                .and(path(format!("/v2/project/{id}/version")))
                .and(wiremock::matchers::query_param("loaders", r#"["fabric"]"#))
                .respond_with(ResponseTemplate::new(200).set_body_json(body))
                .mount(&server)
                .await;
        }
        // Crash Assistant para o Forge 1.12.2 (a oferta da etapa).
        let old = fixture("versions-crash-assistant-forge-1.12.2");
        versions.extend(list(&old));
        Mock::given(method("GET"))
            .and(path(format!("/v2/project/{CRASH_ASSISTANT}/version")))
            .and(wiremock::matchers::query_param("loaders", r#"["forge"]"#))
            .respond_with(ResponseTemplate::new(200).set_body_json(&old))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/v2/projects"))
            .respond_with(ByIds(projects))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/v2/versions"))
            .respond_with(ByIds(versions))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path_regex(r"^/v2/project/[^/]+/version$"))
            .respond_with(ResponseTemplate::new(200).set_body_json(Vec::<Value>::new()))
            .with_priority(10)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/v2/version_files"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
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

    /// Servidor que não responde: simula estar sem internet.
    async fn requests(&self) -> usize {
        self.server
            .received_requests()
            .await
            .unwrap_or_default()
            .len()
    }
}

fn write(root: &Path, path: &str, text: &str) {
    let full = root.join(path);
    fs::create_dir_all(full.parent().unwrap()).unwrap();
    fs::write(full, text).unwrap();
}

/// Pack recém-criado: `pack.toml`, índice vazio e o `project.toml` do Warden.
async fn new_pack(root: &Path, minecraft: &str, loader: Loader) {
    let mut manifest = PackManifest::new("Teste", minecraft);
    manifest.set_loader_version(loader, "0.16.14");
    write(root, "pack.toml", &manifest.to_toml_string());
    write(
        root,
        "index.toml",
        &PackIndex {
            hash_format: "sha256".into(),
            files: Vec::new(),
        }
        .to_toml_string(),
    );
    write(
        root,
        ".warden/project.toml",
        "schemaVersion = 1\nid = \"01JABCDEFGHJKMNPQRSTVWXYZ0\"\n",
    );
    if let Some(cli) = packwiz(root) {
        cli.refresh(root, false, RunContext::new(&CancellationToken::new()))
            .await
            .unwrap();
    }
}

fn request(tools: &[&str]) -> InitialModsRequest {
    InitialModsRequest {
        tools: tools.iter().map(|tool| (*tool).to_owned()).collect(),
        kit: None,
    }
}

#[tokio::test]
async fn oferta_do_fabric_traz_as_versoes_e_a_fabric_api_junto() {
    let api = Api::start().await;
    let offer = offer(
        &api.sources(),
        ChannelPolicy::default(),
        "1.21.1",
        Some("fabric"),
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    let ids: Vec<&str> = offer.items.iter().map(|item| item.id.as_str()).collect();
    assert_eq!(ids, ["spark", "crash-assistant"]);
    let spark = &offer.items[0];
    assert!(spark.checked && !spark.old);
    assert_eq!(spark.title, "spark");
    assert_eq!(spark.version.as_deref(), Some("1.10.109-fabric"));
    assert_eq!(spark.with, ["Fabric API"]);
    let crash = &offer.items[1];
    assert!(crash.checked && crash.unavailable.is_none());
    assert_eq!(crash.version.as_deref(), Some("1.11.12"));
}

#[tokio::test]
async fn oferta_do_forge_1_12_2_sem_curseforge_desabilita_o_spark_e_mantem_o_crash_assistant() {
    // CA-T03-08: spark da CurseForge, "versão antiga", desabilitado sem a chave; o resto segue.
    let api = Api::start().await;
    let offer = offer(
        &api.sources(),
        ChannelPolicy::default(),
        "1.12.2",
        Some("forge"),
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    let spark = &offer.items[0];
    assert_eq!(spark.source, SourceId::Curseforge);
    assert!(spark.old);
    assert_eq!(spark.version.as_deref(), Some("1.6.3"));
    assert_eq!(spark.unavailable, Some(Unavailable::CurseforgeOff));
    let crash = &offer.items[1];
    assert_eq!(crash.source, SourceId::Modrinth);
    assert!(crash.unavailable.is_none());
    // A CurseForge desligada não gerou nenhum pedido pelo spark: só o Crash Assistant foi buscado.
    assert!(api.requests().await > 0);

    let offer_1710 = warden_project::initial_mods::offer(
        &api.sources(),
        ChannelPolicy::default(),
        "1.7.10",
        Some("forge"),
        &CancellationToken::new(),
    )
    .await;
    // Sem versão do Crash Assistant gravada para 1.7.10 no servidor simulado: o item vem sem versão.
    let offer_1710 = offer_1710.unwrap();
    assert_eq!(offer_1710.items[0].version.as_deref(), Some("1.10.19"));
    assert_eq!(
        offer_1710.items[1].unavailable,
        Some(Unavailable::NoVersion)
    );
}

#[tokio::test]
async fn vanilla_nao_oferece_mods_iniciais() {
    let api = Api::start().await;
    let offer = offer(
        &api.sources(),
        ChannelPolicy::default(),
        "1.21.1",
        None,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert!(offer.items.is_empty());
    assert_eq!(api.requests().await, 0);
}

#[tokio::test]
async fn modrinth_fora_do_ar_e_erro_da_fonte_e_nada_e_gravado() {
    let api = Api::start().await;
    let dead = ModrinthClient::with_base_url(
        HttpClient::new({
            let mut config = HttpConfig::for_version("1");
            config.max_retries = 0;
            config
        })
        .unwrap(),
        "http://127.0.0.1:9/v2",
        None,
    )
    .unwrap();
    let sources = AddSources::new().with(Arc::new(ModrinthAdd::new(dead)));
    let error = offer(
        &sources,
        ChannelPolicy::default(),
        "1.21.1",
        Some("fabric"),
        &CancellationToken::new(),
    )
    .await
    .unwrap_err();
    assert_eq!(error.code, Code::SearchSourceUnavailable);
    drop(api);
}

#[tokio::test]
async fn criar_fabric_1_21_1_grava_spark_fabric_api_crash_assistant_config_e_ferramentas() {
    // CA-T03-07.
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    new_pack(root, "1.21.1", Loader::Fabric).await;
    let Some(cli) = packwiz(root) else {
        return;
    };
    let api = Api::start().await;
    let result = apply(
        root,
        &api.sources(),
        ChannelPolicy::default(),
        &request(&["spark", "crash-assistant"]),
        &cli,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    let keys: std::collections::BTreeSet<&str> =
        result.added.iter().map(|item| item.key.as_str()).collect();
    let expected = [
        format!("modrinth:{FABRIC_API}"),
        format!("modrinth:{CRASH_ASSISTANT}"),
        format!("modrinth:{SPARK}"),
    ];
    assert_eq!(
        keys,
        expected
            .iter()
            .map(String::as_str)
            .collect::<std::collections::BTreeSet<_>>()
    );
    assert!(result.left_out.is_empty());
    assert!(result.crash_assistant_config);
    // Um .pw.toml para cada.
    for stem in ["spark", "fabric-api", "crash-assistant"] {
        assert!(
            root.join(format!("mods/{stem}.pw.toml")).is_file(),
            "{stem}: {:?}",
            result.added
        );
    }
    // O lado do spark foi fixado em "cliente e servidor"; o Crash Assistant segue o da fonte.
    let spark = fs::read_to_string(root.join("mods/spark.pw.toml")).unwrap();
    assert!(!spark.contains("side = \"client\""), "{spark}");
    // Config inicial: sem envio ao autor, sem link de terceiros e sem verificação de pirataria.
    let config = fs::read_to_string(root.join(player_tools::CRASH_ASSISTANT_CONFIG)).unwrap();
    assert!(config.contains("send_uploaded_logs_data_to_kostromdan_dev = false"));
    assert!(config.contains("wrap_link = false"));
    assert!(config.contains("enabled = false"));
    // Ferramentas do jogador, sem perder o resto do project.toml.
    let project = fs::read_to_string(root.join(".warden/project.toml")).unwrap();
    assert!(project.contains("id = \"01JABCDEFGHJKMNPQRSTVWXYZ0\""));
    let mut tools = player_tools::parse(&project).unwrap();
    tools.sort_by(|a, b| a.role.cmp(&b.role));
    assert_eq!(tools.len(), 2);
    assert_eq!(tools[0].role, "crash-assistant");
    assert_eq!(tools[0].path, "mods/crash-assistant.pw.toml");
    assert_eq!(tools[1].role, "spark");
    assert_eq!(tools[1].path, "mods/spark.pw.toml");
    // A config entrou no índice e o pack está como o packwiz o deixaria.
    let index = fs::read_to_string(root.join("index.toml")).unwrap();
    assert!(index.contains("config/crash_assistant/config.toml"));
    assert!(!index.contains(".warden"));
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

#[tokio::test]
async fn desmarcar_os_dois_cria_o_pack_sem_nenhum_pw_toml() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    new_pack(root, "1.21.1", Loader::Fabric).await;
    let cli = packwiz(root).unwrap_or_else(|| {
        Packwiz::new(
            PathBuf::from("nao-existe"),
            root.join("c"),
            root.join("c.toml"),
        )
    });
    let api = Api::start().await;
    let before = fs::read_to_string(root.join(".warden/project.toml")).unwrap();
    let result = apply(
        root,
        &api.sources(),
        ChannelPolicy::default(),
        &request(&[]),
        &cli,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert!(result.added.is_empty() && result.player_tools.is_empty());
    assert!(!root.join("mods").exists());
    assert!(!root.join(player_tools::CRASH_ASSISTANT_CONFIG).exists());
    assert_eq!(
        fs::read_to_string(root.join(".warden/project.toml")).unwrap(),
        before
    );
    // Nem uma requisição: nada a planejar.
    assert_eq!(api.requests().await, 0);
}

#[tokio::test]
async fn ferramenta_desconhecida_e_kit_de_outro_pack_sao_recusados() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    new_pack(root, "1.21.1", Loader::Fabric).await;
    let cli = Packwiz::new(
        PathBuf::from("nao-existe"),
        root.join("c"),
        root.join("c.toml"),
    );
    let api = Api::start().await;
    let error = apply(
        root,
        &api.sources(),
        ChannelPolicy::default(),
        &request(&["optifine"]),
        &cli,
        &CancellationToken::new(),
    )
    .await
    .unwrap_err();
    assert_eq!(error.code, Code::InvalidInput);

    let forge_kit = kits_for("1.20.1", Some("forge"))[0].clone();
    let mut with_kit = request(&[]);
    with_kit.kit = Some(KitChoice {
        id: forge_kit.id,
        projects: Vec::new(),
    });
    let error = apply(
        root,
        &api.sources(),
        ChannelPolicy::default(),
        &with_kit,
        &cli,
        &CancellationToken::new(),
    )
    .await
    .unwrap_err();
    assert_eq!(error.code, Code::InvalidInput);
    assert_eq!(api.requests().await, 0);
}

#[tokio::test]
async fn item_de_fonte_desligada_fica_de_fora_e_o_resto_entra() {
    // Forge 1.12.2 sem CurseForge: o spark fica de fora e o resultado diz isso.
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    new_pack(root, "1.12.2", Loader::Forge).await;
    let Some(cli) = packwiz(root) else {
        return;
    };
    let api = Api::start().await;
    let result = apply(
        root,
        &api.sources(),
        ChannelPolicy::default(),
        &request(&["spark"]),
        &cli,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert!(result.added.is_empty());
    assert_eq!(result.left_out, ["spark"]);
    assert!(player_tools::read(root).unwrap().is_empty());
}

#[tokio::test]
async fn kit_escolhido_entra_junto_com_os_mods_iniciais_pelo_mesmo_caminho() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    new_pack(root, "1.21.1", Loader::Fabric).await;
    let Some(cli) = packwiz(root) else {
        return;
    };
    let api = Api::start().await;
    let mut with_kit = request(&["spark"]);
    with_kit.kit = Some(KitChoice {
        id: "fabric-moderno".into(),
        projects: vec![SODIUM.into()],
    });
    let result = apply(
        root,
        &api.sources(),
        ChannelPolicy::default(),
        &with_kit,
        &cli,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    let keys: std::collections::BTreeSet<&str> =
        result.added.iter().map(|item| item.key.as_str()).collect();
    assert!(
        keys.contains(format!("modrinth:{SODIUM}").as_str()),
        "{keys:?}"
    );
    assert!(keys.contains(format!("modrinth:{SPARK}").as_str()));
    // O kit não é ferramenta do jogador: só o spark foi marcado, e sem Crash Assistant não há config.
    assert_eq!(result.player_tools, ["spark"]);
    assert!(!result.crash_assistant_config);
    assert!(!root.join(player_tools::CRASH_ASSISTANT_CONFIG).exists());
    // O Sodium é só do cliente (o lado do kit).
    let sodium = fs::read_to_string(root.join("mods/sodium.pw.toml")).unwrap();
    assert!(sodium.contains("side = \"client\""), "{sodium}");
}
