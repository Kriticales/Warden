//! "Testar como o jogador recebe" com o packwiz-installer-bootstrap real (ROADMAP L-07;
//! CA-T13-14; ARCHITECTURE §8.5).
//!
//! Um servidor local imita o `raw.githubusercontent.com` (o pack em `/<dono>/<repo>/main/`),
//! os arquivos dos mods e a URL de download dos Releases (os jars fixados). O bootstrap roda
//! com `--bootstrap-no-update` e com `--bootstrap-update-url` apontando para o mesmo servidor:
//! se ele tentasse atualizar, o pedido apareceria no registro. Um proxy HTTPS fechado garante
//! que nada sai para a internet.
//!
//! Requer `cargo xtask installer` (Java e jars fixados). Sem eles, os testes avisam e passam;
//! com `WARDEN_REQUIRE_EXTERNALS=1`, falham.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod conformance_support;

use std::fs;
use std::path::Path;

use conformance_support::{
    Externals, TestMod, TestPack, TestSide, curseforge_client, curseforge_mod, externals, temp_dir,
};
use warden_core::{CancellationToken, NoProgress};
use warden_http::{HttpClient, HttpConfig, Url};
use warden_instance::player_install::{
    CompareInput, InstallerCommand, LocalPackServer, PlayerItemKind, PlayerSource, Received,
    blocked_notices, compare, supply_blocked,
};
use warden_instance::{
    BOOTSTRAP, CleanInstance, DownloadCache, INSTALLER, InstallerCache, InstallerJars,
};
use warden_packwiz::HashFormat;
use warden_packwiz::hash::hash_bytes;
use wiremock::matchers::{method, path, path_regex};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Repositório simulado.
const RAW_PREFIX: &str = "/dono/vale/main";

/// Marca o Crash Assistant como ferramenta do jogador (D16).
const PROJECT: &str = "[player-tools]\ncrash-assistant = \"mods/crash-assistant.pw.toml\"\n";

/// Proxy HTTPS fechado: qualquer `https://` (API do GitHub, CurseForge) falha na hora.
const NO_INTERNET: [&str; 2] = ["-Dhttps.proxyHost=127.0.0.1", "-Dhttps.proxyPort=9"];

fn pack_v1() -> TestPack {
    TestPack::default()
        .with_mod(TestMod::new("sodium").side(TestSide::Client))
        .with_mod(TestMod::new("lithium"))
        .with_mod(TestMod::new("crash-assistant").side(TestSide::Client))
        .with_mod(TestMod::new("so-servidor").side(TestSide::Server))
        .with_mod(TestMod::new("zoom").side(TestSide::Client).optional(false))
        .with_file("config/sodium.json", "{}\n")
}

/// Grava o pack (com o `[player-tools]`) e devolve o texto do `pack.toml`.
fn write_pack(pack: &TestPack, root: &Path, server: &MockServer) -> Vec<u8> {
    pack.write_to(root, &server.uri());
    conformance_support::write(root, ".warden/project.toml", PROJECT.as_bytes());
    fs::read(root.join("pack.toml")).unwrap()
}

/// Serve no `raw` simulado cada arquivo do pack (menos `.warden/`).
async fn serve_raw(root: &Path, server: &MockServer) {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<(String, Vec<u8>)>) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            let relative = path
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            if relative.starts_with(".warden") {
                continue;
            }
            if path.is_dir() {
                walk(root, &path, out);
            } else {
                out.push((relative, fs::read(&path).unwrap()));
            }
        }
    }
    let mut files = Vec::new();
    walk(root, root, &mut files);
    for (relative, bytes) in files {
        Mock::given(method("GET"))
            .and(path(format!("{RAW_PREFIX}/{relative}")))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(bytes))
            .with_priority(1)
            .mount(server)
            .await;
    }
}

/// Os jars fixados na "URL de download do Release" simulada.
async fn serve_jars(externals: &Externals, server: &MockServer) {
    for (jar, file) in [
        (BOOTSTRAP, &externals.bootstrap),
        (INSTALLER, &externals.installer),
    ] {
        Mock::given(method("GET"))
            .and(path(format!("/releases/{}", jar.file_name)))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(fs::read(file).unwrap()))
            .mount(server)
            .await;
    }
}

