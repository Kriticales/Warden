//! A rodada com o servidor local (D-13; CA-T25-06), com os golden logs reais da matriz da L-05
//! (`tests/fixtures/matriz/<loader>-<versão>-{cliente,servidor}.log`, origem no
//! `FIXTURES.md`): nas versões sem Quick Play, o [`RoundWatcher`] com [`Goal::Server`] recebe as
//! linhas dos dois lados (o servidor até o "Done", o jogo e o resto do servidor) e o "entrou no
//! mundo" é o `logged in with entity id` do servidor.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::Path;

use warden_bisect::estimate::STABILITY_MS;
use warden_bisect::watch::FREEZE_FLOOR_MS;
use warden_bisect::{Anchor, Decision, Goal, RoundWatcher, Stage, WatchConfig};
use warden_launcher::log::{LogParser, LogSource};
use warden_perf::Loader;

/// Intervalo entre linhas (os golden logs não têm o tempo do supervisor).
const STEP_MS: u64 = 100;

fn fixture(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/matriz")
        .join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

fn watcher(minecraft: &str, loader: Loader) -> RoundWatcher {
    RoundWatcher::new(WatchConfig {
        minecraft: minecraft.into(),
        loader,
        goal: Goal::Server,
        log_line: None,
        assisted: false,
        freeze_ms: FREEZE_FLOOR_MS,
        max_ms: 3_600_000,
        stability_ms: STABILITY_MS,
    })
}

/// Uma linha de um dos lados, com o lado.
struct Fed {
    decision: Option<Decision>,
    /// O lado da linha que começou a falha (`true` = servidor).
    anchor_on_server: Option<bool>,
    at: u64,
}

/// Passa as linhas (texto cru, lado) pelo leitor de cada lado e pelo watcher, como a D-12 faz
/// com o servidor local.
fn feed(watcher: &mut RoundWatcher, lines: &[(&str, bool)]) -> Fed {
    let mut game = LogParser::new(LogSource::Stdout);
    let mut server = LogParser::new(LogSource::Stdout);
    let mut fed = Fed {
        decision: None,
        anchor_on_server: None,
        at: 0,
    };
    for (index, (raw, on_server)) in lines.iter().enumerate() {
        fed.at = index as u64 * STEP_MS;
        let parser = if *on_server { &mut server } else { &mut game };
        for line in parser.push(raw, fed.at) {
            let settling = watcher.is_settling();
            let found = watcher.observe(&line.text, fed.at);
            if !settling && (watcher.is_settling() || found.is_some()) {
                fed.anchor_on_server.get_or_insert(*on_server);
            }
            if let Some(found) = found {
                fed.decision.get_or_insert(found);
            }
        }
        if let Some(found) = watcher.tick(fed.at) {
            fed.decision.get_or_insert(found);
        }
    }
    fed
}

/// O servidor até o "Done", o jogo inteiro e o resto do servidor.
fn both_sides<'a>(server: &'a str, game: &'a str) -> Vec<(&'a str, bool)> {
    let lines: Vec<&str> = server.lines().collect();
    let done = lines
        .iter()
        .position(|line| line.contains("Done ("))
        .expect("servidor sem o Done");
    let (before, after) = lines.split_at(done + 1);
    before
        .iter()
        .map(|line| (*line, true))
        .chain(game.lines().map(|line| (line, false)))
        .chain(after.iter().map(|line| (*line, true)))
        .collect()
}

/// As faixas da matriz sem Quick Play para um mundo de um jogador (o servidor local é o caminho).
const PRE_QUICK_PLAY: &[(&str, &str, Loader)] = &[
    ("forge", "1.7.10", Loader::Forge),
    ("forge", "1.12.2", Loader::Forge),
    ("forge", "1.16.5", Loader::Forge),
    ("forge", "1.18.2", Loader::Forge),
    ("forge", "1.19.2", Loader::Forge),
    ("fabric", "1.16.5", Loader::Fabric),
    ("fabric", "1.18.2", Loader::Fabric),
    ("fabric", "1.19.2", Loader::Fabric),
];

/// Em cada faixa antes de 1.20, os logs reais dos dois lados dão "passou": o jogo carregou, o
/// servidor viu o jogador entrar, e nada do log do servidor vira falha.
#[test]
fn cada_faixa_passa_com_os_logs_dos_dois_lados() {
    for (loader_name, minecraft, loader) in PRE_QUICK_PLAY {
        let server = fixture(&format!("{loader_name}-{minecraft}-servidor.log"));
        let game = fixture(&format!("{loader_name}-{minecraft}-cliente.log"));
        let mut watcher = watcher(minecraft, *loader);
        let fed = feed(&mut watcher, &both_sides(&server, &game));
        assert!(
            !matches!(fed.decision, Some(Decision::Failed(_))),
            "{loader_name} {minecraft}: {:?}",
            fed.decision
        );
        assert!(
            watcher.world_ms().is_some(),
            "{loader_name} {minecraft}: o servidor não viu a entrada"
        );
        let decision = fed.decision.or_else(|| watcher.tick(fed.at + STABILITY_MS));
        assert!(
            matches!(decision, Some(Decision::Passed { .. })),
            "{loader_name} {minecraft}: {decision:?}"
        );
    }
}

/// Só o jogo (sem o log do servidor) nunca é "entrou no mundo" com o servidor local: o cliente
/// antes de 1.20 não escreve a entrada no mundo de um servidor.
#[test]
fn sem_o_log_do_servidor_nao_entra_no_mundo() {
    let game = fixture("forge-1.16.5-cliente.log");
    let lines: Vec<(&str, bool)> = game.lines().map(|line| (line, false)).collect();
    let mut watcher = watcher("1.16.5", Loader::Forge);
    let fed = feed(&mut watcher, &lines);
    assert!(watcher.loaded_ms().is_some());
    assert!(watcher.world_ms().is_none());
    assert!(fed.decision.is_none(), "{:?}", fed.decision);
}

/// CA-T25-06 com o veredito dos dois lados: o servidor que não consegue pôr o jogador no mundo
/// (Forge 1.16.5) dá "travou" ao entrar no mundo, com a assinatura do log do servidor, e o sinal
/// fica marcado como do servidor.
#[test]
fn falha_ao_entrar_vem_do_log_do_servidor() {
    let server = fixture("forge-1.16.5-servidor.log");
    let game = fixture("forge-1.16.5-cliente.log");
    let mut lines = both_sides(&server, &game);
    // A entrada do jogador vira a falha do servidor de verdade.
    let at = lines
        .iter()
        .position(|(line, on_server)| *on_server && line.contains("logged in with entity id"))
        .unwrap();
    lines.truncate(at);
    lines.push((
        "[12:00:00] [Server thread/ERROR] [minecraft/PlayerList]: Couldn't place player in world",
        true,
    ));
    lines.push((
        "java.lang.IllegalStateException: Ticking entity at 12, 64, -3",
        true,
    ));
    let mut watcher = watcher("1.16.5", Loader::Forge);
    let fed = feed(&mut watcher, &lines);
    let Some(Decision::Failed(failure)) = fed.decision else {
        panic!("{:?}", fed.decision);
    };
    assert_eq!(failure.anchor, Anchor::JoinWorld);
    assert_eq!(failure.stage, Stage::World);
    assert!(
        failure
            .signature
            .contains("IllegalStateException: Ticking entity at #, #, -#"),
        "{}",
        failure.signature
    );
    assert_eq!(fed.anchor_on_server, Some(true));
    assert!(!watcher.is_settling());
}
