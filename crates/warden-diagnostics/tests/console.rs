//! Console agrupado (D-09) com logs reais de Forge, NeoForge e Fabric: os recortes da matriz da
//! L-05 (`warden-launcher/tests/matrix/`, capturados no Windows) e os esqueletos de jars reais
//! da D-05 (`warden-jarmeta/tests/indice/esqueletos.json`) para o índice de pacotes.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr
)]

use std::collections::BTreeMap;
use std::io::{Cursor, Write as _};
use std::path::{Path, PathBuf};
use std::time::Instant;

use serde_json::Value;
use warden_diagnostics::console::{
    ConsoleGrouper, GroupKind, ItemSeverity, LineLevel, LinesTarget, LogEntry, ModDirectory,
};
use warden_jarmeta::index::{PackIndex, index_jar_bytes};
use warden_jarmeta::{Limits, Loader};
use warden_launcher::log::{LogLevel, LogLine, LogParser, LogSource};
use zip::write::{SimpleFileOptions, ZipWriter};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

/// Esqueleto de um jar real versionado pela D-05.
fn skeleton(name: &str) -> Value {
    let path = root().join("../warden-jarmeta/tests/indice/esqueletos.json");
    let all: BTreeMap<String, Value> =
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    all[name].clone()
}

/// Remonta o jar (descritores, uma classe por pacote e as configs de mixin), como a D-05.
fn rebuild(skeleton: &Value) -> Vec<u8> {
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default();
    let empty = serde_json::Map::new();
    for (name, content) in skeleton["arquivos"].as_object().unwrap_or(&empty) {
        zip.start_file(name.as_str(), options).unwrap();
        let bytes: Vec<u8> = match content {
            Value::String(text) => text.as_bytes().to_vec(),
            other => serde_json::from_value(other.clone()).unwrap(),
        };
        zip.write_all(&bytes).unwrap();
    }
    for package in skeleton["pacotes"].as_array().into_iter().flatten() {
        let name = format!("{}/X.class", package.as_str().unwrap().replace('.', "/"));
        zip.start_file(name, options).unwrap();
        zip.write_all(&[0xCA, 0xFE, 0xBA, 0xBE, 0, 0, 0, 65])
            .unwrap();
    }
    for config in skeleton["mixins"].as_array().into_iter().flatten() {
        zip.start_file(config.as_str().unwrap(), options).unwrap();
        zip.write_all(b"{}").unwrap();
    }
    zip.finish().unwrap().into_inner()
}

/// Os mods do pack: `(esqueleto, metafile, arquivo do jar)`.
fn directory(loader: Loader, mods: &[(&str, &str, &str)]) -> ModDirectory {
    let mut items = Vec::new();
    let mut files = Vec::new();
    for (name, item, file) in mods {
        let index = index_jar_bytes(&rebuild(&skeleton(name)), &Limits::default()).unwrap();
        items.push(((*item).to_owned(), index));
        files.push(((*item).to_owned(), (*file).to_owned()));
    }
    ModDirectory::new(PackIndex::new(items, Some(loader)), files)
}

/// Lê um log como o console lê o `output.log` (eventos XML e texto).
fn parse(text: &str) -> Vec<LogLine> {
    let mut parser = LogParser::new(LogSource::Stdout);
    let mut lines: Vec<LogLine> = text.lines().flat_map(|line| parser.push(line, 0)).collect();
    lines.extend(parser.finish(0));
    lines
}

fn level(level: LogLevel) -> LineLevel {
    match level {
        LogLevel::Trace => LineLevel::Trace,
        LogLevel::Debug => LineLevel::Debug,
        LogLevel::Info => LineLevel::Info,
        LogLevel::Warn => LineLevel::Warn,
        LogLevel::Error => LineLevel::Error,
        LogLevel::Fatal => LineLevel::Fatal,
    }
}

fn entries(lines: &[LogLine]) -> Vec<LogEntry<'_>> {
    lines
        .iter()
        .enumerate()
        .map(|(seq, line)| LogEntry {
            seq: u32::try_from(seq).unwrap(),
            level: line.level.map(level),
            logger: line.logger.as_deref(),
            time: line.time.as_deref(),
            timestamp_ms: line.timestamp_ms,
            text: &line.text,
        })
        .collect()
}

fn group(text: &str, directory: ModDirectory) -> ConsoleGrouper {
    let lines = parse(text);
    let mut grouper = ConsoleGrouper::new(directory);
    // Em lotes pequenos, como chegam do jogo: o stack trace atravessa os lotes.
    for batch in entries(&lines).chunks(7) {
        grouper.push_batch(batch);
    }
    grouper
}

fn fixture(name: &str) -> String {
    std::fs::read_to_string(root().join("../warden-launcher/tests/matrix").join(name)).unwrap()
}