async fn jars(cache_dir: &Path, server: &MockServer) -> InstallerJars {
    let base = Url::parse(&format!("{}/releases/", server.uri())).unwrap();
    InstallerCache::new(cache_dir)
        .with_base(Some(base))
        .ensure(
            &HttpClient::new(HttpConfig::default()).unwrap(),
            &NoProgress,
            &CancellationToken::new(),
        )
        .await
        .unwrap()
}

fn command(
    externals: &Externals,
    jars: InstallerJars,
    link: Url,
    clean: &CleanInstance,
    update_url: Option<String>,
) -> InstallerCommand {
    InstallerCommand {
        java: externals.java.clone(),
        jars,
        link,
        pack_folder: clean.dirs().game_dir.clone(),
        work_dir: clean.work_dir(),
        extra_jvm_args: NO_INTERNET.iter().map(|arg| (*arg).to_owned()).collect(),
        update_url,
    }
}

/// O pack 1.0.0 publicado no `raw` simulado, com os jars na "URL do Release" e a "API do
/// GitHub" registrando (e recusando com 403, como no limite) qualquer chamada.
struct Published {
    dir: tempfile::TempDir,
    server: MockServer,
    externals: Externals,
    jars: InstallerJars,
    link: Url,
    root: std::path::PathBuf,
    pack: TestPack,
    pack_toml: Vec<u8>,
}

impl Published {
    async fn start() -> Option<Self> {
        let externals = externals()?;
        let dir = temp_dir();
        let server = MockServer::start().await;
        Mock::given(path_regex("^/api/"))
            .respond_with(ResponseTemplate::new(403))
            .mount(&server)
            .await;
        serve_jars(&externals, &server).await;
        let pack = pack_v1();
        pack.serve(&server).await;
        let root = dir.path().join("publicado");
        let pack_toml = write_pack(&pack, &root, &server);
        serve_raw(&root, &server).await;
        let jars = jars(&dir.path().join("cache-installer"), &server).await;
        let link = Url::parse(&format!("{}{RAW_PREFIX}/pack.toml", server.uri())).unwrap();
        Some(Self {
            dir,
            server,
            externals,
            jars,
            link,
            root,
            pack,
            pack_toml,
        })
    }

    fn update_url(&self) -> String {
        format!(
            "{}/api/repos/packwiz/packwiz-installer/releases",
            self.server.uri()
        )
    }

    fn source(&self, version: &str) -> PlayerSource {
        PlayerSource::Published {
            link: self.link.to_string(),
            version: Some(version.into()),
        }
    }
}

/// CA-T13-14: pelo link do `raw` simulado, com o bootstrap real, `--bootstrap-no-update` e
/// nenhuma chamada à API do GitHub; o Crash Assistant chega à instância limpa.
#[tokio::test(flavor = "multi_thread")]
async fn instala_pelo_link_com_o_bootstrap_real_sem_api_do_github() {
    let Some(ctx) = Published::start().await else {
        return;
    };
    let clean = CleanInstance::create(&ctx.dir.path().join("tmp"), "teste1").unwrap();
    let lines = std::sync::Mutex::new(Vec::new());
    let run = command(
        &ctx.externals,
        ctx.jars.clone(),
        ctx.link.clone(),
        &clean,
        Some(ctx.update_url()),
    )
    .run(&CancellationToken::new(), &|line| {
        lines.lock().unwrap().push(line.to_owned());
    })
    .await
    .unwrap();
    assert!(run.success, "{:#?}", run.lines);
    assert_eq!(
        lines.lock().unwrap().len(),
        run.lines.len(),
        "cada linha passou pelo aviso"
    );

    let game = &clean.dirs().game_dir;
    assert!(
        game.join("mods/crash-assistant-1.0.jar").is_file(),
        "Crash Assistant (D16)"
    );
    assert!(
        !game.join("mods/so-servidor-1.0.jar").exists(),
        "lado do servidor fica fora"
    );
    assert!(!game.join("saves").exists());
    let received = Received::read(game).unwrap();
    assert_eq!(
        received.pack_sha256.as_deref(),
        Some(hash_bytes(HashFormat::Sha256, &ctx.pack_toml).as_str())
    );
    let report = compare(&CompareInput {
        pack_root: &ctx.root,
        game_dir: game,
        received: &received,
        blocked: &blocked_notices(&run.lines, game),
        source: ctx.source("1.0.0"),
        received_version: Some("1.0.0".into()),
        published_sha256: received.pack_sha256.clone(),
    })
    .unwrap();
    assert!(!report.outdated);
    assert_eq!(report.crash_assistant, Some(true));
    let kinds: Vec<_> = report
        .items
        .iter()
        .map(|item| (item.kind, item.path.as_str()))
        .collect();
    assert_eq!(kinds, [(PlayerItemKind::Optional, "mods/zoom-1.0.jar")]);
    assert_eq!(report.items[0].default_on, Some(false));
    assert_eq!(report.received_files, 5);

    // Nenhuma chamada à API do GitHub, nem para atualizar o installer.
    let requests = ctx.server.received_requests().await.unwrap();
    let api: Vec<_> = requests
        .iter()
        .filter(|request| request.url.path().starts_with("/api/"))
        .collect();
    assert!(api.is_empty(), "o bootstrap chamou a API: {api:?}");
    assert!(
        requests
            .iter()
            .any(|request| request.url.path() == format!("{RAW_PREFIX}/pack.toml")),
        "o pack veio do raw simulado"
    );
}

