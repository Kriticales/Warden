//! Golden tests do "Testar com perfil de desempenho" (L-12; CA-T13-13) com arquivos reais:
//! os `debug.log` (e o `fml-server-latest.log` do 1.7.10) e os perfis do spark de servidores
//! dedicados reais rodados em 18/10/2026 (origem, versões e o que foi trocado em
//! `tests/FIXTURES.md`). O código do FML que grava as linhas de tempo é o mesmo no cliente e no
//! servidor.

// Os números do resumo já saem arredondados a uma casa: a comparação exata é a do valor
// mostrado.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::float_cmp
)]

use std::path::{Path, PathBuf};

use proptest::prelude::*;
use warden_perf::Loader;
use warden_perf::mod_timing::{TRACE_PROPERTY, TimingSource, TimingStatus, parse, plan};
use warden_perf::spark::{ClassOwner, SparkKind, SparkSide, summarize};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join(name)
}

fn log(name: &str) -> String {
    std::fs::read_to_string(fixture(&format!("logs/{name}"))).unwrap()
}

fn spark(name: &str) -> Vec<u8> {
    std::fs::read(fixture(&format!("spark/{name}"))).unwrap()
}

fn ranking(timings: &warden_perf::mod_timing::ModTimings) -> Vec<(&str, u32)> {
    timings
        .mods
        .iter()
        .map(|item| (item.key.as_str(), item.ms))
        .collect()
}

#[test]
fn forge_1_12_2_le_o_bar_step_do_debug_log() {
    let plan = plan("1.12.2", Loader::Forge, false);
    assert_eq!(plan.source, Some(TimingSource::BarStep));
    assert_eq!(plan.logs[0], "logs/debug.log");
    // Sempre ligado: este teste não precisa de argumento nenhum.
    assert!(plan.jvm_args.is_empty());
    let timings = parse(
        TimingSource::BarStep,
        &log("forge-1.12.2-servidor-debug.log"),
        "logs/debug.log",
    );
    assert_eq!(timings.status, TimingStatus::Measured);
    assert_eq!(
        ranking(&timings),
        [
            ("Just Enough Items", 138),
            ("AppleSkin", 51),
            ("Clumps", 31)
        ]
    );
    // Minecraft, MCP, FML e Forge, sem as fases de abrir e fechar o mundo.
    assert_eq!(timings.game_and_loader_ms, 1345);
    let jei = &timings.mods[0];
    assert_eq!(jei.phases[0].phase, "Construction");
    assert_eq!(jei.phases[1].phase, "PreInitialization");
    assert_eq!(jei.phases[1].ms, 83);
    assert!(
        jei.phases
            .iter()
            .all(|phase| !phase.phase.starts_with("Server"))
    );
}

#[test]
fn forge_1_20_1_com_o_registro_detalhado_deste_teste() {
    let plan = plan("1.20.1", Loader::Forge, false);
    assert_eq!(plan.source, Some(TimingSource::Trace));
    assert_eq!(plan.jvm_args, [TRACE_PROPERTY]);
    let text = log("forge-1.20.1-servidor-debug.log");
    let timings = parse(TimingSource::Trace, &text, "logs/debug.log");
    assert_eq!(timings.status, TimingStatus::Measured);
    assert_eq!(
        ranking(&timings),
        [
            ("jei", 288),
            ("mezz_config", 60),
            ("spark", 51),
            ("clumps", 6),
            ("ferritecore", 5)
        ]
    );
    assert_eq!(timings.game_and_loader_ms, 350);
    assert_eq!(timings.mods[0].phases[0].phase, "instance");
    assert_eq!(timings.mods[0].phases[0].ms, 252);
    // Os nomes dos eventos sem o `@hash` do `toString`.
    assert!(
        timings
            .mods
            .iter()
            .flat_map(|item| &item.phases)
            .all(|phase| !phase.phase.contains('@'))
    );

    // O mesmo log sem o TRACE (o nível padrão do debug.log): sem tempo por mod.
    let without_trace = text
        .lines()
        .filter(|line| !line.contains("/TRACE]"))
        .collect::<Vec<_>>()
        .join("\n");
    let timings = parse(TimingSource::Trace, &without_trace, "logs/debug.log");
    assert_eq!(timings.status, TimingStatus::NoLines);
}

#[test]
fn neoforge_1_21_1_casa_os_eventos_por_thread() {
    let timings = parse(
        TimingSource::Trace,
        &log("neoforge-1.21.1-servidor-debug.log"),
        "logs/debug.log",
    );
    assert_eq!(timings.status, TimingStatus::Measured);
    // O NeoForge não registra a construção da instância: só os eventos de cada mod. As linhas
    // "for phase NORMAL" (uma por prioridade) não contam duas vezes.
    let keys: Vec<&str> = timings.mods.iter().map(|item| item.key.as_str()).collect();
    assert_eq!(
        keys,
        ["ferritecore", "clumps", "jei", "mezz_config", "spark"]
    );
    assert_eq!(timings.mods_ms, 4);
    assert_eq!(timings.game_and_loader_ms, 5);
}

#[test]
fn forge_1_7_10_le_os_pares_de_eventos_do_fml() {
    let plan = plan("1.7.10", Loader::Forge, true);
    assert_eq!(plan.logs, ["logs/fml-server-latest.log"]);
    let timings = parse(
        TimingSource::FmlEvents,
        &log("forge-1.7.10-fml-server-latest.log"),
        "logs/fml-server-latest.log",
    );
    assert_eq!(timings.status, TimingStatus::Measured);
    assert_eq!(ranking(&timings), [("worldedit", 0)]);
    // O FMLServerAboutToStartEvent (1 s) é de abrir o mundo: fica de fora.
    assert!(timings.mods[0].phases.is_empty());
}

