//! Testes da busca do culpado pelo app inteiro com o jogo simulado da `warden-launcher` no
//! modo `culpado` (ele falha ao entrar no mundo quando os jars dados estão em `mods/`, e só
//! abre o mundo do Quick Play se `saves/<mundo>/level.dat` existir) e o servidor simulado da
//! `warden-server` (gera o "mundo" com a semente do `server.properties`): mundo de teste
//! gerado e copiado a cada rodada, EULA, materialização da base e de cada rodada, processo,
//! veredito, pausar, continuar, cancelar e o modo assistido, sem baixar Java nem Minecraft.
//!
//! Usam o Java de `WARDEN_TEST_JAVA` ou `JAVA_HOME`; sem Java, avisam e passam (como os testes
//! do Testar).

#![allow(clippy::print_stderr)] // aviso de teste pulado

use std::collections::BTreeMap;
use std::io::Write as _;
use std::net::{Ipv4Addr, SocketAddrV4, TcpListener};
use std::sync::Mutex as StdMutex;

use sha2::{Digest as _, Sha256};
use warden_packwiz::{IndexEntry, PackIndex, PackManifest};
use warden_project::registry::PackRecord;

use super::*;
use crate::state::tests::test_state;
use crate::test_session::TestSessions;

/// O `java` dos testes.
fn java() -> Option<PathBuf> {
    if let Some(java) = std::env::var_os("WARDEN_TEST_JAVA").filter(|v| !v.is_empty()) {
        return Some(PathBuf::from(java));
    }
    let home = std::env::var_os("JAVA_HOME").filter(|v| !v.is_empty())?;
    let java = Path::new(&home)
        .join("bin")
        .join(if cfg!(windows) { "java.exe" } else { "java" });
    java.is_file().then_some(java)
}

fn class_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../crates/warden-launcher/tests/fixtures/java")
}

/// Pasta do `WardenFakeServer.class` (o servidor vanilla que gera o mundo de teste).
fn server_class_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../crates/warden-server/tests/fixtures/java")
}

/// O estado com o jogo e o servidor simulados e a EULA aceita (`None` sem Java).
fn fake_state(root: &Path) -> Option<Arc<AppState>> {
    let state = fake_state_without_eula(root)?;
    state
        .settings
        .set_eula_accepted_at(Some("2026-10-18T12:00:00Z".into()))
        .unwrap();
    Some(state)
}

/// O mesmo, sem o aceite da EULA.
fn fake_state_without_eula(root: &Path) -> Option<Arc<AppState>> {
    let Some(java) = java() else {
        eprintln!("sem Java (WARDEN_TEST_JAVA ou JAVA_HOME): teste da busca do culpado pulado");
        return None;
    };
    let mut state = test_state(root);
    state.tests = Arc::new(
        TestSessions::with_fake_game(&root.join("shared"), class_dir(), java, 17)
            .with_fake_server(server_class_dir()),
    );
    // Cada teste parte de uma porta dada pelo sistema: dois servidores simulados (testes em
    // paralelo) não disputam a 25565.
    state.servers.set_first_port(free_port());
    Some(Arc::new(state))
}

fn free_port() -> u16 {
    TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0))
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

/// O modo do servidor simulado na próxima geração do mundo (`None`: normal).
fn fake_server_mode(root: &Path, mode: Option<&str>) {
    let file = root.join(crate::server_session::FAKE_ARGS_FILE);
    match mode {
        Some(mode) => std::fs::write(file, mode).unwrap(),
        None => {
            let _ = std::fs::remove_file(file);
        }
    }
}

/// A linha que o jogo simulado escreve ao abrir o mundo de teste gerado com a semente fixa.
fn seeded_world_line() -> String {
    format!("Mundo de teste: semente={}", world::WORLD_SEED)
}

fn sha256(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

/// Um jar Fabric mínimo (`fabric.mod.json` com o id e as dependências).
fn fabric_jar(id: &str, depends: &[&str]) -> Vec<u8> {
    let depends: Vec<String> = depends
        .iter()
        .map(|dep| format!("\"{dep}\": \"*\""))
        .collect();
    let json = format!(
        "{{\"schemaVersion\":1,\"id\":\"{id}\",\"version\":\"1.0\",\"name\":\"{}\",\"depends\":{{{}}}}}",
        id.to_uppercase(),
        depends.join(",")
    );
    let mut buffer = std::io::Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut buffer);
        zip.start_file("fabric.mod.json", zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(json.as_bytes()).unwrap();
        zip.finish().unwrap();
    }
    buffer.into_inner()
}

