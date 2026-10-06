//! Aceite do editor do pack (P1-08): inventário, detalhes, lado, remover e informações, com o
//! sidecar real do packwiz quando disponível e sem rede (porta fechada no lugar das APIs).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};

use warden_catalog::Loader;
use warden_core::CancellationToken;
use warden_curseforge::{CurseforgeClient, SecretString};
use warden_http::{HttpClient, HttpConfig};
use warden_modrinth::{MetadataCache, ModrinthClient, Project, UnixClock, Version};
use warden_packwiz::{
    CurseForgeFile, HashFormat, IndexEntry, Metafile, ModrinthFile, PackIndex, PackManifest, Side,
};
use warden_packwiz_cli::{Packwiz, RunContext};
use warden_project::ProjectErrorCode as Code;
use warden_project::create::{CreatePack, create};
use warden_project::details::{DescriptionFormat, DetailsSource, item_details};
use warden_project::inventory::{ItemSide, ItemSource, ItemState, inventory};
use warden_project::meta::{MetaUpdate, read_meta, update_meta};
use warden_project::registry::Registry;
use warden_project::remove::{direct_dependencies, removal_plan, remove};
use warden_project::side::{SideChoice, set_sides};

const SODIUM_PROJECT: &str =
    include_str!("../../warden-modrinth/tests/fixtures/http/2026-10-04-project-sodium.json");
const SODIUM_VERSION: &str =
    include_str!("../../warden-modrinth/tests/fixtures/http/2026-10-04-version-SMxNOGZ6.json");
const PROJECTS_3: &str =
    include_str!("../../warden-modrinth/tests/fixtures/http/2026-10-04-projects-3.json");

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

/// Endereço local em que nada escuta: simula "sem internet".
fn closed_port() -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    format!("http://127.0.0.1:{port}/")
}

fn offline_http() -> HttpClient {
    let mut config = HttpConfig::for_version("1");
    config.max_retries = 0;
    HttpClient::new(config).unwrap()
}

struct Offline {
    modrinth: ModrinthClient,
    cache: MetadataCache,
    now: Arc<AtomicI64>,
}

fn offline_modrinth() -> Offline {
    let now = Arc::new(AtomicI64::new(1_790_000_000));
    let reader = Arc::clone(&now);
    let clock: UnixClock = Arc::new(move || reader.load(Ordering::SeqCst));
    let cache = MetadataCache::in_memory(Some(clock)).unwrap();
    let modrinth =
        ModrinthClient::with_base_url(offline_http(), &closed_port(), Some(cache.clone())).unwrap();
    Offline {
        modrinth,
        cache,
        now,
    }
}

async fn put_project(cache: &MetadataCache, json: &str) {
    let project: Project = serde_json::from_str(json).unwrap();
    cache
        .put_projects(vec![(project, json.to_owned())])
        .await
        .unwrap();
}

async fn put_version(cache: &MetadataCache, json: &str) {
    let version: Version = serde_json::from_str(json).unwrap();
    cache
        .put_versions(vec![(version, json.to_owned())], true)
        .await
        .unwrap();
}

fn write(root: &Path, path: &str, text: &str) {
    let full = root.join(path);
    fs::create_dir_all(full.parent().unwrap()).unwrap();
    fs::write(full, text).unwrap();
}

fn modrinth_metafile(title: &str, project: &str, version: &str, filename: &str) -> Metafile {
    Metafile::modrinth(
        &ModrinthFile {
            title: title.into(),
            project_id: project.into(),
            version_id: version.into(),
            filename: filename.into(),
            url: format!("https://cdn.modrinth.com/data/{project}/versions/{version}/{filename}"),
            hash_format: HashFormat::Sha512,
            hash: "ab".repeat(64),
        },
        Side::Both,
    )
}

fn curseforge_metafile() -> Metafile {
    Metafile::curseforge(
        &CurseForgeFile {
            name: "Just Enough Items (JEI)".into(),
            project_id: 238_222,
            file_id: 5_101_366,
            filename: "jei-1.20.1-forge-15.2.0.27.jar".into(),
            hash_format: HashFormat::Sha1,
            hash: "0123456789abcdef0123456789abcdef01234567".into(),
        },
        Side::Client,
    )
}