fn forge_pack() -> ModDirectory {
    directory(
        Loader::Forge,
        &[
            (
                "forge-1.20.1-supplementaries",
                "mods/supplementaries.pw.toml",
                "supplementaries-1.20-3.1.13.jar",
            ),
            (
                "forge-1.20.1-epic-fight",
                "mods/epic-fight.pw.toml",
                "epicfight-forge-20.9.4-1.20.1.jar",
            ),
            (
                "forge-1.20.1-create",
                "mods/create.pw.toml",
                "create-1.20.1-0.5.1.f.jar",
            ),
        ],
    )
}

fn xml(
    logger: &str,
    timestamp: i64,
    level: &str,
    message: &str,
    throwable: Option<&str>,
) -> String {
    let throwable = throwable.map_or(String::new(), |t| {
        format!("    <log4j:Throwable><![CDATA[{t}\n]]></log4j:Throwable>\n")
    });
    format!(
        "  <log4j:Event logger=\"{logger}\" timestamp=\"{timestamp}\" level=\"{level}\" thread=\"Render thread\">\n    <log4j:Message><![CDATA[{message}]]></log4j:Message>\n{throwable}  </log4j:Event>\n"
    )
}

/// CA-T13-12: 1.284 repetições de `Missing texture for {id}` viram um item "×1.284" do mod
/// certo pelo índice de pacotes; um stack trace vira um item recolhido com a primeira linha de
/// mod; "Só problemas" esconde as linhas de informação.
#[test]
#[allow(clippy::too_many_lines)] // o cenário inteiro do CA num teste só
fn ca_t13_12_repeticoes_stack_trace_e_so_problemas() {
    let mut log = fixture("falhas/forge-entrar.log");
    let start: i64 = 1_791_160_000_000;
    let textures = ["block/jar", "block/sconce", "item/rope", "block/bellows"];
    for index in 0..1284_i64 {
        log.push_str(&xml(
            "net.mehvahdjukaar.supplementaries.Supplementaries",
            start + index * 120,
            "WARN",
            &format!(
                "Missing texture for supplementaries:{} at {}",
                textures[usize::try_from(index).unwrap() % textures.len()],
                index % 16
            ),
            None,
        ));
    }
    let trace = concat!(
        "java.lang.NullPointerException: Cannot invoke \"net.minecraft.world.entity.Entity.getX()\"\n",
        "\tat java.base/java.util.Objects.requireNonNull(Objects.java:259)\n",
        "\tat net.minecraft.world.entity.LivingEntity.travel(LivingEntity.java:2108) ~[client-1.20.1-20230612.114412-srg.jar%23255!/:?]\n",
        "\tat yesman.epicfight.world.capabilities.entitypatch.LivingEntityPatch.onTravel(LivingEntityPatch.java:412) ~[epicfight-forge-20.9.4-1.20.1.jar%23154!/:20.9.4]\n",
        "\tat net.minecraft.world.entity.LivingEntity.aiStep(LivingEntity.java:2541) ~[client-1.20.1-20230612.114412-srg.jar%23255!/:?]",
    );
    log.push_str(&xml(
        "net.minecraft.server.MinecraftServer",
        start + 200_000,
        "ERROR",
        "Encountered an unexpected exception",
        Some(trace),
    ));
    let grouper = group(&log, forge_pack());

    let groups = grouper.groups("");
    let supplementaries = groups
        .iter()
        .find(|g| g.item.as_deref() == Some("mods/supplementaries.pw.toml"))
        .expect("grupo do Supplementaries");
    assert_eq!(supplementaries.kind, GroupKind::Item);
    assert_eq!(supplementaries.mod_id.as_deref(), Some("supplementaries"));
    assert_eq!(
        (supplementaries.warnings, supplementaries.repeated),
        (1284, 1284)
    );
    let (items, total) = grouper.group_items(&supplementaries.key, "", 50);
    assert_eq!(total, 1);
    assert_eq!(items[0].count, 1284);
    assert_eq!(items[0].text, "Missing texture for {id} at {n}");
    assert_eq!(items[0].first.at_ms, Some(1_791_160_000_000.0));
    assert_eq!(
        items[0].last.at_ms,
        Some(1_791_160_000_000.0 + 1283.0 * 120.0)
    );

    // Copiar o grupo devolve as linhas originais, na ordem.
    let originals = grouper.original_lines(&LinesTarget::Group(supplementaries.key.clone()), None);
    assert_eq!(originals.len(), 1284);
    assert_eq!(
        originals[0].text,
        "Missing texture for supplementaries:block/jar at 0"
    );
    assert_eq!(
        originals[5].text,
        "Missing texture for supplementaries:block/sconce at 5"
    );

    let epic = groups
        .iter()
        .find(|g| g.item.as_deref() == Some("mods/epic-fight.pw.toml"))
        .expect("grupo do Epic Fight (jar no frame)");
    assert_eq!(epic.errors, 1);
    let (items, _) = grouper.group_items(&epic.key, "", 50);
    let crash = &items[0];
    assert!(crash.trace);
    assert_eq!(crash.severity, ItemSeverity::Error);
    assert_eq!(crash.text, "Encountered an unexpected exception");
    assert_eq!(crash.extra_lines, 5);
    assert!(
        crash
            .mod_frame
            .as_deref()
            .is_some_and(|frame| frame.starts_with("at yesman.epicfight.")),
        "{:?}",
        crash.mod_frame
    );

    // Só problemas: avisos, erros e stack traces; nada de informação nem de ruído conhecido.
    let (problems, total) = grouper.problems("", 10_000);
    assert_eq!(problems.len(), usize::try_from(total).unwrap());
    assert!(
        problems
            .iter()
            .all(|p| p.severity != ItemSeverity::Info || p.trace)
    );
    assert!(problems.iter().all(|p| p.noise.is_none()));
    assert!(problems.iter().any(|p| p.count == 1284));
    assert!(problems.iter().any(|p| p.trace));
    // O recorte real tem o ruído offline e o do Forge, marcados no agrupado.
    let game = grouper.group_items("game", "", 10_000).0;
    let noise: Vec<&str> = game.iter().filter_map(|i| i.noise.as_deref()).collect();
    assert!(
        noise.contains(&"offline.verify_authentication"),
        "{noise:?}"
    );
    assert!(
        noise.contains(&"forge.library_without_mods_toml"),
        "{noise:?}"
    );
    assert!(
        game.iter().any(|i| i.severity == ItemSeverity::Info),
        "as linhas de informação continuam no agrupado"
    );
}