/// Um pack Fabric 1.20.1 com seis mods locais (`delta` exige `alfa`) e o jogo simulado no
/// modo `culpado` com os jars dados.
fn write_pack(root: &Path, culprits: &str) {
    std::fs::create_dir_all(root.join("mods")).unwrap();
    let mut index = PackIndex::default();
    for (id, depends) in [
        ("alfa", &[][..]),
        ("bravo", &[][..]),
        ("charlie", &[][..]),
        ("delta", &["alfa"][..]),
        ("echo", &[][..]),
        ("foxtrot", &[][..]),
    ] {
        let bytes = fabric_jar(id, depends);
        let path = format!("mods/{id}.jar");
        std::fs::write(root.join(&path), &bytes).unwrap();
        index.files.push(IndexEntry {
            file: path,
            hash: sha256(&bytes),
            hash_format: String::new(),
            alias: String::new(),
            metafile: false,
            preserve: false,
        });
    }
    let config = b"x = 1\n";
    std::fs::create_dir_all(root.join("config")).unwrap();
    std::fs::write(root.join("config/jogo.toml"), config).unwrap();
    index.files.push(IndexEntry {
        file: "config/jogo.toml".into(),
        hash: sha256(config),
        hash_format: String::new(),
        alias: String::new(),
        metafile: false,
        preserve: false,
    });
    let index_text = index.to_toml_string();
    std::fs::write(root.join("index.toml"), &index_text).unwrap();
    let mut manifest = PackManifest::new("Vale do Culpado", "1.20.1");
    manifest.version = "1.0.0".into();
    manifest.index.hash = sha256(index_text.as_bytes());
    manifest
        .versions
        .get_or_insert_with(BTreeMap::new)
        .insert("fabric".into(), "0.16.10".into());
    std::fs::write(root.join("pack.toml"), manifest.to_toml_string()).unwrap();
    std::fs::write(
        root.join(".warden-fake-game"),
        format!("culpado 300 {culprits}"),
    )
    .unwrap();
}

fn register(state: &AppState, root: &Path) -> PackId {
    let record = PackRecord {
        id: PackId::new(),
        name: "Vale do Culpado".into(),
        path: root.to_path_buf(),
        last_test: None,
        extra: BTreeMap::new(),
    };
    let id = record.id;
    state.packs.insert(record).unwrap();
    id
}

/// Um mundo do usuário na instância de teste (salvo com todos os mods: a busca nunca o usa).
fn write_user_world(state: &AppState, pack: PackId) -> PathBuf {
    let world = InstanceDirs::for_pack(&state.paths, pack)
        .game_dir
        .join("saves")
        .join("Mundo de teste");
    std::fs::create_dir_all(world.join("region")).unwrap();
    std::fs::write(world.join("level.dat"), "semente=mundo do usuário").unwrap();
    std::fs::write(world.join("region/r.0.0.mca"), "chunks").unwrap();
    world
}

/// Hash de todos os arquivos de uma pasta (caminho e conteúdo).
fn tree_hash(dir: &Path) -> String {
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(next) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&next) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                let relative = path
                    .strip_prefix(dir)
                    .unwrap()
                    .to_string_lossy()
                    .into_owned();
                files.push((relative, std::fs::read(&path).unwrap()));
            }
        }
    }
    files.sort();
    let mut hasher = Sha256::new();
    for (path, bytes) in files {
        hasher.update(path.as_bytes());
        hasher.update(&bytes);
    }
    hex::encode(hasher.finalize())
}

#[derive(Default)]
struct Seen {
    events: StdMutex<Vec<OperationEvent>>,
    phases: StdMutex<Vec<GamePhase>>,
}

impl Seen {
    fn rounds_done(&self) -> usize {
        self.events
            .lock()
            .unwrap()
            .iter()
            .filter(|event| {
                matches!(
                    event,
                    OperationEvent::Round {
                        result: Some(_),
                        ..
                    }
                )
            })
            .count()
    }

