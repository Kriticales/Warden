//! Testes da busca do culpado com o servidor local (D-13; CA-T25-06).
//!
//! - as peças da rodada (pasta do servidor, instalação ligada, mundo descartável, EULA,
//!   `server.properties`), sem processo;
//! - a busca inteira pelo app, com o jogo simulado da `warden-launcher` no modo
//!   `entrar-servidor` (carrega e entra no servidor por `--server`/`--port` ou Quick Play) e o
//!   servidor simulado da `warden-server` no modo `culpado` (o jogador não consegue entrar
//!   quando os jars dados estão em `mods/` do servidor da rodada).
//!
//! Os testes com processo usam o Java de `WARDEN_TEST_JAVA` ou `JAVA_HOME`; sem Java, avisam e
//! passam (como os da D-12).

#![allow(clippy::print_stderr)] // aviso de teste pulado

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::io::Write as _;
use std::net::{Ipv4Addr, SocketAddrV4, TcpListener};
use std::time::{Duration, Instant};

use warden_bisect::{BisectState, SearchOutcome, Symptom};
use warden_packwiz::{IndexEntry, PackIndex, PackManifest};

use super::*;
use crate::bisect_session::tests::{
    fake_state, free_port, register, sha256, spawn, tree_hash, wait_until, write_user_world,
};
use crate::bisect_session::{self, BisectRequest, BisectStatus, RunMode};

/// O mundo de teste modelo do pack, gerado sem mods pela busca (D-12: `bisect_session::world`).
fn world_model(state: &AppState, pack: PackId) -> PathBuf {
    InstanceDirs::for_pack(&state.paths, pack)
        .state_dir
        .join("bisect-world")
        .join("world")
}

fn port_is_free(port: u16) -> bool {
    TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port)).is_ok()
}

/// Um jar Forge mínimo (`META-INF/mods.toml` com o id e as dependências obrigatórias).
fn forge_jar(id: &str, depends: &[&str]) -> Vec<u8> {
    let mut toml = format!(
        "modLoader=\"javafml\"\nloaderVersion=\"[36,)\"\nlicense=\"MIT\"\n\n[[mods]]\nmodId=\"{id}\"\nversion=\"1.0\"\ndisplayName=\"{}\"\n",
        id.to_uppercase()
    );
    for dep in depends {
        let _ = write!(
            toml,
            "\n[[dependencies.{id}]]\nmodId=\"{dep}\"\nmandatory=true\nversionRange=\"[1,)\"\nordering=\"NONE\"\nside=\"BOTH\"\n"
        );
    }
    let mut buffer = std::io::Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut buffer);
        zip.start_file(
            "META-INF/mods.toml",
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
        zip.write_all(toml.as_bytes()).unwrap();
        zip.finish().unwrap();
    }
    buffer.into_inner()
}

/// Um pack Forge 1.16.5 (sem Quick Play para um mundo) com cinco mods locais (`delta` exige
/// `alfa`) e um `server.properties` que pediria outro mundo; o servidor simulado barra a entrada
/// com os jars `culprits`.
fn write_pack(root: &Path, minecraft: &str, loader: (&str, &str), culprits: &str) {
    std::fs::create_dir_all(root.join("mods")).unwrap();
    let mut index = PackIndex::default();
    let mut add = |path: &str, bytes: &[u8]| {
        let file = root.join(path);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, bytes).unwrap();
        index.files.push(IndexEntry {
            file: path.to_owned(),
            hash: sha256(bytes),
            hash_format: String::new(),
            alias: String::new(),
            metafile: false,
            preserve: false,
        });
    };
    for (id, depends) in [
        ("alfa", &[][..]),
        ("bravo", &[][..]),
        ("charlie", &[][..]),
        ("delta", &["alfa"][..]),
        ("echo", &[][..]),
    ] {
        add(&format!("mods/{id}.jar"), &forge_jar(id, depends));
    }
    add(
        "server.properties",
        b"level-name=../mundo-do-pack\r\ndifficulty=hard\r\n",
    );
    let index_text = index.to_toml_string();
    std::fs::write(root.join("index.toml"), &index_text).unwrap();
    let mut manifest = PackManifest::new("Vale do Servidor", minecraft);
    manifest.version = "1.0.0".into();
    manifest.index.hash = sha256(index_text.as_bytes());
    manifest
        .versions
        .get_or_insert_with(BTreeMap::new)
        .insert(loader.0.into(), loader.1.into());
    std::fs::write(root.join("pack.toml"), manifest.to_toml_string()).unwrap();
    std::fs::write(root.join(".warden-fake-game"), "entrar-servidor 300").unwrap();
    std::fs::write(
        root.join(crate::server_session::FAKE_ARGS_FILE),
        format!("culpado {culprits}"),
    )
    .unwrap();
}

