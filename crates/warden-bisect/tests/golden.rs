//! Golden logs reais do spike S-R5-3 (`tests/fixtures/`, origem no `FIXTURES.md`) passando
//! pelo mesmo leitor da supervisão do jogo (`warden_launcher::log::LogParser`) e pelo
//! [`RoundWatcher`] (CA-T25-05 e CA-T25-07).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeSet;
use std::path::Path;

use warden_bisect::estimate::STABILITY_MS;
use warden_bisect::judge::{PackLookup, Symptom, Target, judge_failure};
use warden_bisect::watch::{FREEZE_FLOOR_MS, quick_play_entered};
use warden_bisect::{Anchor, Decision, Failure, Goal, RoundWatcher, Stage, Verdict, WatchConfig};
use warden_launcher::log::{LogParser, LogSource};
use warden_perf::Loader;

/// Intervalo entre linhas nos testes (os golden logs não têm o tempo do supervisor).
const STEP_MS: u64 = 100;

fn fixture(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

fn config(minecraft: &str, loader: Loader, goal: Goal) -> WatchConfig {
    WatchConfig {
        minecraft: minecraft.into(),
        loader,
        goal,
        log_line: None,
        assisted: false,
        freeze_ms: FREEZE_FLOOR_MS,
        max_ms: 3_600_000,
        stability_ms: STABILITY_MS,
    }
}

/// Passa o log pelo leitor e pelo watcher; devolve a primeira decisão e o tempo da última
/// linha.
fn feed(watcher: &mut RoundWatcher, text: &str) -> (Option<Decision>, u64) {
    let mut parser = LogParser::new(LogSource::Stdout);
    let mut first = None;
    let mut at = 0;
    for (index, raw) in text.lines().enumerate() {
        at = index as u64 * STEP_MS;
        for line in parser.push(raw, at) {
            if let Some(decision) = watcher.observe(&line.text, line.elapsed_ms) {
                first.get_or_insert(decision);
            }
        }
        if let Some(decision) = watcher.tick(at) {
            first.get_or_insert(decision);
        }
    }
    for line in parser.finish(at) {
        if let Some(decision) = watcher.observe(&line.text, line.elapsed_ms) {
            first.get_or_insert(decision);
        }
    }
    (first, at)
}

fn loader_of(name: &str) -> Loader {
    match name {
        "fabric" => Loader::Fabric,
        "quilt" => Loader::Quilt,
        "forge" => Loader::Forge,
        "neoforge" => Loader::NeoForge,
        _ => Loader::Vanilla,
    }
}

fn failure_of(file: &str, minecraft: &str, loader: Loader) -> Failure {
    let mut watcher = RoundWatcher::new(config(minecraft, loader, Goal::World));
    let (decision, at) = feed(&mut watcher, &fixture(file));
    let decision = decision.or_else(|| watcher.tick(at + 5_000));
    match decision {
        Some(Decision::Failed(failure)) => failure,
        other => panic!("{file}: esperava falha, veio {other:?}"),
    }
}

/// CA-T25-05: em cada faixa da matriz, os marcadores de pronto e de mundo do S-R5-3 §3.
#[test]
fn ca_t25_05_marcadores_de_cada_faixa() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/matriz");
    let mut checked = 0;
    for entry in std::fs::read_dir(&dir).unwrap() {
        let name = entry.unwrap().file_name().to_string_lossy().into_owned();
        let Some(combo) = name.strip_suffix("-cliente.log") else {
            continue;
        };
        let (loader, minecraft) = combo.split_once('-').unwrap();
        let loader = loader_of(loader);
        let quick_play = !matches!(
            minecraft,
            "1.7.10" | "1.12.2" | "1.16.5" | "1.18.2" | "1.19.2"
        );
        let goal = if quick_play { Goal::World } else { Goal::Menu };
        let mut watcher = RoundWatcher::new(config(minecraft, loader, goal));
        let (decision, at) = feed(&mut watcher, &fixture(&format!("matriz/{name}")));
        assert!(
            !matches!(decision, Some(Decision::Failed(_))),
            "{name}: {decision:?}"
        );
        assert!(watcher.loaded_ms().is_some(), "{name}: não carregou");
        if quick_play {
            assert!(watcher.world_ms().is_some(), "{name}: não entrou no mundo");
        } else {
            // Antes de 1.20 o mundo é o do servidor local (D-13): o log do servidor tem a
            // entrada do jogador.
            let server = fixture(&format!("matriz/{combo}-servidor.log"));
            let mut on_server = RoundWatcher::new(config(minecraft, loader, Goal::World));
            feed(&mut on_server, &server);
            assert!(
                on_server.world_ms().is_some(),
                "{combo}: servidor sem a entrada"
            );
        }
        let passed = decision.or_else(|| watcher.tick(at + STABILITY_MS));
        assert!(
            matches!(passed, Some(Decision::Passed { .. })),
            "{name}: {passed:?}"
        );
        checked += 1;
    }
    assert_eq!(checked, 20);
}