    fn asking(&self) -> usize {
        self.events
            .lock()
            .unwrap()
            .iter()
            .filter(|event| matches!(event, OperationEvent::Round { asking: true, .. }))
            .count()
    }
}

fn spawn(
    state: &Arc<AppState>,
    pack: PackId,
    mode: RunMode,
) -> (
    tokio::task::JoinHandle<Result<BisectView, AppError>>,
    Arc<Seen>,
) {
    let seen = Arc::new(Seen::default());
    let events = Arc::clone(&seen);
    let phases = Arc::clone(&seen);
    let sink = BisectSink {
        events: Arc::new(move |event| events.events.lock().unwrap().push(event)),
        game_state: Arc::new(move |game: &GameState| {
            phases.phases.lock().unwrap().push(game.state);
        }),
    };
    let state = Arc::clone(state);
    let task = tokio::spawn(async move { run(&state, pack, mode, sink).await });
    (task, seen)
}

async fn wait_until(what: &str, mut ready: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(60);
    while !ready() {
        assert!(Instant::now() < deadline, "esperando: {what}");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

fn start(symptom: Symptom) -> RunMode {
    RunMode::Start(BisectRequest {
        symptom,
        origin_crash: Some("abc123".into()),
        suspects: Vec::new(),
    })
}

/// Uma busca inteira acha o par e o jogo da rodada nunca fica aberto depois do veredito.
#[tokio::test(flavor = "multi_thread")]
async fn acha_o_par_de_ponta_a_ponta() {
    let dir = tempfile::tempdir().unwrap();
    let Some(state) = fake_state(dir.path()) else {
        return;
    };
    let root = dir.path().join("pack");
    write_pack(&root, "bravo echo");
    let pack = register(&state, &root);
    let (task, seen) = spawn(&state, pack, start(Symptom::CrashOnWorld));
    let view = task.await.unwrap().unwrap();
    let Some(SearchOutcome::Found {
        confidence,
        another_problem,
        ..
    }) = &view.outcome
    else {
        panic!("{:?}", view.outcome);
    };
    let names: BTreeSet<&str> = view
        .outcome_items
        .iter()
        .map(|item| item.name.as_str())
        .collect();
    assert_eq!(names, BTreeSet::from(["bravo.jar", "echo.jar"]));
    assert_eq!(*confidence, warden_bisect::CulpritConfidence::High);
    assert!(!another_problem);
    assert_eq!(view.status, BisectStatus::Finished);
    // game-state: busca do culpado e depois fechado.
    let phases = seen.phases.lock().unwrap().clone();
    assert_eq!(phases.first(), Some(&GamePhase::Bisecting));
    assert_eq!(phases.last(), Some(&GamePhase::Exited));
    assert!(state.tests.current().is_none());
    // As pastas da busca somem no fim; o resultado fica ligado ao travamento de origem.
    let dirs = Dirs::of(&state.paths, pack);
    assert!(!dirs.round_root.exists());
    assert!(!dirs.base.game_dir.exists());
    assert_eq!(
        searches_for(&dirs.state_dir, "abc123"),
        std::slice::from_ref(&view.id)
    );
    // O log de cada rodada.
    assert!(view.rounds.iter().all(|round| round.has_log));
    let log = round_log(&state.paths, pack, 0).unwrap();
    assert!(
        log.iter()
            .any(|line| line.text.contains("Couldn't place player in world"))
    );
}

/// D-12 item 7: a primeira busca gera o mundo de teste sem mods (servidor vanilla da mesma
/// versão, semente fixa), guarda o modelo e cada rodada abre uma cópia dele; a busca seguinte
/// reaproveita o modelo (sem abrir servidor), e mudar a versão do Minecraft gera de novo. O
/// mundo do usuário nunca é usado nem muda.
#[tokio::test(flavor = "multi_thread")]
async fn mundo_de_teste_gerado_uma_vez_e_copiado_a_cada_rodada() {
    let dir = tempfile::tempdir().unwrap();
    let Some(state) = fake_state(dir.path()) else {
        return;
    };
    let root = dir.path().join("pack");
    write_pack(&root, "charlie");
    let pack = register(&state, &root);
    let user_world = write_user_world(&state, pack);
    let user_before = tree_hash(&user_world);
    assert!(!plan(&state, pack).await.unwrap().world_ready);

    let (task, seen) = spawn(&state, pack, start(Symptom::CrashOnWorld));
    let view = task.await.unwrap().unwrap();
    assert!(matches!(view.outcome, Some(SearchOutcome::Found { .. })));
    // A tela foi avisada da geração (o "Criando o mundo de teste" e o fim dela).
    assert!(seen.events.lock().unwrap().iter().any(|event| matches!(
        event,
        OperationEvent::Stage { stage, .. } if stage == STAGE_WORLD
    )));
    // O modelo fica no estado da instância de teste, com o marcador da versão e da semente; a
    // pasta do servidor some e nenhum servidor fica aberto.
    let worlds = world::WorldDirs::of(&state.paths, pack);
    assert!(worlds.ready("1.20.1"));
    let template = std::fs::read_to_string(worlds.world().join("level.dat")).unwrap();
    assert_eq!(template.trim(), format!("semente={}", world::WORLD_SEED));
    assert!(
        !worlds.world().join("session.lock").exists(),
        "o modelo levou o session.lock"
    );
    assert!(
        !state
            .paths
            .instance_dir(pack)
            .join("bisect-world-server")
            .exists(),
        "a pasta do servidor ficou"
    );
    assert!(state.servers.current().is_none());
    assert!(plan(&state, pack).await.unwrap().world_ready);
    // Cada rodada abriu a cópia do mundo gerado (o jogo simulado lê o level.dat da cópia).
    for round in &view.rounds {
        let log = round_log(&state.paths, pack, round.number).unwrap();
        assert!(
            log.iter()
                .any(|line| line.text.contains(&seeded_world_line())),
            "rodada {} sem o mundo de teste gerado",
            round.number
        );
    }
    // As cópias somem com a instância da busca.
    assert!(!Dirs::of(&state.paths, pack).round_root.exists());

    // Segunda busca: o modelo é reaproveitado (um servidor que falha não é nem aberto).
    fake_server_mode(&root, Some("sair 3"));
    let (task, seen) = spawn(&state, pack, start(Symptom::CrashOnWorld));
    let view = task.await.unwrap().unwrap();
    assert!(matches!(view.outcome, Some(SearchOutcome::Found { .. })));
    assert!(!seen.events.lock().unwrap().iter().any(|event| matches!(
        event,
        OperationEvent::Stage { stage, .. } if stage == STAGE_WORLD
    )));

    // Versão nova do Minecraft (modelo de outra versão): a busca tenta gerar de novo.
    let marker = worlds.world().with_file_name(world::MARKER_FILE);
    let text = std::fs::read_to_string(&marker).unwrap();
    std::fs::write(&marker, text.replace("1.20.1", "1.19.4")).unwrap();
    assert!(!plan(&state, pack).await.unwrap().world_ready);
    let (task, _) = spawn(&state, pack, start(Symptom::CrashOnWorld));
    let error = task.await.unwrap().unwrap_err();
    assert_eq!(error.code.to_string(), "bisect.TEST_WORLD_FAILED");
    assert_eq!(
        tree_hash(&user_world),
        user_before,
        "o mundo do usuário mudou"
    );
}

/// A geração que falha (o servidor fecha antes do "pronto") dá um erro claro, com o motivo nos
/// detalhes técnicos; a busca fica pausada antes da primeira rodada, sem modelo pela metade nem
/// servidor aberto, e "Continuar" tenta de novo. Uma porta ocupada não atrapalha.
#[tokio::test(flavor = "multi_thread")]
async fn geracao_que_falha_pausa_e_continuar_tenta_de_novo() {
    let dir = tempfile::tempdir().unwrap();
    let Some(state) = fake_state(dir.path()) else {
        return;
    };
    let root = dir.path().join("pack");
    write_pack(&root, "charlie");
    let pack = register(&state, &root);
    fake_server_mode(&root, Some("sair 3"));
    let (task, _) = spawn(&state, pack, start(Symptom::CrashOnWorld));
    let error = task.await.unwrap().unwrap_err();
    assert_eq!(error.code.to_string(), "bisect.TEST_WORLD_FAILED");
    assert_eq!(error.params["minecraft"], "1.20.1");
    let detail = error.detail.clone().unwrap();
    assert!(detail.contains("server.EXITED_BEFORE_READY"), "{detail}");
    assert!(
        !detail.contains(&dir.path().display().to_string()),
        "{detail}"
    );
    let worlds = world::WorldDirs::of(&state.paths, pack);
    assert!(!worlds.ready("1.20.1"));
    assert!(
        !state
            .paths
            .instance_dir(pack)
            .join("bisect-world-server")
            .exists()
    );
    assert!(state.servers.current().is_none());
    assert!(state.tests.current().is_none());
    let saved = get(&state.paths, pack).unwrap().unwrap();
    assert_eq!(saved.status, BisectStatus::Paused);
    assert!(saved.rounds.is_empty());

    // O servidor volta a funcionar, com a primeira porta tentada ocupada.
    fake_server_mode(&root, None);
    let busy = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0)).unwrap();
    state
        .servers
        .set_first_port(busy.local_addr().unwrap().port());
    let (task, _) = spawn(&state, pack, RunMode::Resume);
    let view = task.await.unwrap().unwrap();
    let Some(SearchOutcome::Found { culprits, .. }) = &view.outcome else {
        panic!("{:?}", view.outcome);
    };
    assert_eq!(culprits, &["mods/charlie.jar".to_owned()]);
    assert!(worlds.ready("1.20.1"));
}

