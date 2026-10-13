//! "Testar como o jogador recebe" pelo app inteiro (CA-T13-14; ROADMAP L-07): o jogo simulado no
//! lugar do Minecraft, o bootstrap e o packwiz-installer **reais** (do `cargo xtask installer`) e
//! um servidor local que imita o `raw.githubusercontent.com`, a URL de download dos Releases e a
//! API do GitHub (que registra qualquer chamada).
//!
//! Sem Java (`WARDEN_TEST_JAVA` ou `JAVA_HOME`) ou sem os jars, avisam e passam; com
//! `WARDEN_REQUIRE_EXTERNALS=1` (CI), a falta dos jars é falha.

#![allow(clippy::print_stderr)] // aviso de teste pulado

use std::collections::BTreeMap;
use std::fs;
use std::time::{Duration, Instant};

use warden_instance::player_install::{PlayerItemKind, PlayerSource};
use warden_launcher::session::SESSION_FILE;
use warden_project::registry::PackRecord;
use warden_versioning::publish::{PublishRequest, PublishTree};
use warden_versioning::{Identity, InitialPoint, Moment, PackRepo, SaveVersion};
use wiremock::matchers::{method, path, path_regex};
use wiremock::{Mock, MockServer, ResponseTemplate};

use warden_core::PackId;
use warden_launcher::{EngineDirs, PortableMcEngine};

use super::super::tests::{
    Seen, class_dir, java, java_major, spawn_test, wait_running, write_pack,
};
use super::*;
use crate::error::ErrorCode;
use crate::events::{GamePhase, OperationEvent};
use crate::state::AppState;
use crate::state::tests::test_state;
use crate::test_session::fake::FakeGame;
use crate::test_session::{TestMode, TestRequest, TestSessions, session_dir, sessions_of};

/// Proxy HTTPS fechado: nada sai para a internet.
const NO_INTERNET: [&str; 2] = ["-Dhttps.proxyHost=127.0.0.1", "-Dhttps.proxyPort=9"];

/// O repositório simulado.
const REPO: &str = "dono/vale";

/// Os jars do `cargo xtask installer` (`externals.json`).
fn installer_jars() -> Option<(PathBuf, PathBuf)> {
    let dir = std::env::var_os("WARDEN_INSTALLER_CACHE")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            let base = if cfg!(windows) {
                PathBuf::from(std::env::var_os("LOCALAPPDATA")?)
                    .join("Warden-dev")
                    .join("cache")
            } else {
                std::env::var_os("XDG_CACHE_HOME")
                    .filter(|value| !value.is_empty())
                    .map(PathBuf::from)
                    .or_else(|| Some(PathBuf::from(std::env::var_os("HOME")?).join(".cache")))?
                    .join("Warden-dev")
            };
            Some(base.join("installer"))
        })?;
    let found = (
        dir.join(warden_instance::BOOTSTRAP.file_name),
        dir.join(warden_instance::INSTALLER.file_name),
    );
    if found.0.is_file() && found.1.is_file() {
        return Some(found);
    }
    assert!(
        std::env::var_os("WARDEN_REQUIRE_EXTERNALS").is_none_or(|value| value != "1"),
        "WARDEN_REQUIRE_EXTERNALS=1, mas os jars do instalador não estão em {dir:?} (rode \
         `cargo xtask installer`)"
    );
    eprintln!("sem os jars do instalador (cargo xtask installer): teste pulado");
    None
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

/// Um metafile por link, com o jar no servidor simulado.
fn metafile(server: &MockServer, name: &str, file: &str, side: &str, extra: &str) -> String {
    format!(
        "name = \"{name}\"\nfilename = \"{file}\"\nside = \"{side}\"\n\n[download]\nurl = \"{}/files/{file}\"\nhash-format = \"sha256\"\nhash = \"{}\"\n{extra}",
        server.uri(),
        sha256_hex(file.as_bytes())
    )
}

/// O pack, o app e o "GitHub" simulado.
struct Ctx {
    _dir: tempfile::TempDir,
    state: Arc<AppState>,
    server: MockServer,
    root: PathBuf,
    pack: PackId,
}