/// NeoForge 1.21.1: a falha de Mixin com o stack trace em `MC-BOOTSTRAP` é do mod que o Mixin
/// cita; as linhas de texto puro ficam em "Jogo e loader" pelo logger `minecraft/…`.
#[test]
fn neoforge_falha_de_mixin_e_texto() {
    let grouper = group(&fixture("falhas/neo-mixin.log"), ModDirectory::empty());
    let groups = grouper.groups("");
    assert_eq!(groups[0].key, "mod:wardenfalhas", "{groups:?}");
    let (items, _) = grouper.group_items("mod:wardenfalhas", "", 10);
    assert!(
        items
            .iter()
            .any(|item| item.trace && item.severity == ItemSeverity::Error)
    );
    assert!(groups.iter().any(|g| g.kind == GroupKind::Game));

    let text = fixture("matriz/neoforge-1.21.1-cliente.log");
    let grouper = group(&text, ModDirectory::empty());
    let game = grouper
        .groups("")
        .into_iter()
        .find(|g| g.key == "game")
        .unwrap();
    assert!(game.entries >= 12, "{game:?}");
    let items = grouper.group_items("game", "logged in", 10).0;
    assert_eq!(items[0].first.time.as_deref(), Some("19:26:37"));
}

/// Fabric 1.20.1: a falha de Mixin cita `handler$…$wardenfalhas$…` e "for mod wardenfalhas".
#[test]
fn fabric_falha_de_mixin() {
    let grouper = group(&fixture("falhas/fab-mixin.log"), ModDirectory::empty());
    let groups = grouper.groups("");
    let mixin = groups
        .iter()
        .find(|g| g.key == "mod:wardenfalhas")
        .expect("grupo do mod");
    assert!(mixin.errors >= 1, "{mixin:?}");
}

/// Fabric com o índice: o logger com o nome do mod (`Sodium`) e a classe do mod.
#[test]
fn fabric_logger_com_nome_e_classe_do_mod() {
    let dir = directory(
        Loader::Fabric,
        &[(
            "fabric-1.20.1-sodium",
            "mods/sodium.pw.toml",
            "sodium-fabric-0.5.11.jar",
        )],
    );
    let log = concat!(
        "[12:00:01] [Render thread/INFO] [Sodium/]: Loaded configuration file\n",
        "[12:00:02] [Render thread/WARN] [me.jellysquid.mods.sodium.client.SodiumClientMod/]: Driver antigo\n",
        "[12:00:03] [Render thread/INFO] [me.jellysquid.mods.sodium.client.SodiumClientMod/]: Pronto\n",
        "[12:00:04] [Render thread/INFO] [outra.coisa.Qualquer/]: Sem dono\n",
    );
    let grouper = group(log, dir);
    let groups = grouper.groups("");
    assert_eq!(groups[0].item.as_deref(), Some("mods/sodium.pw.toml"));
    assert_eq!((groups[0].entries, groups[0].warnings), (3, 1));
    assert_eq!(groups.last().unwrap().key, "unknown");
}