/// Cancelar enquanto o servidor gera o mundo encerra o servidor na hora (a porta fica livre),
/// não deixa modelo pela metade e guarda a busca cancelada; o tempo esgotado também encerra o
/// servidor e vira o erro da geração.
#[tokio::test(flavor = "multi_thread")]
async fn cancelar_ou_esgotar_o_tempo_durante_a_geracao() {
    let dir = tempfile::tempdir().unwrap();
    let Some(state) = fake_state(dir.path()) else {
        return;
    };
    let root = dir.path().join("pack");
    write_pack(&root, "charlie");
    let pack = register(&state, &root);
    // O servidor simulado demora 60 s para ficar pronto.
    fake_server_mode(&root, Some("lento 60000"));
    let (task, _) = spawn(&state, pack, start(Symptom::CrashOnWorld));
    wait_until("servidor do mundo de teste aberto", || {
        state
            .servers
            .current()
            .is_some_and(|server| server.port.is_some())
            && get(&state.paths, pack)
                .ok()
                .flatten()
                .is_some_and(|view| view.creating_world)
    })
    .await;
    let port = state.servers.current().unwrap().port.unwrap();
    cancel(&state.paths, pack).await.unwrap();
    let view = tokio::time::timeout(Duration::from_secs(5), task)
        .await
        .expect("a busca não parou em 5 s")
        .unwrap()
        .unwrap();
    assert!(matches!(
        view.outcome,
        Some(SearchOutcome::Cancelled { .. })
    ));
    assert!(view.rounds.is_empty());
    assert!(!view.creating_world);
    assert!(state.servers.current().is_none());
    assert!(
        warden_server::properties::port_is_free(port),
        "o servidor ficou aberto"
    );
    assert!(!world::WorldDirs::of(&state.paths, pack).ready("1.20.1"));
    assert!(
        !state
            .paths
            .instance_dir(pack)
            .join("bisect-world-server")
            .exists()
    );

    // Tempo esgotado (10 s nos testes): o servidor é encerrado e a busca pausa com o erro.
    let (task, _) = spawn(&state, pack, start(Symptom::CrashOnWorld));
    let error = tokio::time::timeout(Duration::from_secs(40), task)
        .await
        .expect("o tempo da geração não esgotou")
        .unwrap()
        .unwrap_err();
    assert_eq!(error.code.to_string(), "bisect.TEST_WORLD_FAILED");
    assert!(error.detail.unwrap().contains("server.READY_TIMEOUT"));
    assert!(state.servers.current().is_none());
    assert_eq!(
        get(&state.paths, pack).unwrap().unwrap().status,
        BisectStatus::Paused
    );
}