impl Ctx {
    /// O pack (Sodium, Crash Assistant marcado como ferramenta do jogador, Zoom opcional
    /// desligado e uma config), registrado; `published`: com o link do GitHub e a versão 1.0.0
    /// publicada, servida pelo `raw` simulado.
    async fn start(published: bool) -> Option<Self> {
        let java = java().or_else(|| {
            eprintln!("sem Java (WARDEN_TEST_JAVA ou JAVA_HOME): teste pulado");
            None
        })?;
        let (bootstrap, installer) = installer_jars()?;
        let dir = tempfile::tempdir().unwrap();
        let server = MockServer::start().await;
        Mock::given(path_regex("^/api/"))
            .respond_with(ResponseTemplate::new(403))
            .mount(&server)
            .await;
        for (jar, file) in [
            (warden_instance::BOOTSTRAP, &bootstrap),
            (warden_instance::INSTALLER, &installer),
        ] {
            Mock::given(method("GET"))
                .and(path(format!("/releases/{}", jar.file_name)))
                .respond_with(ResponseTemplate::new(200).set_body_bytes(fs::read(file).unwrap()))
                .mount(&server)
                .await;
        }
        for file in ["sodium.jar", "crash.jar", "zoom.jar"] {
            Mock::given(method("GET"))
                .and(path(format!("/files/{file}")))
                .respond_with(ResponseTemplate::new(200).set_body_bytes(file.as_bytes().to_vec()))
                .mount(&server)
                .await;
        }

        let mut state = test_state(dir.path());
        let engine = PortableMcEngine::new(EngineDirs::under(&dir.path().join("shared")), None);
        let major = java_major(&java);
        let mut sessions = TestSessions::with_engine(
            Arc::new(engine),
            Some(FakeGame::new(class_dir(), java, major)),
        );
        sessions.player.installer_base =
            Some(Url::parse(&format!("{}/releases/", server.uri())).unwrap());
        sessions.player.jvm_args = NO_INTERNET.iter().map(|arg| (*arg).to_owned()).collect();
        sessions.player.raw_base = Some(Url::parse(&format!("{}/raw/", server.uri())).unwrap());
        sessions.player.packwiz = crate::commands::inventory::tests::packwiz_binary()
            .map(|binary| crate::commands::inventory::tests::test_packwiz(&state, binary));
        state.tests = Arc::new(sessions);

        let root = dir.path().join("pack");
        let sodium = metafile(&server, "Sodium", "sodium.jar", "client", "");
        let crash = metafile(&server, "Crash Assistant", "crash.jar", "client", "");
        let zoom = metafile(
            &server,
            "Zoom",
            "zoom.jar",
            "client",
            "\n[option]\noptional = true\ndefault = false\n",
        );
        write_pack(
            &root,
            &[
                ("config/sodium.json", b"{}\n"),
                ("mods/sodium.pw.toml", sodium.as_bytes()),
                ("mods/crash.pw.toml", crash.as_bytes()),
                ("mods/zoom.pw.toml", zoom.as_bytes()),
            ],
            "filhos 1",
        );
        fs::create_dir_all(root.join(".warden")).unwrap();
        fs::write(
            root.join(".warden/project.toml"),
            "[player-tools]\ncrash-assistant = \"mods/crash.pw.toml\"\n",
        )
        .unwrap();
        let mut extra = BTreeMap::new();
        if published {
            let (owner, name) = REPO.split_once('/').unwrap();
            extra.insert(
                "github".to_owned(),
                serde_json::json!({ "owner": owner, "name": name, "private": false }),
            );
        }
        let record = PackRecord {
            id: PackId::new(),
            name: "Vale de Teste".into(),
            path: root.clone(),
            last_test: None,
            extra,
        };
        let pack = record.id;
        state.packs.insert(record).unwrap();
        let ctx = Self {
            _dir: dir,
            state: Arc::new(state),
            server,
            root,
            pack,
        };
        if published {
            ctx.publish();
        }
        Some(ctx)
    }