/// O 1.7.10 cria um atlas `16x16` antes dos mods: só o definitivo vale como pronto; o crash
/// report de mentira do `SplashProgress` não é falha.
#[test]
fn forge_1_7_10_atlas_definitivo_e_falso_crash_report() {
    let mut watcher = RoundWatcher::new(config("1.7.10", Loader::Forge, Goal::Menu));
    let early = "Created: 16x16 textures/blocks-atlas";
    assert!(watcher.observe("Sound engine started", 1).is_none());
    assert!(watcher.observe(early, 2).is_none());
    assert!(watcher.loaded_ms().is_none());
    let (decision, _) = feed(
        &mut watcher,
        &fixture("mecanismos/forge-1.7.10-falso-crash-report-splash.log"),
    );
    assert!(
        !matches!(decision, Some(Decision::Failed(_))),
        "{decision:?}"
    );
}

/// CA-T25-07: a tela de erro de carregamento do NeoForge e do Forge dá "Travou", nunca
/// "Passou", mesmo com os marcadores de menu no log.
#[test]
fn ca_t25_07_tela_de_erro_de_carregamento_e_falha() {
    for (file, minecraft, loader) in [
        ("falhas/neo-dep-faltando.log", "1.21.1", Loader::NeoForge),
        ("falhas/forge-dep-faltando.log", "1.20.1", Loader::Forge),
    ] {
        let mut watcher = RoundWatcher::new(config(minecraft, loader, Goal::Menu));
        let text = fixture(file);
        let (decision, at) = feed(&mut watcher, &text);
        // Mesmo depois de "pronto" (som e atlas), o tempo passando não vira "passou".
        let later = watcher.tick(at + STABILITY_MS * 3);
        let Some(Decision::Failed(failure)) = decision else {
            panic!("{file}: {decision:?}");
        };
        assert!(later.is_none());
        assert_eq!(failure.anchor, Anchor::MissingDependency, "{file}");
        assert_eq!(
            failure.signature, "dependencia | supplementaries>moonlight",
            "{file}"
        );
        // O 401 do perfil offline não entra na assinatura.
        assert!(!failure.signature.contains("401"));
    }
}

/// CA-T25-07: `Couldn't place player in world` é falha sem crash report, decidida na linha
/// (o Warden encerra o jogo, que não fecharia sozinho).
#[test]
fn ca_t25_07_falha_ao_entrar_no_mundo_sem_crash() {
    for (file, minecraft, loader) in [
        ("falhas/neo-entrar.log", "1.21.1", Loader::NeoForge),
        ("falhas/fab-entrar.log", "1.20.1", Loader::Fabric),
        ("falhas/forge-entrar.log", "1.20.1", Loader::Forge),
    ] {
        let failure = failure_of(file, minecraft, loader);
        assert_eq!(failure.anchor, Anchor::JoinWorld, "{file}");
        assert_eq!(failure.stage, Stage::World, "{file}");
        assert_eq!(
            failure.signature,
            "entrar | java.lang.IllegalStateException: Warden: falha de teste ao entrar no mundo (wardenfalhas)",
            "{file}"
        );
    }
}

/// CA-T25-07: NeoForge vivo e calado depois de `Starting integrated minecraft server` por mais
/// que o limite vira "Travou".
#[test]
fn ca_t25_07_jogo_vivo_e_calado() {
    for (file, minecraft, loader) in [
        ("falhas/neo-travar.log", "1.21.1", Loader::NeoForge),
        ("falhas/fab-travar.log", "1.20.1", Loader::Fabric),
        ("falhas/forge-travar.log", "1.20.1", Loader::Forge),
    ] {
        let mut watcher = RoundWatcher::new(config(minecraft, loader, Goal::World));
        let (decision, at) = feed(&mut watcher, &fixture(file));
        assert!(decision.is_none(), "{file}: {decision:?}");
        assert!(watcher.started_world_ms().is_some(), "{file}");
        assert!(watcher.tick(at + FREEZE_FLOOR_MS - 1).is_none(), "{file}");
        let Some(Decision::Failed(failure)) = watcher.tick(at + FREEZE_FLOOR_MS) else {
            panic!("{file}: não travou");
        };
        assert_eq!(failure.anchor, Anchor::Frozen);
        assert_eq!(failure.signature, "travou");
    }
}

struct NoLookup;

impl PackLookup for NoLookup {
    fn item_of_mod(&self, _: &str) -> Option<String> {
        None
    }

    fn item_of_class(&self, _: &str) -> Option<String> {
        None
    }
}