/// Pack escrito à mão (sem packwiz), com o índice listando os arquivos dados.
fn manual_pack(root: &Path, files: &[(&str, String)]) {
    write(
        root,
        "pack.toml",
        &PackManifest::new("Manual", "1.21.1").to_toml_string(),
    );
    let index = PackIndex {
        hash_format: "sha256".into(),
        files: files
            .iter()
            .map(|(path, _)| IndexEntry::new(path, "00"))
            .collect(),
    };
    write(root, "index.toml", &index.to_toml_string());
    for (path, text) in files {
        write(root, path, text);
    }
}

#[tokio::test]
async fn ca_t06_01_um_metafile_invalido_nao_derruba_os_outros_99() {
    let temp = tempfile::tempdir().unwrap();
    let mut files: Vec<(String, String)> = (0..99)
        .map(|n| {
            (
                format!("mods/mod{n:03}.pw.toml"),
                Metafile::url(
                    &format!("Mod {n:03}"),
                    &format!("https://example.org/mod{n:03}.jar"),
                    "00",
                )
                .unwrap()
                .to_toml_string(),
            )
        })
        .collect();
    files.push((
        "mods/quebrado.pw.toml".into(),
        "name = \"Quebrado\"\nfilename = \n".into(),
    ));
    let borrowed: Vec<(&str, String)> = files
        .iter()
        .map(|(path, text)| (path.as_str(), text.clone()))
        .collect();
    manual_pack(temp.path(), &borrowed);
    let result = inventory(temp.path(), None).await.unwrap();
    assert_eq!(result.items.len(), 100);
    assert!(result.index_error.is_none());
    let invalid: Vec<_> = result
        .items
        .iter()
        .filter(|item| item.state == ItemState::Invalid)
        .collect();
    assert_eq!(invalid.len(), 1);
    assert_eq!(invalid[0].path, "mods/quebrado.pw.toml");
    assert!(invalid[0].error.is_some());
    let valid = result
        .items
        .iter()
        .filter(|item| item.state == ItemState::Ok)
        .count();
    assert_eq!(valid, 99);
    assert_eq!(result.items[0].name, "Mod 000");
    assert_eq!(result.items[0].version.as_deref(), Some("mod000"));
}

fn looks_like_internal_id(version: &str) -> bool {
    (version.len() == 8 && version.chars().all(|c| c.is_ascii_alphanumeric()))
        || version.chars().all(|c| c.is_ascii_digit())
}

#[tokio::test]
async fn ca_t06_02_versao_legivel_do_cache_e_nunca_um_id() {
    let temp = tempfile::tempdir().unwrap();
    let offline = offline_modrinth();
    put_project(&offline.cache, SODIUM_PROJECT).await;
    put_version(&offline.cache, SODIUM_VERSION).await;
    // Cache vencido e sem internet: o inventário usa o que houver.
    offline.now.fetch_add(48 * 60 * 60, Ordering::SeqCst);
    manual_pack(
        temp.path(),
        &[
            (
                "mods/sodium.pw.toml",
                modrinth_metafile(
                    "Sodium",
                    "AANobbMI",
                    "SMxNOGZ6",
                    "sodium-fabric-0.8.13+mc1.21.1.jar",
                )
                .to_toml_string(),
            ),
            (
                "mods/lithium.pw.toml",
                modrinth_metafile(
                    "Lithium",
                    "gvQqBUqZ",
                    "Abc12345",
                    "lithium-fabric-0.15.0+mc1.21.1.jar",
                )
                .to_toml_string(),
            ),
            ("mods/jei.pw.toml", curseforge_metafile().to_toml_string()),
        ],
    );
    let result = inventory(temp.path(), Some(&offline.modrinth))
        .await
        .unwrap();
    let by_path = |path: &str| result.items.iter().find(|i| i.path == path).unwrap();
    let sodium = by_path("mods/sodium.pw.toml");
    assert_eq!(sodium.version.as_deref(), Some("mc1.21.1-0.8.13-fabric"));
    assert!(
        sodium
            .summary
            .as_deref()
            .unwrap()
            .starts_with("A high-performance")
    );
    assert!(sodium.icon_url.is_some());
    assert_eq!(sodium.key, "modrinth:AANobbMI");
    // Sem cache da versão: o nome do arquivo, não o ID.
    let lithium = by_path("mods/lithium.pw.toml");
    assert_eq!(
        lithium.version.as_deref(),
        Some("lithium-fabric-0.15.0+mc1.21.1")
    );
    let jei = by_path("mods/jei.pw.toml");
    assert_eq!(jei.source, ItemSource::Curseforge);
    assert_eq!(jei.version.as_deref(), Some("jei-1.20.1-forge-15.2.0.27"));
    assert_eq!(jei.side, ItemSide::Client);
    for item in &result.items {
        let version = item.version.as_deref().unwrap();
        assert!(!looks_like_internal_id(version), "{version}");
    }
}