    /// Os arquivos do pack que vão para o GitHub (sem `.warden/` e sem o arquivo do jogo
    /// simulado).
    fn tree(&self) -> PublishTree {
        let mut tree = PublishTree::new();
        for path in [
            "pack.toml",
            "index.toml",
            "config/sodium.json",
            "mods/sodium.pw.toml",
            "mods/crash.pw.toml",
            "mods/zoom.pw.toml",
        ] {
            tree.insert(path.to_owned(), fs::read(self.root.join(path)).unwrap());
        }
        tree
    }

    /// A versão 1.0.0 salva, final e publicada (a linha de publicação local).
    fn publish(&self) {
        let identity = Identity::for_pack("Autor", None);
        let repo = PackRepo::init(
            &self.root,
            InitialPoint::Created,
            &identity,
            Moment::now_utc(),
        )
        .unwrap();
        fs::write(self.root.join("README.md"), "Vale\n").unwrap();
        repo.save_version(&SaveVersion {
            version: "1.0.0",
            tag_message: "## 1.0.0\n",
            identity: &identity,
            when: Moment::now_utc(),
            mark_final: true,
        })
        .unwrap();
        let tree = self.tree();
        let prepared = repo
            .prepare_publication(&PublishRequest {
                version: "1.0.0",
                tree: &tree,
                notes: "",
                pack_name: "Vale de Teste",
                identity: &identity,
                when: Moment::now_utc(),
            })
            .unwrap();
        repo.record_publication(&prepared).unwrap();
    }

    /// O `raw` simulado entrega `tree`.
    async fn serve_raw(&self, tree: &PublishTree) {
        for (relative, bytes) in tree {
            Mock::given(method("GET"))
                .and(path(format!("/raw/{REPO}/main/{relative}")))
                .respond_with(ResponseTemplate::new(200).set_body_bytes(bytes.clone()))
                .with_priority(1)
                .mount(&self.server)
                .await;
        }
    }

    /// A instância de teste normal do pack, com um mundo e uma config mudada.
    fn work_instance(&self) -> PathBuf {
        let game = InstanceDirs::for_pack(&self.state.paths, self.pack).game_dir;
        fs::create_dir_all(game.join("saves/Mundo")).unwrap();
        fs::write(game.join("saves/Mundo/level.dat"), "mundo").unwrap();
        fs::create_dir_all(game.join("config")).unwrap();
        fs::write(game.join("config/sodium.json"), "{\"local\": true}\n").unwrap();
        game
    }

    /// A instância limpa do teste em andamento.
    fn clean_game_dir(&self) -> Option<PathBuf> {
        fs::read_dir(self.state.paths.tmp_dir())
            .ok()?
            .flatten()
            .find(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(warden_instance::CLEAN_PREFIX)
            })
            .map(|entry| entry.path().join("minecraft"))
    }

    /// Testa como o jogador recebe, espera o jogo abrir, roda `while_open` e para o jogo.
    async fn run(&self, while_open: impl FnOnce(&Path)) -> (TestSessionSummary, Arc<Seen>) {
        let request = TestRequest {
            mode: TestMode::AsPlayer,
            ..TestRequest::default()
        };
        let (task, seen) = spawn_test(&self.state, self.pack, request);
        let deadline = Instant::now() + Duration::from_secs(60);
        while !self
            .state
            .tests
            .current()
            .is_some_and(|game| game.state == GamePhase::Running)
        {
            if task.is_finished() {
                let result = task.await.unwrap();
                panic!("o teste terminou antes de abrir: {result:?}");
            }
            assert!(Instant::now() < deadline, "o jogo não abriu em 60 s");
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        wait_running(&self.state, &seen).await;
        while_open(
            &self
                .clean_game_dir()
                .expect("instância limpa durante o jogo"),
        );
        self.state.tests.stop(self.pack).await.unwrap();
        let summary = tokio::time::timeout(Duration::from_secs(20), task)
            .await
            .expect("o teste não terminou")
            .unwrap()
            .unwrap();
        (summary, seen)
    }

    fn report(&self, summary: &TestSessionSummary) -> PlayerReport {
        let dir = session_dir(&self.state.paths, self.pack, &summary.id).unwrap();
        report_of(&dir.read_record().unwrap()).expect("resultado na sessão")
    }
}