fn start(symptom: Symptom, server: bool) -> RunMode {
    RunMode::Start(BisectRequest {
        symptom,
        origin_crash: None,
        suspects: Vec::new(),
        server,
    })
}

/// As portas que o servidor simulado abriu (do log de cada rodada).
fn ports_in(log: &str) -> Vec<u16> {
    log.lines()
        .filter_map(|line| line.split("Starting Minecraft server on 127.0.0.1:").nth(1))
        .filter_map(|port| port.trim().parse().ok())
        .collect()
}

#[test]
fn level_name_e_sempre_a_copia_do_mundo() {
    let text = "#x\r\nlevel-name=../outro\r\ndifficulty=hard\r\nlevel-seed=42\r\n";
    let copied = with_world(text);
    assert!(copied.contains("level-name=world\n"));
    assert!(!copied.contains("../outro"));
    assert!(copied.contains("difficulty=hard\n"));
    assert!(copied.contains("level-seed=42\n"));
    assert_eq!(copied.matches("level-name").count(), 1);
}

/// Uma base de servidor de mentira: o manifesto com um mod e um config, e a instalação ao lado.
fn fake_base(root: &Path) -> (ServerDirs, ServerBase) {
    let dirs = ServerDirs::of(root);
    let game = &dirs.base.game_dir;
    for (path, bytes) in [
        ("mods/alfa.jar", &b"alfa"[..]),
        ("mods/bravo.jar", b"bravo"),
        ("mods/fora-do-pack.jar", b"?"),
        ("config/x.toml", b"x = 1"),
        ("server.jar", b"servidor"),
        ("libraries/net/forge/forge.jar", b"lib"),
        ("libraries/net/forge/args.txt", b"--args"),
        ("logs/latest.log", b"velho"),
        ("crash-reports/crash.txt", b"velho"),
        ("world/level.dat", b"velho"),
        ("eula.txt", b"eula=true"),
    ] {
        let file = game.join(path);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(file, bytes).unwrap();
    }
    let mut manifest = warden_instance::Manifest::default();
    for (file, item) in [
        ("mods/alfa.jar", "mods/alfa.jar"),
        ("mods/bravo.jar", "mods/bravo.jar"),
        ("config/x.toml", "config/x.toml"),
    ] {
        manifest.files.insert(
            file.to_owned(),
            warden_instance::ManifestEntry {
                sha256: String::new(),
                size: 0,
                modified_ns: 0,
                index_path: item.to_owned(),
                index_hash: String::new(),
                expected: String::new(),
                origin: warden_instance::EntryOrigin::Pack,
            },
        );
    }
    manifest.save_if_changed(&dirs.base.state_dir).unwrap();
    let base = ServerBase {
        installed: InstalledServer {
            dir: game.clone(),
            spec: warden_launcher::GameSpec::new(
                "1.16.5",
                warden_launcher::LoaderSpec::Forge {
                    version: "36.2.39".into(),
                },
            )
            .unwrap(),
            release_time: "2021-01-14T16:05:32+00:00".into(),
            launch: warden_server::ServerLaunch::Jar {
                jar: "server.jar".into(),
            },
        },
        java: JavaRuntime {
            java: "java".into(),
            launcher: "java".into(),
            major: 8,
        },
        memory_mb: 4096,
    };
    (dirs, base)
}