/// CA-T13-14: a versão 1.1 foi publicada, mas o `raw` ainda entrega a 1.0 (cache de até 5
/// minutos): aviso de versão anterior. Cada teste recria a instância do zero e a apaga no fim.
#[tokio::test(flavor = "multi_thread")]
async fn pack_toml_antigo_vira_aviso_de_versao_anterior() {
    let Some(ctx) = Published::start().await else {
        return;
    };
    let tmp = ctx.dir.path().join("tmp");
    let first = CleanInstance::create(&tmp, "teste1").unwrap();
    std::fs::create_dir_all(first.dirs().game_dir.join("saves/Mundo")).unwrap();
    let local = ctx.dir.path().join("pack-local");
    let v2 = ctx
        .pack
        .clone()
        .with_mod(TestMod::new("novo").side(TestSide::Client));
    let pack_v2_toml = write_pack(&v2, &local, &ctx.server);
    let clean = CleanInstance::create(&tmp, "teste2").unwrap();
    assert!(
        !first.dirs().game_dir.exists(),
        "a instância anterior foi apagada; a nova começa do zero"
    );
    let run = command(
        &ctx.externals,
        ctx.jars.clone(),
        ctx.link.clone(),
        &clean,
        Some(ctx.update_url()),
    )
    .run(&CancellationToken::new(), &|_| {})
    .await
    .unwrap();
    assert!(run.success, "{:#?}", run.lines);
    let game = &clean.dirs().game_dir;
    assert!(!game.join("saves").exists());
    let report = compare(&CompareInput {
        pack_root: &local,
        game_dir: game,
        received: &Received::read(game).unwrap(),
        blocked: &[],
        source: ctx.source("1.1.0"),
        received_version: Some("1.0.0".into()),
        published_sha256: Some(hash_bytes(HashFormat::Sha256, &pack_v2_toml)),
    })
    .unwrap();
    assert!(report.outdated, "o GitHub ainda entrega a versão anterior");
    assert!(
        report
            .items
            .iter()
            .any(|item| item.kind == PlayerItemKind::NotPublished
                && item.path == "mods/novo-1.0.jar"),
        "{:?}",
        report.items
    );
    let root = game.parent().unwrap().to_path_buf();
    clean.remove().unwrap();
    assert!(!root.exists(), "a instância temporária é apagada no fim");
}

/// Pack ainda não publicado: o Warden serve a pasta exportada só em `127.0.0.1` e o bootstrap
/// instala dela.
#[tokio::test(flavor = "multi_thread")]
async fn pack_nao_publicado_vem_do_servidor_local() {
    let Some(externals) = externals() else {
        return;
    };
    let dir = temp_dir();
    let files = MockServer::start().await;
    serve_jars(&externals, &files).await;
    let pack = pack_v1();
    pack.serve(&files).await;
    let exported = dir.path().join("exportado");
    write_pack(&pack, &exported, &files);
    let server = LocalPackServer::start(exported.clone()).await.unwrap();
    let link = server.link("pack.toml").unwrap();
    assert_eq!(link.host_str(), Some("127.0.0.1"));

    let jars = jars(&dir.path().join("cache-installer"), &files).await;
    let clean = CleanInstance::create(&dir.path().join("tmp"), "local").unwrap();
    let run = command(&externals, jars, link, &clean, None)
        .run(&CancellationToken::new(), &|_| {})
        .await
        .unwrap();
    assert!(run.success, "{:#?}", run.lines);
    assert!(
        server.requests() >= 3,
        "pack.toml, index.toml e os metafiles"
    );
    let received = Received::read(&clean.dirs().game_dir).unwrap();
    let report = compare(&CompareInput {
        pack_root: &exported,
        game_dir: &clean.dirs().game_dir,
        received: &received,
        blocked: &[],
        source: PlayerSource::Local,
        received_version: Some("1.0.0".into()),
        published_sha256: None,
    })
    .unwrap();
    assert!(!report.outdated);
    assert_eq!(report.count(PlayerItemKind::Optional), 1);
}