/// D18: sem o aceite da EULA, nenhum servidor abre e nenhum mundo é gerado: "trava ao entrar
/// no mundo" é recusado com o que fazer, e os outros sintomas vão só até o menu.
#[tokio::test(flavor = "multi_thread")]
async fn sem_eula_nao_gera_o_mundo() {
    let dir = tempfile::tempdir().unwrap();
    let Some(state) = fake_state_without_eula(dir.path()) else {
        return;
    };
    let root = dir.path().join("pack");
    write_pack(&root, "charlie");
    let pack = register(&state, &root);
    let shown = plan(&state, pack).await.unwrap();
    assert!(shown.quick_play);
    assert!(!shown.world_ready);
    let (task, _) = spawn(&state, pack, start(Symptom::CrashOnWorld));
    let error = task.await.unwrap().unwrap_err();
    assert_eq!(error.code.to_string(), "bisect.EULA_NEEDED");
    assert!(get(&state.paths, pack).unwrap().is_none());
    assert!(state.tests.current().is_none());

    // "Trava ao abrir o jogo" roda até o menu, sem mundo (o jogo simulado só trava no mundo).
    let (task, seen) = spawn(&state, pack, start(Symptom::CrashOnOpen));
    let view = task.await.unwrap().unwrap();
    assert_eq!(view.status, BisectStatus::Finished);
    assert!(!seen.events.lock().unwrap().iter().any(|event| matches!(
        event,
        OperationEvent::Stage { stage, .. } if stage == STAGE_WORLD
    )));
    let log = round_log(&state.paths, pack, 0).unwrap();
    assert!(!log.iter().any(|line| line.text.contains("Mundo de teste")));
    assert!(!world::WorldDirs::of(&state.paths, pack).ready("1.20.1"));
    assert!(
        !state
            .paths
            .instance_dir(pack)
            .join("bisect-world-server")
            .exists()
    );
    // Nenhum eula=true em lugar nenhum.
    let server = world::WorldDirs::of(&state.paths, pack);
    assert!(!server.world().exists());
}