fn round_spec(root: &Path, world: PathBuf, eula: bool) -> RoundSpec {
    RoundSpec {
        excluded: BTreeSet::from(["mods/bravo.jar".to_owned()]),
        world,
        eula_accepted_at: eula.then(|| "2026-10-18T12:00:00Z".to_owned()),
        first_port: free_port(),
        motd: "Vale do Servidor".into(),
        log: root.join("logs").join("server").join("output.log"),
    }
}

/// CA-T27-02 na busca: sem o aceite da EULA, nada é copiado nem `eula=true` gravado.
#[test]
fn sem_eula_a_rodada_nao_prepara_nada() {
    let dir = tempfile::tempdir().unwrap();
    let (dirs, base) = fake_base(dir.path());
    let error = prepare_round(
        &base,
        &dirs,
        &round_spec(dir.path(), dir.path().join("mundo"), false),
    )
    .unwrap_err();
    assert_eq!(error.code.to_string(), "server.EULA_NOT_ACCEPTED");
    assert!(!dirs.round.exists());
}

/// A pasta do servidor da rodada: os itens ligados, a instalação (sem o mundo, os logs e os
/// crash reports da base), a cópia do mundo de teste, a EULA e o `server.properties`
/// controlado; a cópia do mundo sai com a rodada e a base não muda.
#[test]
fn rodada_do_servidor_com_mundo_descartavel() {
    let dir = tempfile::tempdir().unwrap();
    let (dirs, base) = fake_base(dir.path());
    let world = dir
        .path()
        .join("teste")
        .join("saves")
        .join("Mundo de teste");
    std::fs::create_dir_all(world.join("region")).unwrap();
    std::fs::write(world.join("level.dat"), "nbt").unwrap();
    std::fs::write(world.join("region/r.0.0.mca"), "chunks").unwrap();
    std::fs::write(world.join("session.lock"), "lock").unwrap();
    let base_before = tree_hash(&dirs.base.game_dir);
    let world_before = tree_hash(&world);
    let round = prepare_round(&base, &dirs, &round_spec(dir.path(), world.clone(), true)).unwrap();
    let server = &dirs.round;
    assert!(server.join("mods/alfa.jar").is_file());
    assert!(!server.join("mods/bravo.jar").exists(), "item desligado");
    assert!(
        !server.join("mods/fora-do-pack.jar").exists(),
        "só os itens do pack"
    );
    assert_eq!(
        std::fs::read(server.join("config/x.toml")).unwrap(),
        b"x = 1"
    );
    assert_eq!(
        std::fs::read(server.join("server.jar")).unwrap(),
        b"servidor"
    );
    assert!(server.join("libraries/net/forge/forge.jar").is_file());
    assert!(server.join("libraries/net/forge/args.txt").is_file());
    assert!(!server.join("logs").exists());
    assert!(!server.join("crash-reports").exists());
    assert_eq!(
        std::fs::read(server.join("world/level.dat")).unwrap(),
        b"nbt"
    );
    assert!(server.join("world/region/r.0.0.mca").is_file());
    assert!(!server.join("world/session.lock").exists());
    assert!(
        std::fs::read_to_string(server.join("eula.txt"))
            .unwrap()
            .contains("eula=true")
    );
    let properties = std::fs::read_to_string(server.join("server.properties")).unwrap();
    assert!(properties.contains("server-ip=127.0.0.1"));
    assert!(properties.contains(&format!("server-port={}", round.port())));
    assert!(properties.contains("online-mode=false"));
    assert!(properties.contains("level-name=world"));
    assert_eq!(round.command.cwd, *server);
    assert!(round.command.args.iter().any(|arg| arg == "server.jar"));
    assert!(round.command.args.iter().any(|arg| arg == "-Xmx4096M"));
    // Os jars da instalação são links físicos: regravar o config da rodada não mexe na base.
    std::fs::write(server.join("libraries/net/forge/args.txt"), "mudou").unwrap();
    drop(round);
    assert!(
        !server.join("world").exists(),
        "a cópia do mundo sai com a rodada"
    );
    assert_eq!(tree_hash(&dirs.base.game_dir), base_before);
    assert_eq!(tree_hash(&world), world_before);
}