/// 50.000 linhas (e 300.000) agrupadas sem travar: o tempo de agrupar e de montar a tela.
#[test]
fn desempenho_50_mil_linhas() {
    let dir = forge_pack();
    let lines = synthetic(50_000);
    let started = Instant::now();
    let mut grouper = ConsoleGrouper::new(dir);
    for batch in entries(&lines).chunks(500) {
        grouper.push_batch(batch);
    }
    let pushed = started.elapsed();
    let started = Instant::now();
    let groups = grouper.groups("");
    let _ = grouper.group_items("game", "", 200);
    let _ = grouper.problems("", 500);
    let view = started.elapsed();
    eprintln!(
        "50.000 linhas: agrupar {pushed:?}, montar a tela {view:?}, {} itens, {} grupos",
        grouper.item_count(),
        groups.len()
    );
    assert_eq!(grouper.lines(), 50_000);
    // Folga para o build de depuração e máquinas lentas da CI; em release fica bem abaixo.
    assert!(pushed.as_secs() < 20, "{pushed:?}");
    assert!(view.as_secs() < 2, "{view:?}");
}

/// Medição com 300.000 linhas (rode com `--release --ignored --nocapture`).
#[test]
#[ignore = "medição: rodar em release"]
fn desempenho_300_mil_linhas() {
    let lines = synthetic(300_000);
    let started = Instant::now();
    let mut grouper = ConsoleGrouper::new(forge_pack());
    for batch in entries(&lines).chunks(500) {
        grouper.push_batch(batch);
    }
    let pushed = started.elapsed();
    let started = Instant::now();
    let _ = grouper.groups("");
    let _ = grouper.group_items("game", "", 200);
    let _ = grouper.problems("", 500);
    let view = started.elapsed();
    let started = Instant::now();
    let _ = grouper.groups("texture");
    let search = started.elapsed();
    eprintln!(
        "300.000 linhas: agrupar {pushed:?}, montar a tela {view:?}, buscar {search:?}, {} itens",
        grouper.item_count()
    );
}

/// Um log grande e variado: linhas do jogo, de mods, repetições e stack traces em texto puro.
fn synthetic(count: usize) -> Vec<LogLine> {
    let mut text = String::new();
    let mut index = 0;
    while index < count {
        let n = index;
        let line = match n % 10 {
            0 => format!(
                "[18:02:{:02}] [Render thread/WARN] [net.mehvahdjukaar.supplementaries.Supplementaries/]: Missing texture for supplementaries:block/x{} at {}\n",
                n % 60,
                n % 37,
                n
            ),
            1 => format!(
                "[18:02:{:02}] [Server thread/INFO] [minecraft/MinecraftServer]: Saving chunks for level 'ServerLevel[mundo {n}]'/minecraft:overworld\n",
                n % 60
            ),
            2 => format!(
                "[18:02:{:02}] [Worker-Main-{}/INFO] [com.simibubi.create.Create/]: Registro {n} da linha única {}\n",
                n % 60,
                n % 8,
                n * 7919
            ),
            3 => format!(
                "[18:02:{:02}] [Render thread/INFO] [minecraft/TextureAtlas]: Created: {}x{}x4 minecraft:textures/atlas/blocks.png-atlas\n",
                n % 60,
                n % 4096,
                n % 2048
            ),
            4 if n % 1000 == 4 => {
                index += 4;
                format!(
                    "[18:02:{:02}] [Render thread/ERROR] [yesman.epicfight.main.EpicFightMod/]: Falha {n}\njava.lang.IllegalStateException: estado {n}\n\tat yesman.epicfight.world.X.m(X.java:{n}) ~[epicfight-forge-20.9.4-1.20.1.jar%23154!/:20.9.4]\n\tat net.minecraft.world.entity.LivingEntity.travel(LivingEntity.java:2108)\n\t... 12 more\n",
                    n % 60
                )
            }
            5 => format!(
                "[18:02:{:02}] [Render thread/INFO] [minecraft/ModelBakery]: Carregando o modelo {}\n",
                n % 60,
                word(n)
            ),
            _ => format!(
                "[18:02:{:02}] [Render thread/INFO] [minecraft/Minecraft]: Linha comum {n} com valor 0x{:x}\n",
                n % 60,
                n * 31
            ),
        };
        text.push_str(&line);
        index += 1;
    }
    parse(&text).into_iter().take(count).collect()
}

/// Uma palavra diferente para cada número (linhas que nunca repetem).
fn word(mut n: usize) -> String {
    let mut out = String::new();
    loop {
        out.push(char::from(b'a' + u8::try_from(n % 26).unwrap()));
        n /= 26;
        if n == 0 {
            return out;
        }
    }
}