/// CA-T25-07: Java errado vira problema de ambiente, não de mod.
#[test]
fn ca_t25_07_java_errado_e_ambiente() {
    for (file, minecraft, loader) in [
        ("falhas/fab-java8.log", "1.20.1", Loader::Fabric),
        ("falhas/neo-java17.log", "1.21.1", Loader::NeoForge),
        ("falhas/f112-java21.log", "1.12.2", Loader::Forge),
    ] {
        let failure = failure_of(file, minecraft, loader);
        assert_eq!(failure.anchor, Anchor::JavaVersion, "{file}");
        let verdict = judge_failure(
            &failure,
            None,
            &Symptom::CrashOnWorld,
            &mut Target::default(),
            &BTreeSet::new(),
            &NoLookup,
        );
        assert_eq!(verdict, Verdict::Environment, "{file}");
    }
}

/// Os outros sinais da §5.2: mixin (config + alvo), falta de memória, crash report ao abrir,
/// dependência e duplicado do Forge 1.12.2, dependência do Fabric.
#[test]
fn sinais_de_falha_da_tabela_do_spike() {
    for (file, minecraft, loader) in [
        ("falhas/fab-mixin.log", "1.20.1", Loader::Fabric),
        ("falhas/neo-mixin.log", "1.21.1", Loader::NeoForge),
        ("falhas/forge-mixin.log", "1.20.1", Loader::Forge),
    ] {
        let failure = failure_of(file, minecraft, loader);
        assert_eq!(failure.anchor, Anchor::Mixin, "{file}");
        assert!(
            failure.signature.starts_with("mixin | wardenfalhas.")
                && failure.signature.contains(".mixins.json:MixinQuebrado"),
            "{file}: {}",
            failure.signature
        );
    }
    for (file, minecraft, loader) in [
        ("falhas/fab-memoria.log", "1.20.1", Loader::Fabric),
        ("falhas/neo-memoria.log", "1.21.1", Loader::NeoForge),
    ] {
        let failure = failure_of(file, minecraft, loader);
        assert_eq!(failure.anchor, Anchor::OutOfMemory, "{file}");
    }
    for (file, minecraft, loader) in [
        ("falhas/fab-iniciar.log", "1.20.1", Loader::Fabric),
        ("falhas/neo-iniciar.log", "1.21.1", Loader::NeoForge),
    ] {
        let failure = failure_of(file, minecraft, loader);
        assert_eq!(failure.stage, Stage::Opening, "{file}");
    }
    let quark = failure_of("falhas/f112-dep-faltando.log", "1.12.2", Loader::Forge);
    assert_eq!(quark.anchor, Anchor::MissingDependency);
    assert_eq!(quark.signature, "dependencia | quark>autoreglib");
    let duplicate = failure_of("falhas/f112-duplicado.log", "1.12.2", Loader::Forge);
    assert_eq!(duplicate.anchor, Anchor::DuplicateMods);
    let fabric = failure_of(
        "falhas/fab-dep-faltando-nogui.log",
        "1.20.1",
        Loader::Fabric,
    );
    assert_eq!(fabric.anchor, Anchor::MissingDependency);
    assert!(
        fabric.signature.contains("comforts>fabric-api"),
        "{}",
        fabric.signature
    );
    // A versão errada no NeoForge cai na classe que falta, com quem a procurou.
    let version = failure_of("falhas/neo-dep-versao.log", "1.21.1", Loader::NeoForge);
    let missing = &version.missing_classes[0];
    assert!(missing.class.starts_with("net.mehvahdjukaar.moonlight."));
    assert_eq!(missing.needed_by.as_deref(), Some("supplementaries"));
}

/// As rodadas da busca real do spike (§7): a rodada 0 com o par plantado, o "falhou
/// diferente" da dependência sem `type` e a confirmação que passou.
#[test]
fn rodadas_da_busca_real() {
    let r0 = failure_of(
        "busca/r0-pack-inteiro-falhou.log",
        "1.21.1",
        Loader::NeoForge,
    );
    assert_eq!(
        r0.signature,
        "entrar | java.lang.IllegalStateException: Warden: conflito de teste com vectorwing.farmersdelight.FarmersDelight (wardenfalhas)"
    );
    let other = failure_of(
        "busca/tentativa1-falhou-diferente-dependencia.log",
        "1.21.1",
        Loader::NeoForge,
    );
    assert!(
        other
            .hints
            .iter()
            .any(|hint| hint.from == "continuity" && hint.to == "fabric_api")
    );
    let mut target = Target::default();
    let none = BTreeSet::new();
    assert_eq!(
        judge_failure(
            &r0,
            None,
            &Symptom::CrashOnWorld,
            &mut target,
            &none,
            &NoLookup
        ),
        Verdict::FailedSame
    );
    assert_eq!(
        judge_failure(
            &other,
            None,
            &Symptom::CrashOnWorld,
            &mut target,
            &none,
            &NoLookup
        ),
        Verdict::FailedOther
    );
    let mut watcher = RoundWatcher::new(config("1.21.1", Loader::NeoForge, Goal::World));
    let (decision, at) = feed(
        &mut watcher,
        &fixture("busca/c2-pack-sem-o-culpado-passou.log"),
    );
    let decision = decision.or_else(|| watcher.tick(at + STABILITY_MS));
    assert!(
        matches!(decision, Some(Decision::Passed { .. })),
        "{decision:?}"
    );
}