/// CA-T25-06: num pack Forge 1.16.5 com o problema "trava ao entrar no mundo", com o aceite,
/// cada rodada sobe o servidor em `127.0.0.1` com uma cópia descartável do mundo, o jogo entra
/// nele e o servidor fecha ao fim da rodada; o veredito usa o log do servidor e acha o mod.
#[tokio::test(flavor = "multi_thread")]
async fn ca_t25_06_acha_o_culpado_pelo_servidor_local() {
    let dir = tempfile::tempdir().unwrap();
    let Some(state) = fake_state(dir.path()) else {
        return;
    };
    let root = dir.path().join("pack");
    write_pack(&root, "1.16.5", ("forge", "36.2.39"), "charlie");
    let pack = register(&state, &root);
    let user_world = write_user_world(&state, pack);
    let test_dirs = InstanceDirs::for_pack(&state.paths, pack);
    let before = (tree_hash(&root), tree_hash(&user_world));
    let (task, seen) = spawn(&state, pack, start(Symptom::CrashOnWorld, true));
    let view = task.await.unwrap().unwrap();
    // O servidor da rodada abre uma cópia do mundo de teste da D-12 (gerado sem mods, com a
    // semente fixa), nunca o mundo do usuário.
    let model = world_model(&state, pack);
    let model_before = tree_hash(&model);
    assert_eq!(
        std::fs::read_to_string(model.join("level.dat")).unwrap(),
        "semente=20261004\n"
    );
    let Some(SearchOutcome::Found { culprits, .. }) = &view.outcome else {
        panic!("{:?}", view.outcome);
    };
    assert_eq!(culprits, &["mods/charlie.jar".to_owned()]);
    assert_eq!(view.status, BisectStatus::Finished);
    assert!(view.server);
    assert!(seen.rounds_done() >= 2);
    // Cada rodada: o servidor com a cópia do mundo, o jogo entrou por `--server`/`--port` e o
    // servidor parou no fim; as que travaram, travaram no servidor.
    let saved = BisectState::load(&test_dirs.state_dir).unwrap().unwrap();
    for (round, info) in view.rounds.iter().zip(&saved.rounds) {
        assert!(
            round.has_server_log && round.has_log,
            "rodada {}",
            round.number
        );
        let server_log = std::fs::read_to_string(
            test_dirs
                .state_dir
                .join(info.server_log.as_deref().unwrap()),
        )
        .unwrap();
        assert!(server_log.contains("Preparing level \"world\" (mundo existente)"));
        assert!(server_log.contains("Done ("));
        assert!(
            server_log.contains("Stopping the server"),
            "rodada {}",
            round.number
        );
        for port in ports_in(&server_log) {
            assert!(port_is_free(port));
        }
        let game_log =
            std::fs::read_to_string(test_dirs.state_dir.join(info.log.as_deref().unwrap()))
                .unwrap();
        assert!(game_log.contains("Connecting to 127.0.0.1"));
        // Antes de 1.20, o jogo entra no servidor por `--server`/`--port`.
        assert!(
            game_log.contains("entra pelo --server"),
            "rodada {}",
            round.number
        );
        let failed = matches!(round.verdict, warden_bisect::Verdict::FailedSame);
        assert_eq!(round.server_failed, failed, "rodada {}", round.number);
    }
    // "Ver log": o do jogo e o do servidor, com a origem.
    let log = bisect_session::round_log(&state.paths, pack, 0).unwrap();
    assert!(log.iter().any(|line| {
        line.origin == crate::test_session::ConsoleOrigin::Server
            && line.text.contains("Couldn't place player in world")
    }));
    assert!(log.iter().any(|line| {
        line.origin == crate::test_session::ConsoleOrigin::Game
            && line.text.contains("Connecting to")
    }));
    // Nada do servidor fica aberto nem no disco; o pack, o mundo de teste e a pasta do "Testar
    // como servidor" não mudam.
    assert!(state.servers.current().is_none());
    let server_dirs = ServerDirs::of(&state.paths.instance_dir(pack));
    assert!(!server_dirs.round.exists());
    assert!(!server_dirs.base.game_dir.exists());
    assert!(
        !crate::server_session::server_dirs(&state.paths, pack)
            .game_dir
            .exists()
    );
    assert_eq!((tree_hash(&root), tree_hash(&user_world)), before);
    assert_eq!(tree_hash(&model), model_before);
}