#[tokio::test]
async fn ca_t07_02_sem_internet_modrinth_abre_do_cache_e_curseforge_mostra_o_pw_toml() {
    let temp = tempfile::tempdir().unwrap();
    let offline = offline_modrinth();
    put_project(&offline.cache, SODIUM_PROJECT).await;
    put_version(&offline.cache, SODIUM_VERSION).await;
    offline.now.fetch_add(48 * 60 * 60, Ordering::SeqCst);
    let jei = curseforge_metafile();
    manual_pack(
        temp.path(),
        &[
            (
                "mods/sodium.pw.toml",
                modrinth_metafile(
                    "Sodium",
                    "AANobbMI",
                    "SMxNOGZ6",
                    "sodium-fabric-0.8.13+mc1.21.1.jar",
                )
                .to_toml_string(),
            ),
            ("mods/jei.pw.toml", jei.to_toml_string()),
            ("mods/local.jar", "jar".into()),
        ],
    );
    let cf = CurseforgeClient::with_base_url(
        offline_http(),
        &closed_port(),
        Some(SecretString::from("chave-de-teste".to_owned())),
    )
    .unwrap();
    let details = item_details(
        temp.path(),
        "mods/sodium.pw.toml",
        Some(&offline.modrinth),
        Some(&cf),
        None,
    )
    .await
    .unwrap();
    assert_eq!(details.source, DetailsSource::Cache);
    assert_eq!(details.title, "Sodium");
    assert_eq!(
        details.page_url.as_deref(),
        Some("https://modrinth.com/mod/sodium")
    );
    assert_eq!(details.description_format, DescriptionFormat::Markdown);
    assert!(details.description.is_some());
    let version = details.version.unwrap();
    assert_eq!(version.number, "mc1.21.1-0.8.13-fabric");
    assert_eq!(version.loaders, ["fabric"]);
    assert_eq!(version.size_bytes, Some(1_574_609.0));
    assert!(details.changelog.is_some());
    assert_eq!(details.file.unwrap().hash_format, "sha512");

    let details = item_details(
        temp.path(),
        "mods/jei.pw.toml",
        Some(&offline.modrinth),
        Some(&cf),
        None,
    )
    .await
    .unwrap();
    assert_eq!(details.source, DetailsSource::Offline);
    assert_eq!(details.title, "Just Enough Items (JEI)");
    assert_eq!(details.item.side, ItemSide::Client);
    assert_eq!(details.description_format, DescriptionFormat::Html);
    let file = details.file.unwrap();
    assert_eq!(file.file_name, "jei-1.20.1-forge-15.2.0.27.jar");
    assert_eq!(file.hash_format, "sha1");
    assert_eq!(file.hash, jei.download.hash);

    // Sem chave da CurseForge.
    let no_key = CurseforgeClient::with_base_url(offline_http(), &closed_port(), None).unwrap();
    let details = item_details(temp.path(), "mods/jei.pw.toml", None, Some(&no_key), None)
        .await
        .unwrap();
    assert_eq!(details.source, DetailsSource::NoKey);
    assert!(details.file.is_some());

    // Arquivo local: sem fonte, com o hash do índice.
    let details = item_details(temp.path(), "mods/local.jar", None, None, None)
        .await
        .unwrap();
    assert_eq!(details.source, DetailsSource::None);
    assert_eq!(details.file.unwrap().hash_format, "sha256");

    let error = item_details(temp.path(), "mods/nada.pw.toml", None, None, None)
        .await
        .unwrap_err();
    assert_eq!(error.code, Code::ItemNotFound);
    assert_eq!(error.params["path"], "mods/nada.pw.toml");
}