/// CA-T25-03: cancelar no meio encerra o jogo em até 5 s, apaga a instância da busca, guarda o
/// resultado parcial e não altera o pack, a instância de teste nem os mundos (hash antes e
/// depois).
#[tokio::test(flavor = "multi_thread")]
async fn ca_t25_03_cancelar_no_meio() {
    let dir = tempfile::tempdir().unwrap();
    let Some(state) = fake_state(dir.path()) else {
        return;
    };
    let root = dir.path().join("pack");
    write_pack(&root, "charlie");
    let pack = register(&state, &root);
    let world = write_user_world(&state, pack);
    let test_dirs = InstanceDirs::for_pack(&state.paths, pack);
    let before = (
        tree_hash(&root),
        tree_hash(&test_dirs.game_dir),
        tree_hash(&world),
    );

    let (task, seen) = spawn(&state, pack, start(Symptom::CrashOnWorld));
    // Espera duas rodadas terminarem e a terceira abrir o jogo.
    wait_until("duas rodadas", || seen.rounds_done() >= 2).await;
    wait_until("jogo da terceira rodada", || {
        control_of(pack)
            .and_then(|control| control.current.lock().unwrap().clone())
            .is_some_and(|current| current.number == 2)
            && state.tests.current().is_some()
    })
    .await;
    let clock = Instant::now();
    cancel(&state.paths, pack).await.unwrap();
    let view = tokio::time::timeout(Duration::from_secs(5), task)
        .await
        .expect("a busca não parou em 5 s")
        .unwrap()
        .unwrap();
    assert!(clock.elapsed() < Duration::from_secs(5));
    let Some(SearchOutcome::Cancelled { suspects }) = &view.outcome else {
        panic!("{:?}", view.outcome);
    };
    assert!(suspects.contains(&"mods/charlie.jar".to_owned()));
    assert_eq!(view.status, BisectStatus::Finished);
    let dirs = Dirs::of(&state.paths, pack);
    assert!(!dirs.round_root.exists(), "a instância da busca ficou");
    assert!(!dirs.base.game_dir.exists(), "a base da busca ficou");
    assert!(state.tests.current().is_none());
    let after = (
        tree_hash(&root),
        tree_hash(&test_dirs.game_dir),
        tree_hash(&world),
    );
    assert_eq!(
        before, after,
        "o pack, a instância de teste ou o mundo mudaram"
    );
    // Cancelar de novo: já terminou.
    assert!(cancel(&state.paths, pack).await.is_err());
}