/// Problema de servidor numa versão com Quick Play (Forge 1.20.1): com o aceite, a busca usa o
/// servidor local mesmo assim, o jogo entra por `--quickPlayMultiplayer` e o servidor abre a
/// cópia do mundo de teste.
#[tokio::test(flavor = "multi_thread")]
async fn problema_de_servidor_com_quick_play() {
    let dir = tempfile::tempdir().unwrap();
    let Some(state) = fake_state(dir.path()) else {
        return;
    };
    let root = dir.path().join("pack");
    write_pack(&root, "1.20.1", ("forge", "47.3.0"), "bravo echo");
    let pack = register(&state, &root);
    let (task, _seen) = spawn(&state, pack, start(Symptom::CrashOnWorld, true));
    let view = task.await.unwrap().unwrap();
    let Some(SearchOutcome::Found { culprits, .. }) = &view.outcome else {
        panic!("{:?}", view.outcome);
    };
    let found: BTreeSet<&str> = culprits.iter().map(String::as_str).collect();
    assert_eq!(found, BTreeSet::from(["mods/bravo.jar", "mods/echo.jar"]));
    let log = bisect_session::round_log(&state.paths, pack, 0).unwrap();
    assert!(
        log.iter()
            .any(|line| line.text.contains("entra pelo --quickPlayMultiplayer"))
    );
    assert!(log.iter().any(|line| {
        line.text
            .contains("Preparing level \"world\" (mundo existente)")
    }));
    assert!(state.servers.current().is_none());
    assert!(
        !ServerDirs::of(&state.paths.instance_dir(pack))
            .round
            .exists()
    );
}

/// Sem o aceite, "trava ao entrar no mundo" numa versão sem Quick Play continua pedindo o
/// servidor (`NEEDS_SERVER`); com o aceite e sem a EULA, nada abre (`EULA_NOT_ACCEPTED`).
#[tokio::test(flavor = "multi_thread")]
async fn sem_aceite_ou_sem_eula_nada_abre() {
    let dir = tempfile::tempdir().unwrap();
    let Some(state) = fake_state(dir.path()) else {
        return;
    };
    let root = dir.path().join("pack");
    write_pack(&root, "1.16.5", ("forge", "36.2.39"), "charlie");
    let pack = register(&state, &root);
    let (task, _) = spawn(&state, pack, start(Symptom::CrashOnWorld, false));
    let error = task.await.unwrap().unwrap_err();
    assert_eq!(error.code.to_string(), "bisect.NEEDS_SERVER");
    state.settings.set_eula_accepted_at(None).unwrap();
    let (task, _) = spawn(&state, pack, start(Symptom::CrashOnWorld, true));
    let error = task.await.unwrap().unwrap_err();
    assert_eq!(error.code.to_string(), "server.EULA_NOT_ACCEPTED");
    assert!(state.servers.current().is_none());
    let server_dirs = ServerDirs::of(&state.paths.instance_dir(pack));
    assert!(!server_dirs.round.exists());
    assert!(!server_dirs.base.game_dir.exists());
}