#[test]
fn fabric_nao_tem_tempo_por_mod() {
    for loader in [Loader::Fabric, Loader::Quilt, Loader::Vanilla] {
        assert_eq!(plan("1.20.1", loader, false).source, None);
    }
}

/// O dono pelos pacotes dos jars reais do pack de teste (o que o índice da D-05 diria).
fn index(class: &str) -> Option<ClassOwner> {
    let owner = |item: &str, id: &str| ClassOwner {
        item: Some(item.into()),
        mod_id: Some(id.into()),
    };
    [
        ("mezz.jei.", "mods/jei.pw.toml", "jei"),
        ("me.lucko.spark.", "mods/spark.pw.toml", "spark"),
        (
            "malte0811.ferritecore.",
            "mods/ferrite-core.pw.toml",
            "ferritecore",
        ),
        ("com.blamejared.clumps.", "mods/clumps.pw.toml", "clumps"),
    ]
    .into_iter()
    .find(|(package, _, _)| class.starts_with(package))
    .map(|(_, item, id)| owner(item, id))
}

#[test]
fn perfil_do_spark_vira_o_resumo_com_os_mods() {
    let summary = summarize(
        SparkKind::Profile,
        &spark("forge-1.20.1-servidor.sparkprofile"),
        &index,
    )
    .unwrap();
    let platform = summary.platform.as_ref().unwrap();
    assert_eq!(platform.side, SparkSide::Server);
    assert_eq!(platform.name, "Forge");
    assert_eq!(platform.minecraft.as_deref(), Some("1.20.1"));
    assert_eq!(summary.started_at_ms, Some(1_792_284_127_503));
    assert_eq!(summary.ended_at_ms, Some(1_792_284_167_737));
    let ticks = summary.ticks.as_ref().unwrap();
    assert_eq!(ticks.mspt_median, 1.6);
    assert_eq!(ticks.mspt_max, 213.7);
    assert_eq!(ticks.tps, Some(20.0));
    assert_eq!(ticks.ticks, Some(800));
    // Servidor parado (sem jogadores): quase tudo é do jogo; o spark aparece pelo índice.
    assert_eq!(summary.total, 1856);
    assert_eq!(summary.top.len(), 1);
    assert_eq!(summary.top[0].item.as_deref(), Some("mods/spark.pw.toml"));
    assert_eq!(summary.top[0].mod_id.as_deref(), Some("spark"));
    assert_eq!(summary.top[0].percent, 1.7);
    assert_eq!(summary.game_percent, 98.3);

    // Sem o índice, o dono vem do `class_sources` do próprio arquivo.
    let alone = summarize(
        SparkKind::Profile,
        &spark("forge-1.20.1-servidor.sparkprofile"),
        &|_| None,
    )
    .unwrap();
    assert_eq!(alone.top[0].item, None);
    assert_eq!(alone.top[0].mod_id.as_deref(), Some("spark"));
}

#[test]
fn perfil_do_neoforge_sem_as_esperas_da_thread() {
    let summary = summarize(
        SparkKind::Profile,
        &spark("neoforge-1.21.1-servidor.sparkprofile"),
        &index,
    )
    .unwrap();
    // 40 s de perfil com o motor nativo: o servidor esperou quase o tempo todo
    // (`Unsafe.park`, `Thread.yield`), e essas amostras não contam.
    assert_eq!(summary.total, 428);
    assert_eq!(summary.platform.as_ref().unwrap().name, "NeoForge");
    assert_eq!(summary.ticks.as_ref().unwrap().mspt_median, 0.5);
    assert_eq!(summary.top[0].mod_id.as_deref(), Some("spark"));
}

#[test]
fn memoria_do_spark_agrupada_pelos_jars() {
    let summary = summarize(
        SparkKind::Heap,
        &spark("forge-1.20.1-servidor.sparkheap"),
        &index,
    )
    .unwrap();
    assert_eq!(summary.kind, SparkKind::Heap);
    assert_eq!(summary.platform.as_ref().unwrap().side, SparkSide::Server);
    let ids: Vec<&str> = summary
        .top
        .iter()
        .filter_map(|share| share.mod_id.as_deref())
        .collect();
    assert_eq!(ids, ["ferritecore", "spark", "jei", "clumps"]);
    assert!(summary.game_percent > 90.0);
    assert!(summary.total > 100_000_000);
}

#[test]
fn arquivos_reais_cortados_nao_entram_em_panico() {
    for (name, kind) in [
        ("forge-1.20.1-servidor.sparkprofile", SparkKind::Profile),
        ("forge-1.20.1-servidor.sparkheap", SparkKind::Heap),
    ] {
        let bytes = spark(name);
        for cut in [1, 7, bytes.len() / 3, bytes.len() / 2, bytes.len() - 1] {
            // Cortado no meio de um campo: erro de leitura, nunca pânico.
            let _ = summarize(kind, &bytes[..cut], &index);
        }
    }
}

proptest! {
    #[test]
    fn bytes_quaisquer_nao_entram_em_panico(bytes in proptest::collection::vec(any::<u8>(), 0..2048)) {
        let _ = summarize(SparkKind::Profile, &bytes, &index);
        let _ = summarize(SparkKind::Heap, &bytes, &index);
    }

    #[test]
    fn texto_qualquer_nao_entra_em_panico(text in "\\PC{0,400}") {
        for source in [TimingSource::BarStep, TimingSource::FmlEvents, TimingSource::Trace] {
            let _ = parse(source, &text, "logs/debug.log");
        }
    }
}