/// CA-T25-03: pausar e "reabrir o app" continua da rodada seguinte, a partir do
/// `state/bisect.json`.
#[tokio::test(flavor = "multi_thread")]
async fn ca_t25_03_pausar_e_continuar_da_rodada_seguinte() {
    let dir = tempfile::tempdir().unwrap();
    let Some(state) = fake_state(dir.path()) else {
        return;
    };
    let root = dir.path().join("pack");
    write_pack(&root, "charlie");
    let pack = register(&state, &root);
    let (task, seen) = spawn(&state, pack, start(Symptom::CrashOnWorld));
    wait_until("primeira rodada", || seen.rounds_done() >= 1).await;
    pause(pack).unwrap();
    let paused = task.await.unwrap().unwrap();
    assert_eq!(paused.status, BisectStatus::Paused);
    assert!(paused.outcome.is_none());
    let done = paused.rounds.len();
    assert!(done >= 1);
    assert!(pause(pack).is_err(), "nada rodando para pausar");

    // O app fechou e abriu de novo: só o arquivo de estado sobrou.
    let saved = get(&state.paths, pack).unwrap().unwrap();
    assert_eq!(saved.status, BisectStatus::Paused);
    assert_eq!(saved.rounds.len(), done);
    let (task, _) = spawn(&state, pack, RunMode::Resume);
    let view = task.await.unwrap().unwrap();
    assert_eq!(view.rounds[done].number, u32::try_from(done).unwrap());
    let Some(SearchOutcome::Found { culprits, .. }) = &view.outcome else {
        panic!("{:?}", view.outcome);
    };
    assert_eq!(culprits, &["mods/charlie.jar".to_owned()]);
    // Nenhuma rodada repetida: a rodada 0 só aparece uma vez.
    let reproduce = view
        .rounds
        .iter()
        .filter(|round| matches!(round.kind, RoundKind::Reproduce { .. }))
        .count();
    assert_eq!(reproduce, 1);
    let (task, _) = spawn(&state, pack, RunMode::Resume);
    assert!(task.await.unwrap().is_err(), "busca terminada não continua");
}

/// "Fechar o Warden" (ou "Parar jogo") no meio de uma rodada pausa a busca: o jogo fecha, a
/// rodada interrompida não conta e nenhuma rodada nova abre.
#[tokio::test(flavor = "multi_thread")]
async fn fechar_o_app_no_meio_pausa_sem_contar_a_rodada() {
    let dir = tempfile::tempdir().unwrap();
    let Some(state) = fake_state(dir.path()) else {
        return;
    };
    let root = dir.path().join("pack");
    write_pack(&root, "charlie");
    let pack = register(&state, &root);
    let (task, seen) = spawn(&state, pack, start(Symptom::CrashOnWorld));
    wait_until("primeira rodada", || seen.rounds_done() >= 1).await;
    wait_until("jogo da rodada seguinte", || {
        control_of(pack)
            .and_then(|control| control.current.lock().unwrap().clone())
            .is_some_and(|current| current.number == 1)
            && state.tests.current().is_some()
    })
    .await;
    assert!(
        state.tests.stop_all(Duration::from_secs(10)).await,
        "a busca não parou"
    );
    let view = task.await.unwrap().unwrap();
    assert_eq!(view.status, BisectStatus::Paused);
    assert_eq!(view.rounds.len(), 1, "a rodada interrompida contou");
    assert!(state.tests.current().is_none());
    let saved = get(&state.paths, pack).unwrap().unwrap();
    assert_eq!(saved.status, BisectStatus::Paused);
}