/// O arquivo do `--quickPlayPath` é sinal secundário de "entrou no mundo".
#[test]
fn arquivo_do_quick_play_confirma_o_mundo() {
    for name in [
        "quickplay-path-fabric-1.20.1.json",
        "quickplay-path-neoforge-1.21.1.json",
        "quickplay-path-vanilla-26.3.json",
    ] {
        assert!(
            quick_play_entered(&fixture(&format!("mecanismos/{name}"))),
            "{name}"
        );
    }
    assert!(!quick_play_entered("[]"));
    assert!(!quick_play_entered("não é json"));
    let mut watcher = RoundWatcher::new(config("1.21.1", Loader::NeoForge, Goal::World));
    assert!(watcher.quick_play_file(
        &fixture("mecanismos/quickplay-path-neoforge-1.21.1.json"),
        30_000
    ));
    assert_eq!(watcher.world_ms(), Some(30_000));
    assert!(matches!(
        watcher.tick(50_000),
        Some(Decision::Passed { reached_ms: 30_000 })
    ));
}

/// Modo "aparece uma mensagem no log" e modo assistido.
#[test]
fn linha_do_log_e_modo_assistido() {
    let mut line = config("1.20.1", Loader::Fabric, Goal::World);
    line.log_line = Some("Falhou ao carregar a textura 42".into());
    let mut watcher = RoundWatcher::new(line);
    assert!(watcher.observe("nada", 1).is_none());
    let Some(Decision::Failed(failure)) = watcher.observe(
        "[Render thread/WARN]: Falhou ao carregar a textura 42 de x",
        2,
    ) else {
        panic!("a linha não foi vista");
    };
    assert_eq!(failure.anchor, Anchor::LogLine);
    assert_eq!(failure.signature, "linha | Falhou ao carregar a textura #");

    let mut assisted = config("1.20.1", Loader::Fabric, Goal::World);
    assisted.assisted = true;
    let mut watcher = RoundWatcher::new(assisted);
    assert!(!watcher.take_question());
    watcher.observe(
        "Jogador[local:E:1] logged in with entity id 1 at (0.5, 64.0, 0.5)",
        10,
    );
    assert!(watcher.take_question());
    assert!(!watcher.take_question());
    // Sem resposta, o tempo não decide nada no modo assistido.
    assert!(watcher.tick(10 + STABILITY_MS * 10).is_none());
    // Travou sozinho: decidido sem perguntar.
    let Some(Decision::Failed(failure)) =
        watcher.observe("java.lang.OutOfMemoryError: Java heap space", 20)
    else {
        panic!("a falta de memória não foi vista");
    };
    assert_eq!(failure.anchor, Anchor::OutOfMemory);
    assert_eq!(failure.stage, Stage::World);
}

/// O processo que sai sozinho: com crash report, a assinatura vem dele; sem nada, é "saiu".
#[test]
fn saida_do_processo() {
    let mut watcher = RoundWatcher::new(config("1.20.1", Loader::Fabric, Goal::World));
    let report = "---- Minecraft Crash Report ----\n// x\nDescription: Ticking entity\n\njava.lang.NullPointerException: Cannot invoke \"a\" because \"<local4>\" is null\n\tat com.example.Foo.tick(Foo.java:42)";
    let Decision::Failed(failure) = watcher.exit(Some(-1), Some(report), 9_000) else {
        panic!();
    };
    assert_eq!(failure.anchor, Anchor::CrashReport);
    assert_eq!(
        failure.signature,
        "crash | java.lang.NullPointerException: Cannot invoke \"a\" because \"<local#>\" is null"
    );
    let mut native = RoundWatcher::new(config("1.19.2", Loader::Fabric, Goal::Menu));
    let Decision::Failed(failure) = native.exit(Some(-1_073_741_819), None, 16_200) else {
        panic!();
    };
    assert_eq!(failure.anchor, Anchor::ProcessExit);
    // Tempo-limite sem nenhum marcador.
    let mut slow = config("1.20.1", Loader::Fabric, Goal::World);
    slow.max_ms = 1_000;
    let mut watcher = RoundWatcher::new(slow);
    assert_eq!(watcher.tick(1_000), Some(Decision::Timeout));
}