/// O link quebrado (404) faz o installer falhar, com o motivo numa frase, sem caminhos.
#[tokio::test(flavor = "multi_thread")]
async fn link_quebrado_falha_com_motivo() {
    let Some(externals) = externals() else {
        return;
    };
    let dir = temp_dir();
    let server = MockServer::start().await;
    serve_jars(&externals, &server).await;
    let jars = jars(&dir.path().join("cache-installer"), &server).await;
    let clean = CleanInstance::create(&dir.path().join("tmp"), "quebrado").unwrap();
    let link = Url::parse(&format!("{}{RAW_PREFIX}/pack.toml", server.uri())).unwrap();
    let run = command(&externals, jars, link, &clean, None)
        .run(&CancellationToken::new(), &|_| {})
        .await
        .unwrap();
    assert!(!run.success);
    let reason =
        warden_instance::player_install::failure_reason(&run.lines, &clean.dirs().game_dir);
    assert!(!reason.is_empty(), "{:#?}", run.lines);
    let shown = clean.dirs().game_dir.to_string_lossy().replace('\\', "/");
    assert!(!reason.contains(&shown), "{reason}");
}

/// Entity Culling (distribuição bloqueada; ver `warden-curseforge/tests/rede_curseforge.rs`).
const ENTITY_CULLING: u64 = 448_233;

/// CA-T13-14, com a API real da CurseForge (a chave embutida no installer): um mod bloqueado
/// para apps de terceiros falha como falharia para o jogador e vira resultado ("não baixa
/// sozinho"), com a página do download manual; sem o arquivo no cache, ele espera os
/// downloads manuais da T20.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "rede"]
async fn rede_mod_bloqueado_da_curseforge_vira_resultado() {
    let Some(externals) = externals() else {
        return;
    };
    let dir = temp_dir();
    let server = MockServer::start().await;
    serve_jars(&externals, &server).await;
    let culling = curseforge_mod(&curseforge_client(), "entityculling", ENTITY_CULLING).await;
    let pack = TestPack::default()
        .with_mod(TestMod::new("sodium"))
        .with_mod(culling.clone());
    pack.serve(&server).await;
    let root = dir.path().join("publicado");
    write_pack(&pack, &root, &server);
    serve_raw(&root, &server).await;
    let jars = jars(&dir.path().join("cache-installer"), &server).await;
    let clean = CleanInstance::create(&dir.path().join("tmp"), "bloqueado").unwrap();
    let link = Url::parse(&format!("{}{RAW_PREFIX}/pack.toml", server.uri())).unwrap();
    let mut command = command(&externals, jars, link, &clean, None);
    command.extra_jvm_args.clear();
    let run = command
        .run(&CancellationToken::new(), &|_| {})
        .await
        .unwrap();
    assert!(
        !run.success,
        "sem janela, o installer desiste com um bloqueado"
    );
    let game = &clean.dirs().game_dir;
    let notices = blocked_notices(&run.lines, game);
    let expected = format!("mods/{}", culling.filename);
    assert_eq!(notices.len(), 1, "{:#?}", run.lines);
    assert_eq!(notices[0].path, expected);
    assert!(
        notices[0]
            .page_url
            .as_deref()
            .is_some_and(|url| url.starts_with("https://www.curseforge.com/")),
        "{notices:?}"
    );
    let cache = DownloadCache::open(&dir.path().join("downloads")).unwrap();
    let supplied = supply_blocked(&notices, &root, &cache, game).unwrap();
    assert!(supplied.copied.is_empty());
    assert_eq!(supplied.waiting.len(), 1);
    assert_eq!(supplied.waiting[0].project_id, ENTITY_CULLING);
    let report = compare(&CompareInput {
        pack_root: &root,
        game_dir: game,
        received: &Received::read(game).unwrap(),
        blocked: &notices,
        source: PlayerSource::Local,
        received_version: None,
        published_sha256: None,
    })
    .unwrap();
    assert_eq!(
        report.count(PlayerItemKind::Blocked),
        1,
        "{:?}",
        report.items
    );
}