#[tokio::test]
async fn detalhes_do_modrinth_dentro_da_validade_contam_como_atuais() {
    let temp = tempfile::tempdir().unwrap();
    let offline = offline_modrinth();
    put_project(&offline.cache, SODIUM_PROJECT).await;
    manual_pack(
        temp.path(),
        &[(
            "mods/sodium.pw.toml",
            modrinth_metafile("Sodium", "AANobbMI", "SMxNOGZ6", "sodium.jar").to_toml_string(),
        )],
    );
    let details = item_details(
        temp.path(),
        "mods/sodium.pw.toml",
        Some(&offline.modrinth),
        None,
        None,
    )
    .await
    .unwrap();
    assert_eq!(details.source, DetailsSource::Live);
    // A versão não estava no cache e a rede falhou: o painel continua sem ela.
    assert!(details.version.is_none());
    assert_eq!(details.item.version.as_deref(), Some("sodium"));

    // Sem cache nenhum e sem internet.
    let empty = offline_modrinth();
    let details = item_details(
        temp.path(),
        "mods/sodium.pw.toml",
        Some(&empty.modrinth),
        None,
        None,
    )
    .await
    .unwrap();
    assert_eq!(details.source, DetailsSource::Offline);
    assert_eq!(details.title, "Sodium");
}

#[tokio::test]
async fn dependencias_diretas_pelo_cache_do_modrinth() {
    let temp = tempfile::tempdir().unwrap();
    let offline = offline_modrinth();
    let projects: Vec<serde_json::Value> = serde_json::from_str(PROJECTS_3).unwrap();
    for project in &projects {
        put_project(&offline.cache, &project.to_string()).await;
    }
    // Versão real do Sodium com uma dependência obrigatória da Fabric API acrescentada.
    let mut sodium: serde_json::Value = serde_json::from_str(SODIUM_VERSION).unwrap();
    sodium["dependencies"] = serde_json::json!([
        { "project_id": "P7dR8mSH", "version_id": null, "file_name": null, "dependency_type": "required" },
        { "project_id": "gvQqBUqZ", "version_id": null, "file_name": null, "dependency_type": "optional" },
        { "project_id": "naoestaa", "version_id": null, "file_name": null, "dependency_type": "required" }
    ]);
    put_version(&offline.cache, &sodium.to_string()).await;
    manual_pack(
        temp.path(),
        &[
            (
                "mods/sodium.pw.toml",
                modrinth_metafile("Sodium", "AANobbMI", "SMxNOGZ6", "sodium.jar").to_toml_string(),
            ),
            (
                "mods/fabric-api.pw.toml",
                modrinth_metafile("Fabric API", "P7dR8mSH", "Fapi0001", "fabric-api.jar")
                    .to_toml_string(),
            ),
            (
                "mods/lithium.pw.toml",
                modrinth_metafile("Lithium", "gvQqBUqZ", "Lith0001", "lithium.jar")
                    .to_toml_string(),
            ),
        ],
    );
    let result = inventory(temp.path(), Some(&offline.modrinth))
        .await
        .unwrap();
    let deps = direct_dependencies(&result, Some(&offline.modrinth)).await;
    assert_eq!(deps["modrinth:AANobbMI"], ["modrinth:P7dR8mSH"]);
    assert!(!deps.contains_key("modrinth:gvQqBUqZ"));
    assert!(direct_dependencies(&result, None).await.is_empty());
    let plan = removal_plan(&result, &["mods/fabric-api.pw.toml".into()], &deps).unwrap();
    assert_eq!(plan.targets[0].name, "Fabric API");
    assert_eq!(plan.dependents.len(), 1);
    assert_eq!(plan.dependents[0].name, "Sodium");
    assert_eq!(plan.dependents[0].needs, ["Fabric API"]);
}

// ---------------------------------------------------------------------------------------------
// Com o packwiz real.

fn services(root: &Path, binary: PathBuf) -> (Registry, Packwiz) {
    fs::create_dir_all(root.join("dados")).unwrap();
    (
        Registry::open(root.join("dados/packs.json")).unwrap(),
        Packwiz::new(
            binary,
            root.join("dados/cache"),
            root.join("dados/config.toml"),
        ),
    )
}