/// Modo assistido pelo app: cada rodada pergunta ao entrar no mundo; a resposta conta.
#[tokio::test(flavor = "multi_thread")]
async fn modo_assistido_pergunta_em_cada_rodada() {
    let dir = tempfile::tempdir().unwrap();
    let Some(state) = fake_state(dir.path()) else {
        return;
    };
    let root = dir.path().join("pack");
    // O jogo simulado não falha sozinho: o "problema" só a pessoa vê.
    write_pack(&root, "nenhum-jar-com-este-nome");
    let pack = register(&state, &root);
    let (task, seen) = spawn(&state, pack, start(Symptom::Assisted));
    let mods = Dirs::of(&state.paths, pack).round.game_dir.join("mods");
    let mut answered = 0;
    while !task.is_finished() {
        if seen.asking() > answered {
            let happened = mods.join("foxtrot.jar").exists();
            answer(
                pack,
                if happened {
                    AssistedAnswer::Happened
                } else {
                    AssistedAnswer::NotHappened
                },
            )
            .unwrap();
            answered += 1;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let view = task.await.unwrap().unwrap();
    let Some(SearchOutcome::Found { culprits, .. }) = &view.outcome else {
        panic!("{:?}", view.outcome);
    };
    assert_eq!(culprits, &["mods/foxtrot.jar".to_owned()]);
    assert!(
        view.rounds
            .iter()
            .all(|round| matches!(round.verdict, Verdict::UserYes | Verdict::UserNo))
    );
    assert!(answer(pack, AssistedAnswer::Happened).is_err());
}

/// O diálogo: quantos mods, a estimativa e se o mundo de teste já foi gerado.
#[tokio::test(flavor = "multi_thread")]
async fn plano_e_recusas() {
    let dir = tempfile::tempdir().unwrap();
    let Some(state) = fake_state(dir.path()) else {
        return;
    };
    let root = dir.path().join("pack");
    write_pack(&root, "charlie");
    let pack = register(&state, &root);
    // Um mundo do usuário na instância de teste não serve de mundo de teste.
    write_user_world(&state, pack);
    let shown = plan(&state, pack).await.unwrap();
    assert_eq!(shown.searched, 6);
    assert!(shown.quick_play);
    assert!(!shown.world_ready);
    assert_eq!(shown.minecraft, "1.20.1");
    assert_eq!(shown.estimate.rounds_min, 3 + 4);
    assert_eq!(search_id(0), "19700101-000000-000");
    assert_eq!(search_id(1_791_158_153_471), "20261004-235553-471");
}

/// O mundo de teste com o servidor vanilla de verdade (baixa o `server.jar` da Mojang e gera o
/// mundo; QUALITY §4.1, "Servidor local real"). Sob demanda:
/// `cargo nextest run -p warden-app --run-ignored only -E 'test(/mundo_de_teste_com_servidor_vanilla_real/)'`,
/// com um Java 17+ em `WARDEN_TEST_JAVA` ou `JAVA_HOME` (`WARDEN_TEST_JAVA_MAJOR`, padrão 21) e
/// a versão em `WARDEN_TEST_MINECRAFT` (padrão 1.20.1).
#[tokio::test(flavor = "multi_thread")]
#[ignore = "servidor"]
async fn mundo_de_teste_com_servidor_vanilla_real() {
    let dir = tempfile::tempdir().unwrap();
    let java = java().expect("Java 17+ em WARDEN_TEST_JAVA ou JAVA_HOME");
    let major = std::env::var("WARDEN_TEST_JAVA_MAJOR")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(21);
    let minecraft = std::env::var("WARDEN_TEST_MINECRAFT").unwrap_or_else(|_| "1.20.1".to_owned());
    let state = test_state(dir.path());
    state
        .settings
        .set_eula_accepted_at(Some("2026-10-18T12:00:00Z".into()))
        .unwrap();
    state.servers.set_first_port(free_port());
    let root = dir.path().join("pack");
    std::fs::create_dir_all(&root).unwrap();
    let pack = PackId::new();
    let handle = state.operations.start(BISECT_RUN, Some(pack), true);
    let runtime = warden_launcher::JavaRuntime {
        java: java.clone(),
        launcher: java,
        major,
    };
    let clock = Instant::now();
    let template = world::ensure(&state, pack, &root, &minecraft, &runtime, &handle)
        .await
        .unwrap();
    eprintln!(
        "mundo de teste do Minecraft {minecraft} gerado em {:?}",
        clock.elapsed()
    );
    // O level.dat de verdade é NBT comprimido com gzip.
    let level = std::fs::read(template.join("level.dat")).unwrap();
    assert_eq!(&level[..2], &[0x1f, 0x8b]);
    assert!(!template.join("session.lock").exists());
    let worlds = world::WorldDirs::of(&state.paths, pack);
    assert!(worlds.ready(&minecraft));
    assert!(
        !state
            .paths
            .instance_dir(pack)
            .join("bisect-world-server")
            .exists()
    );
    assert!(state.servers.current().is_none());
    // A segunda vez reaproveita o modelo, sem servidor.
    let clock = Instant::now();
    world::ensure(&state, pack, &root, &minecraft, &runtime, &handle)
        .await
        .unwrap();
    assert!(clock.elapsed() < Duration::from_secs(1));
}