/// Cancelar com o servidor e o jogo abertos encerra os dois em até 5 s, libera a porta, apaga a
/// cópia do mundo e as pastas da busca e guarda o resultado parcial (CA-T25-03 com o servidor).
#[tokio::test(flavor = "multi_thread")]
async fn cancelar_com_o_servidor_aberto() {
    let dir = tempfile::tempdir().unwrap();
    let Some(state) = fake_state(dir.path()) else {
        return;
    };
    let root = dir.path().join("pack");
    write_pack(&root, "1.16.5", ("forge", "36.2.39"), "charlie");
    // O jogo nunca entra: a rodada fica aberta até o cancelamento.
    std::fs::write(root.join(".warden-fake-game"), "filhos 1").unwrap();
    let pack = register(&state, &root);
    let user_world = write_user_world(&state, pack);
    let user_world_before = tree_hash(&user_world);
    let (task, _seen) = spawn(&state, pack, start(Symptom::CrashOnWorld, true));
    wait_until("o servidor da rodada pronto", || {
        state
            .servers
            .current()
            .is_some_and(|server| server.phase == ServerPhase::Ready)
    })
    .await;
    let port = state
        .servers
        .current()
        .and_then(|server| server.port)
        .unwrap();
    let server_dirs = ServerDirs::of(&state.paths.instance_dir(pack));
    wait_until("a cópia do mundo no servidor", || {
        server_dirs.round.join("world").join("level.dat").is_file()
    })
    .await;
    let asked = Instant::now();
    bisect_session::cancel(&state.paths, pack).await.unwrap();
    let view = tokio::time::timeout(Duration::from_secs(30), task)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(
        asked.elapsed() < Duration::from_secs(5),
        "{:?}",
        asked.elapsed()
    );
    assert!(matches!(
        view.outcome,
        Some(SearchOutcome::Cancelled { .. })
    ));
    assert!(state.servers.current().is_none());
    assert!(port_is_free(port));
    assert!(!server_dirs.round.exists());
    assert!(!server_dirs.base.game_dir.exists());
    assert_eq!(tree_hash(&user_world), user_world_before);
    // O mundo de teste modelo fica para as próximas buscas.
    assert!(world_model(&state, pack).join("level.dat").is_file());
}

/// Integração sob demanda com um servidor Forge 1.16.5 de verdade (CA-T25-06; ROADMAP D-13,
/// "integração com servidor real"). Rode com `WARDEN_BISECT_FORGE_DIR` (uma pasta com o
/// servidor Forge 1.16.5 já instalado e o marcador da `warden-server`, como a instalação do
/// "Testar como servidor" deixa em `instances/<pack>/server`), `WARDEN_BISECT_WORLD` (um mundo
/// 1.16.5) e `WARDEN_TEST_JAVA` (Java 8):
/// `cargo nextest run -p warden-app --run-ignored only servidor_forge_1_16_5_real`.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "servidor de verdade, sob demanda"]
async fn servidor_forge_1_16_5_real() {
    let installed_dir = PathBuf::from(std::env::var_os("WARDEN_BISECT_FORGE_DIR").unwrap());
    let world = PathBuf::from(std::env::var_os("WARDEN_BISECT_WORLD").unwrap());
    let java = PathBuf::from(std::env::var_os("WARDEN_TEST_JAVA").unwrap());
    let spec = warden_launcher::GameSpec::new(
        "1.16.5",
        warden_launcher::LoaderSpec::Forge {
            version: "36.2.39".into(),
        },
    )
    .unwrap();
    let installed = InstalledServer::existing(&installed_dir, &spec)
        .expect("servidor Forge 1.16.5 instalado (com o marcador da warden-server)");
    let temp = tempfile::tempdir().unwrap();
    let dirs = ServerDirs {
        base: InstanceDirs {
            game_dir: installed_dir.clone(),
            state_dir: temp.path().join("estado"),
        },
        round: temp.path().join("rodada"),
    };
    warden_instance::Manifest::default()
        .save_if_changed(&dirs.base.state_dir)
        .unwrap();
    let base = ServerBase {
        installed,
        java: JavaRuntime {
            java: java.clone(),
            launcher: java,
            major: 8,
        },
        memory_mb: 2048,
    };
    let mut spec = round_spec(temp.path(), world, true);
    spec.excluded.clear();
    let round = prepare_round(&base, &dirs, &spec).unwrap();
    let port = round.port();
    let sessions = Arc::new(ServerSessions::default());
    let mut open = round.open(&sessions, PackId::new()).await.unwrap();
    let ready = tokio::time::timeout(Duration::from_secs(600), async {
        loop {
            match open.next().await {
                ServerSignal::Ready => return true,
                ServerSignal::Exited(_) => return false,
                ServerSignal::Line(_) => {}
            }
        }
    })
    .await
    .unwrap();
    assert!(ready, "o servidor Forge 1.16.5 não chegou ao Done");
    open.close(false).await;
    let log = std::fs::read_to_string(&spec.log).unwrap();
    assert!(log.contains("Done ("));
    assert!(sessions.current().is_none());
    assert!(port_is_free(port));
    assert!(!dirs.round.join("world").exists());
}