/// Pack criado pelo Warden (P1-07) com `count` mods do Modrinth, já indexados pelo packwiz.
async fn real_pack(root: &Path, binary: PathBuf, count: usize) -> (PathBuf, Packwiz) {
    let (registry, cli) = services(root, binary);
    let made = create(
        &CreatePack {
            name: "Pack Editor".into(),
            author: "Autor".into(),
            description: "Descrição".into(),
            destination: Some(root.join("pack")),
            minecraft: "1.20.1".into(),
            loader: Some(Loader::Fabric),
            loader_version: Some("0.16.14".into()),
        },
        root,
        &registry,
        &cli,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    for n in 0..count {
        write(
            &made.path,
            &format!("mods/mod{n:02}.pw.toml"),
            &modrinth_metafile(
                &format!("Mod {n:02}"),
                &format!("proj{n:04}"),
                &format!("vers{n:04}"),
                &format!("mod{n:02}-1.0.{n}.jar"),
            )
            .to_toml_string(),
        );
    }
    refresh(&cli, &made.path).await;
    (made.path, cli)
}

async fn refresh(cli: &Packwiz, path: &Path) {
    cli.refresh(path, false, RunContext::new(&CancellationToken::new()))
        .await
        .unwrap();
}

/// Confere que um `refresh` seguinte não muda o manifesto nem o índice.
async fn assert_refresh_is_noop(cli: &Packwiz, path: &Path) {
    let manifest = fs::read(path.join("pack.toml")).unwrap();
    let index = fs::read(path.join("index.toml")).unwrap();
    refresh(cli, path).await;
    assert_eq!(fs::read(path.join("pack.toml")).unwrap(), manifest);
    assert_eq!(fs::read(path.join("index.toml")).unwrap(), index);
}

fn changed_lines(before: &str, after: &str) -> Vec<(String, String)> {
    let before: Vec<&str> = before.lines().collect();
    let after: Vec<&str> = after.lines().collect();
    assert_eq!(before.len(), after.len(), "número de linhas mudou");
    before
        .iter()
        .zip(&after)
        .filter(|(a, b)| a != b)
        .map(|(a, b)| ((*a).to_owned(), (*b).to_owned()))
        .collect()
}

fn index_of(path: &Path) -> PackIndex {
    warden_packwiz::read_pack(path)
        .unwrap()
        .index
        .unwrap()
        .value
}

#[tokio::test]
async fn ca_t06_03_lado_de_10_mods_muda_so_a_linha_side_com_um_refresh() {
    let Some(binary) = binary() else { return };
    let temp = tempfile::tempdir().unwrap();
    let (pack, cli) = real_pack(temp.path(), binary, 10).await;
    let paths: Vec<String> = (0..10).map(|n| format!("mods/mod{n:02}.pw.toml")).collect();
    let before: Vec<String> = paths
        .iter()
        .map(|p| fs::read_to_string(pack.join(p)).unwrap())
        .collect();
    let index_before = index_of(&pack);
    let ignore_before = fs::read(pack.join(".packwizignore")).unwrap();
    let written = set_sides(
        &pack,
        &paths,
        SideChoice::Client,
        &cli,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert_eq!(written, paths);
    for (path, old) in paths.iter().zip(&before) {
        let new = fs::read_to_string(pack.join(path)).unwrap();
        assert_eq!(
            changed_lines(old, &new),
            [("side = \"both\"".to_owned(), "side = \"client\"".to_owned())],
            "{path}"
        );
    }
    let index_after = index_of(&pack);
    for path in &paths {
        assert_ne!(
            index_before.entry(path).unwrap().hash,
            index_after.entry(path).unwrap().hash,
            "{path}"
        );
    }
    assert_eq!(index_before.files.len(), index_after.files.len());
    assert_eq!(
        fs::read(pack.join(".packwizignore")).unwrap(),
        ignore_before
    );
    assert_refresh_is_noop(&cli, &pack).await;
    let listed = inventory(&pack, None).await.unwrap();
    assert!(
        listed
            .items
            .iter()
            .all(|item| item.side == ItemSide::Client)
    );
    // Pedir de novo o mesmo lado não muda nada.
    let again = set_sides(
        &pack,
        &paths,
        SideChoice::Client,
        &cli,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert!(again.is_empty());
}

#[tokio::test]
async fn ca_t06_04_remover_apaga_o_pw_toml_e_a_entrada_do_indice() {
    let Some(binary) = binary() else { return };
    let temp = tempfile::tempdir().unwrap();
    let (pack, cli) = real_pack(temp.path(), binary, 3).await;
    let removed = remove(
        &pack,
        &["mods/mod01.pw.toml".into()],
        &cli,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert_eq!(removed, ["mods/mod01.pw.toml"]);
    assert!(!pack.join("mods/mod01.pw.toml").exists());
    let index = index_of(&pack);
    assert!(index.entry("mods/mod01.pw.toml").is_none());
    assert!(index.entry("mods/mod00.pw.toml").is_some());
    assert_refresh_is_noop(&cli, &pack).await;
    let error = remove(
        &pack,
        &["mods/mod01.pw.toml".into()],
        &cli,
        &CancellationToken::new(),
    )
    .await
    .unwrap_err();
    assert_eq!(error.code, Code::ItemNotFound);
}

#[tokio::test]
async fn transacoes_seguidas_no_mesmo_pack_mantem_o_indice_valido() {
    let Some(binary) = binary() else { return };
    let temp = tempfile::tempdir().unwrap();
    let (pack, cli) = real_pack(temp.path(), binary, 4).await;
    // Um jar fora do índice, que o packwiz indexaria num refresh feito por fora.
    write(&pack, "mods/solto.jar", "conteúdo");
    let listed = inventory(&pack, None).await.unwrap();
    let solto = listed
        .items
        .iter()
        .find(|item| item.path == "mods/solto.jar")
        .unwrap();
    assert_eq!(solto.state, ItemState::OutsideIndex);
    let cancel = CancellationToken::new();
    remove(&pack, &["mods/mod00.pw.toml".into()], &cli, &cancel)
        .await
        .unwrap();
    set_sides(
        &pack,
        &["mods/mod02.pw.toml".into()],
        SideChoice::Server,
        &cli,
        &cancel,
    )
    .await
    .unwrap();
    remove(
        &pack,
        &["mods/mod03.pw.toml".into(), "mods/solto.jar".into()],
        &cli,
        &cancel,
    )
    .await
    .unwrap();
    assert!(!pack.join("mods/solto.jar").exists());
    let index = index_of(&pack);
    let mods: Vec<&str> = index
        .files
        .iter()
        .map(|entry| entry.file.as_str())
        .filter(|file| file.starts_with("mods/"))
        .collect();
    assert_eq!(mods, ["mods/mod01.pw.toml", "mods/mod02.pw.toml"]);
    assert_refresh_is_noop(&cli, &pack).await;
    let listed = inventory(&pack, None).await.unwrap();
    assert_eq!(listed.items.len(), 2);
    assert_eq!(listed.items[1].side, ItemSide::Server);
}

#[tokio::test]
async fn ca_t11_01_mudar_o_nome_altera_so_a_linha_name() {
    let Some(binary) = binary() else { return };
    let temp = tempfile::tempdir().unwrap();
    let (pack, cli) = real_pack(temp.path(), binary, 1).await;
    let before = fs::read_to_string(pack.join("pack.toml")).unwrap();
    let index_before = fs::read(pack.join("index.toml")).unwrap();
    let current = read_meta(&pack).unwrap();
    assert_eq!(current.loader.as_deref(), Some("fabric"));
    let meta = update_meta(
        &pack,
        &MetaUpdate {
            name: "Nome Novo".into(),
            author: current.author.clone(),
            description: current.description.clone(),
        },
    )
    .await
    .unwrap();
    assert_eq!(meta.name, "Nome Novo");
    let after = fs::read_to_string(pack.join("pack.toml")).unwrap();
    assert_eq!(
        changed_lines(&before, &after),
        [(
            "name = \"Pack Editor\"".to_owned(),
            "name = \"Nome Novo\"".to_owned()
        )]
    );
    assert_eq!(fs::read(pack.join("index.toml")).unwrap(), index_before);
    assert_refresh_is_noop(&cli, &pack).await;
    assert_eq!(read_meta(&pack).unwrap().name, "Nome Novo");
}