fn warnings(seen: &Seen) -> Vec<AppErrorCode> {
    seen.events
        .lock()
        .unwrap()
        .iter()
        .filter_map(|event| match event {
            OperationEvent::Warning { error } => match error.code {
                ErrorCode::App(code) => Some(code),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

/// CA-T13-14 pelo app: o pack publicado, instalado do zero pelo link com o bootstrap real e sem
/// nenhuma chamada à API do GitHub, numa instância limpa (sem os mundos e as configs da
/// instância de trabalho) que tem o Crash Assistant e é apagada no fim; os registros ficam na
/// sessão.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ca_t13_14_pelo_link_publicado_numa_instancia_limpa() {
    let Some(ctx) = Ctx::start(true).await else {
        return;
    };
    ctx.serve_raw(&ctx.tree()).await;
    let work = ctx.work_instance();
    let (summary, seen) = ctx
        .run(|game| {
            assert!(
                game.join("mods/crash.jar").is_file(),
                "Crash Assistant (D16)"
            );
            assert!(game.join("mods/sodium.jar").is_file());
            assert!(
                !game.join("saves").exists(),
                "sem os mundos da instância de trabalho"
            );
            assert_eq!(
                fs::read_to_string(game.join("config/sodium.json")).unwrap(),
                "{}\n",
                "a config veio do pack, não da instância de trabalho"
            );
            fs::create_dir_all(game.join("logs")).unwrap();
            fs::write(game.join("logs/latest.log"), "linha do jogo\n").unwrap();
        })
        .await;
    assert_eq!(summary.mode, TestMode::AsPlayer);
    assert_eq!(summary.pack_version.as_deref(), Some("1.0.0"));
    assert!(warnings(&seen).is_empty(), "{:?}", warnings(&seen));

    let report = ctx.report(&summary);
    match &report.source {
        PlayerSource::Published { link, version } => {
            assert!(link.ends_with("/raw/dono/vale/main/pack.toml"), "{link}");
            assert_eq!(version.as_deref(), Some("1.0.0"));
        }
        PlayerSource::Local => panic!("devia vir do link publicado"),
    }
    assert!(!report.outdated);
    assert_eq!(report.crash_assistant, Some(true));
    let kinds: Vec<_> = report
        .items
        .iter()
        .map(|item| (item.kind, item.path.as_str(), item.default_on))
        .collect();
    assert_eq!(
        kinds,
        [(PlayerItemKind::Optional, "mods/zoom.jar", Some(false))]
    );

    // A instância temporária sumiu; os registros ficaram na sessão.
    assert!(
        ctx.clean_game_dir().is_none(),
        "a instância limpa é apagada no fim"
    );
    let session = session_dir(&ctx.state.paths, ctx.pack, &summary.id).unwrap();
    assert_eq!(
        fs::read_to_string(
            session
                .path()
                .join(warden_instance::KEPT_DIR)
                .join("logs/latest.log")
        )
        .unwrap(),
        "linha do jogo\n"
    );
    assert!(session.output_log().is_file());
    let json: serde_json::Value =
        serde_json::from_slice(&fs::read(session.path().join(SESSION_FILE)).unwrap()).unwrap();
    assert_eq!(json["mode"], "asPlayer");
    assert!(
        json["packTree"].is_null(),
        "não conta como teste da pasta local"
    );
    assert!(
        json.get("capture").is_none(),
        "sem captura numa instância que some"
    );
    assert_eq!(sessions_of(&ctx.state.paths, ctx.pack).unwrap()[0], summary);

    // A instância de trabalho não mudou.
    assert!(work.join("saves/Mundo/level.dat").is_file());
    assert_eq!(
        fs::read_to_string(work.join("config/sodium.json")).unwrap(),
        "{\"local\": true}\n"
    );

    // Nenhuma chamada à API do GitHub; o pack veio do raw.
    let requests = ctx.server.received_requests().await.unwrap();
    assert!(
        !requests
            .iter()
            .any(|request| request.url.path().starts_with("/api/")),
        "chamou a API do GitHub"
    );
    let raw_pack = format!("/raw/{REPO}/main/pack.toml");
    assert!(
        requests
            .iter()
            .any(|request| request.url.path() == raw_pack)
    );
}

/// CA-T13-14: com o `pack.toml` antigo sendo servido (o cache de até 5 minutos do GitHub), o
/// teste avisa que o GitHub ainda entrega a versão anterior.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ca_t13_14_pack_toml_antigo_avisa_versao_anterior() {
    let Some(ctx) = Ctx::start(true).await else {
        return;
    };
    let mut old = ctx.tree();
    let pack_toml = String::from_utf8(old["pack.toml"].clone()).unwrap();
    assert!(pack_toml.contains("version = \"1.0.0\""), "{pack_toml}");
    old.insert(
        "pack.toml".into(),
        pack_toml
            .replace("version = \"1.0.0\"", "version = \"0.9.0\"")
            .into_bytes(),
    );
    ctx.serve_raw(&old).await;
    let (summary, seen) = ctx.run(|_| {}).await;
    assert!(
        warnings(&seen).contains(&AppErrorCode::TestPlayerOldVersion),
        "{:?}",
        warnings(&seen)
    );
    let report = ctx.report(&summary);
    assert!(report.outdated);
    assert_eq!(report.received_version.as_deref(), Some("0.9.0"));
    assert_eq!(summary.pack_version.as_deref(), Some("0.9.0"));
}

/// Pack não publicado: o Warden exporta o estado atual e serve só em `127.0.0.1`, com o aviso.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn pack_nao_publicado_vem_do_pack_exportado_agora() {
    let Some(ctx) = Ctx::start(false).await else {
        return;
    };
    if ctx.state.tests.player.packwiz.is_none() {
        eprintln!("sem o sidecar do packwiz: teste pulado");
        return;
    }
    let (summary, seen) = ctx
        .run(|game| {
            assert!(game.join("mods/crash.jar").is_file());
        })
        .await;
    assert_eq!(
        warnings(&seen),
        [AppErrorCode::TestPlayerLocalSource],
        "avisa que o pack exportado agora é servido só neste computador"
    );
    let report = ctx.report(&summary);
    assert_eq!(report.source, PlayerSource::Local);
    assert!(!report.outdated);
    assert_eq!(report.crash_assistant, Some(true));
    assert!(
        !ctx.state.paths.cache_dir().join("staging").exists()
            || fs::read_dir(ctx.state.paths.cache_dir().join("staging"))
                .unwrap()
                .next()
                .is_none(),
        "a cópia exportada não fica para trás"
    );
}

/// O link que não responde vira erro claro, antes de baixar o instalador.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn link_publicado_fora_do_ar_vira_erro() {
    let Some(ctx) = Ctx::start(true).await else {
        return;
    };
    let request = TestRequest {
        mode: TestMode::AsPlayer,
        ..TestRequest::default()
    };
    let (task, _) = spawn_test(&ctx.state, ctx.pack, request);
    let error = task.await.unwrap().unwrap_err();
    assert_eq!(
        error.code,
        ErrorCode::App(AppErrorCode::TestPlayerLinkUnreachable)
    );
    assert_eq!(error.params["status"], "404");
    assert!(ctx.clean_game_dir().is_none());
}

#[test]
fn andamento_das_linhas_do_instalador() {
    let progress = installer_progress("(3/12) Downloaded sodium").unwrap();
    assert_eq!(progress, Progress::items(3, Some(12)));
    assert_eq!(installer_progress("Validating existing files..."), None);
    assert_eq!(installer_progress("(a/b) x"), None);
}

#[test]
fn bloqueados_sem_lugar_viram_erro_com_os_nomes() {
    let error = blocked_error(&[
        BlockedNotice {
            path: "mods/culling.jar".into(),
            page_url: None,
        },
        BlockedNotice {
            path: "mods/sub/b.jar".into(),
            page_url: None,
        },
    ]);
    assert_eq!(error.params["count"], "2");
    assert_eq!(error.params["names"], "culling.jar, b.jar");
}
