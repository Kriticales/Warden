//! Corpus real do S-R5-1 (CA-T07-04 e os casos da D-10): os jars dos 3 packs Better MC, baixados
//! pela lista `tests/corpus/jars-<pack>.json` (URL e sha1 conferido) para a `target`. Nada é
//! versionado além da lista.
//!
//! - `rede_inventario_dourado`: o inventário (configs, classes, alterações por tipo) dos jars do
//!   CA-T07-04 bate com `tests/corpus/dourado-inventario.json`, sem nenhuma classe ilegível.
//!   `WARDEN_DOURADO=1` regrava o dourado (conferir a diferença antes de versionar).
//! - `rede_casos_do_spike`: sodium-extra × sodiumextras e badoptimizations × citadel = alto;
//!   Krypton × Lithium some; Lithium × `ModernFix` e Iris × Sodium = baixo.
//! - `rede_packs_inteiros` (só com `WARDEN_RAIOX_PACKS=1`, baixa ~1,8 GB): os totais dos 3 packs
//!   batem com o resumo do protótipo (`resultado-<pack>.json`), os avisos e o tempo.
//!
//! Roda com `cargo xtask test-network`.

#![allow(clippy::expect_used, clippy::panic, clippy::print_stdout)]

mod common;

use std::collections::BTreeMap;
use std::path::Path;
use std::time::Instant;

use serde_json::{Value, json};
use warden_jarmeta::{Limits, Loader};
use warden_mixin::cache::XrayCache;
use warden_mixin::model::IssueKind;
use warden_mixin::options::{disabled_by_pack_configs, options_data};
use warden_mixin::overlap::{CrossContext, CuratedPair, Overlap, Risk};
use warden_mixin::pack::{CacheSlot, PackJar, PackXray, scan_pack};

const PACKS: [(&str, Loader); 3] = [
    ("fabric-1.20.1", Loader::Fabric),
    ("forge-1.20.1", Loader::Forge),
    ("neoforge-1.21.1", Loader::NeoForge),
];

/// Os jars do CA-T07-04 (o pack Forge do corpus usa o Xenon, porte do Embeddium).
const GOLDEN: [(&str, &[&str]); 3] = [
    (
        "fabric-1.20.1",
        &[
            "sodium-fabric-0.5.13+mc1.20.1.jar",
            "lithium-fabric-mc1.20.1-0.11.4.jar",
            "iris-1.7.6+mc1.20.1.jar",
            "modernfix-fabric-5.24.4+mc1.20.1.jar",
        ],
    ),
    (
        "forge-1.20.1",
        &[
            "xenon-0.3.31+mc1.20.1.jar",
            "oculus-mc1.20.1-1.8.0.jar",
            "modernfix-forge-5.27.51+mc1.20.1.jar",
            "ferritecore-6.0.1-forge.jar",
        ],
    ),
    (
        "neoforge-1.21.1",
        &[
            "sodium-neoforge-0.6.13+mc1.21.1.jar",
            "iris-neoforge-1.8.8+mc1.21.1.jar",
            "ferritecore-7.0.2-neoforge.jar",
        ],
    ),
];

fn jar_list(pack: &str) -> Vec<(String, String, String)> {
    let text =
        std::fs::read_to_string(common::corpus(&format!("jars-{pack}.json"))).expect("lista");
    let value: Value = serde_json::from_str(&text).expect("json");
    value["jars"]
        .as_array()
        .expect("jars")
        .iter()
        .map(|j| {
            let path = j["path"].as_str().expect("path");
            (
                path.trim_start_matches("mods/").to_owned(),
                j["url"].as_str().expect("url").to_owned(),
                j["sha1"].as_str().expect("sha1").to_owned(),
            )
        })
        .collect()
}

/// Baixa (ou acha no cache) os jars do pack cujo nome passa no filtro.
fn jars(pack: &str, keep: impl Fn(&str) -> bool) -> Vec<PackJar> {
    let cache = common::cache_dir("raio-x-corpus");
    jar_list(pack)
        .into_iter()
        .filter(|(name, _, _)| keep(name))
        .map(|(name, url, sha1)| {
            let path = common::download_cached(&url, &sha1, &cache, &name).expect("download");
            PackJar::new(format!("mods/{name}"), path)
        })
        .collect()
}

fn known_compatible() -> Vec<CuratedPair> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../warden-diagnostics/data/mixin-known-compatible.toml");
    let table: toml::Table = std::fs::read_to_string(path)
        .expect("lista")
        .parse()
        .expect("toml");
    let ids = |v: &toml::Value| -> Vec<String> {
        v.as_array()
            .expect("lista")
            .iter()
            .map(|x| x.as_str().expect("id").to_owned())
            .collect()
    };
    let mut out = Vec::new();
    for pair in table["par"].as_array().expect("par") {
        for a in ids(&pair["primeiro"]) {
            for b in ids(&pair["segundo"]) {
                out.push(CuratedPair {
                    first: a.clone(),
                    second: b,
                    label: "lista".into(),
                });
            }
        }
    }
    out
}

fn context(pack: &str) -> CrossContext {
    CrossContext {
        disabled: disabled_by_pack_configs(
            &common::corpus(&format!("{pack}/config")),
            options_data(),
        ),
        known_compatible: known_compatible(),
        ..CrossContext::default()
    }
}

#[test]
#[ignore = "rede"]
fn rede_inventario_dourado() {
    let mut found = BTreeMap::new();
    for ((pack, loader), (_, names)) in PACKS.iter().zip(GOLDEN) {
        for jar in jars(pack, |n| names.contains(&n)) {
            let scan = warden_mixin::scan::scan_jar_file(&jar.path, *loader, &Limits::default());
            let mut configs = Vec::new();
            let mut classes = 0;
            let mut changes: BTreeMap<String, u32> = BTreeMap::new();
            let mut nested_with_mixins = Vec::new();
            for module in &scan.mods {
                let unreadable: Vec<_> = module
                    .issues
                    .iter()
                    .filter(|i| {
                        matches!(
                            i.kind,
                            IssueKind::UnreadableClass | IssueKind::UnreadableJar
                        )
                    })
                    .collect();
                assert!(unreadable.is_empty(), "{}: {unreadable:?}", jar.item);
                configs.extend(module.configs.iter().cloned());
                classes += module.classes.len();
                for change in module.classes.iter().flat_map(|c| &c.changes) {
                    *changes
                        .entry(change.kind.annotation().to_owned())
                        .or_default() += 1;
                }
                if !module.path.is_empty() && !module.classes.is_empty() {
                    nested_with_mixins.push(module.path.clone());
                }
            }
            configs.sort();
            found.insert(
                format!("{pack}/{}", jar.item.trim_start_matches("mods/")),
                json!({ "configs": configs, "classes": classes, "alteracoes": changes, "embutidosComMixins": nested_with_mixins }),
            );
        }
    }
    let golden_path = common::corpus("dourado-inventario.json");
    let found = Value::Object(found.into_iter().collect());
    if std::env::var_os("WARDEN_DOURADO").is_some() {
        let text = serde_json::to_string_pretty(&found).expect("json");
        std::fs::write(&golden_path, format!("{text}\n")).expect("dourado");
    }
    let golden: Value =
        serde_json::from_str(&std::fs::read_to_string(&golden_path).expect("dourado"))
            .expect("json");
    assert_eq!(found, golden);
    // O Sodium para NeoForge guarda todos os mixins no jar aninhado (R5A §5.7).
    assert!(
        golden["neoforge-1.21.1/sodium-neoforge-0.6.13+mc1.21.1.jar"]["embutidosComMixins"]
            .as_array()
            .is_some_and(|a| a.iter().any(|p| p
                .as_str()
                .is_some_and(|p| p.starts_with("META-INF/jarjar/"))))
    );
}

fn pair(overlaps: &[Overlap], a: &str, b: &str) -> Vec<Overlap> {
    overlaps
        .iter()
        .filter(|o| {
            let has = |id: &str| o.participants.iter().any(|p| p.mod_id == id);
            has(a) && has(b)
        })
        .cloned()
        .collect()
}

#[test]
#[ignore = "rede"]
fn rede_casos_do_spike() {
    let fabric_ids = [
        "sodium-extra",
        "sodiumextras",
        "sodium-fabric",
        "debugify",
        "fabric-api",
        "krypton",
        "lithium-fabric",
        "modernfix-fabric",
        "iris-1",
    ];
    let fabric = jars("fabric-1.20.1", |n| {
        fabric_ids.iter().any(|id| n.to_lowercase().starts_with(id))
    });
    let x = PackXray::build(
        scan_pack(&fabric, Loader::Fabric, None, &Limits::default()),
        &context("fabric-1.20.1"),
    );
    let cases = pair(&x.overlaps, "sodium-extra", "sodiumextras");
    assert!(
        cases
            .iter()
            .any(|o| o.risk == Risk::High && o.label() == "SodiumGameOptionPages.general"),
        "{cases:#?}"
    );
    let cases = pair(&x.overlaps, "debugify", "fabric-item-api-v1");
    assert!(
        cases.iter().any(|o| o.risk == Risk::High),
        "debugify × Fabric API: dependência de biblioteca não rebaixa; {cases:#?}"
    );
    assert!(
        pair(&x.overlaps, "krypton", "lithium")
            .iter()
            .all(|o| o.risk == Risk::Low),
        "Krypton desliga a opção do Lithium"
    );
    for (a, b) in [("lithium", "modernfix"), ("iris", "sodium")] {
        let cases = pair(&x.overlaps, a, b);
        assert!(!cases.is_empty(), "{a} × {b}");
        assert!(
            cases.iter().all(|o| o.risk == Risk::Low),
            "{a} × {b}: {cases:#?}"
        );
        assert!(
            cases
                .iter()
                .filter(|o| o.base == Risk::High)
                .all(|o| o.demoters.len() >= 2)
        );
    }

    let forge = jars("forge-1.20.1", |n| {
        let n = n.to_lowercase();
        n.starts_with("badoptimizations") || n.starts_with("citadel")
    });
    let x = PackXray::build(
        scan_pack(&forge, Loader::Forge, None, &Limits::default()),
        &context("forge-1.20.1"),
    );
    let cases = pair(&x.overlaps, "badoptimizations", "citadel");
    assert!(cases.iter().any(|o| o.risk == Risk::High), "{cases:#?}");
}

#[test]
#[ignore = "rede"]
fn rede_packs_inteiros() {
    if std::env::var_os("WARDEN_RAIOX_PACKS").is_none() {
        println!("pulado: defina WARDEN_RAIOX_PACKS=1 para baixar os 3 packs (~1,8 GB)");
        return;
    }
    for (pack, loader) in PACKS {
        let all = jars(pack, |_| true);
        let spike: Value = serde_json::from_str(
            &std::fs::read_to_string(common::corpus(&format!("resultado-{pack}.json")))
                .expect("resultado"),
        )
        .expect("json");
        let cache_dir = tempfile::tempdir().expect("temp");
        let cache = XrayCache::in_cache_dir(cache_dir.path());
        let slot = Some(CacheSlot {
            cache: &cache,
            key: "pack",
        });
        let started = Instant::now();
        let first = scan_pack(&all, loader, slot, &Limits::default());
        let cold = started.elapsed();
        let started = Instant::now();
        let scan = scan_pack(&all, loader, slot, &Limits::default());
        let x = PackXray::build(scan, &context(pack));
        let warm = started.elapsed();
        assert_eq!(first.items, x.scan.items);
        let modules: Vec<_> = x
            .scan
            .items
            .iter()
            .flat_map(|(_, s)| s.mods.iter())
            .collect();
        let classes: usize = modules.iter().map(|m| m.classes.len()).sum();
        let mut annotations: BTreeMap<String, u64> = BTreeMap::new();
        for m in &modules {
            for (name, count) in &m.other_annotations {
                *annotations.entry(name.clone()).or_default() += u64::from(*count);
            }
        }
        let expected = &spike["resumo"];
        println!(
            "{pack}: {} jars, {classes} classes (protótipo {}), sem cache {cold:?}, com cache {warm:?}, altos {}, médios {}",
            all.len(),
            expected["classes_de_mixin"],
            x.overlaps.iter().filter(|o| o.risk == Risk::High).count(),
            x.overlaps.iter().filter(|o| o.risk == Risk::Medium).count(),
        );
        // O protótipo descia em todo `.jar` de META-INF/jarjar; o Warden, só nos declarados (como o
        // loader). Diferença conhecida: o Crash Assistant para NeoForge (2 jars não declarados).
        let tolerance = if loader == Loader::NeoForge { 3 } else { 0 };
        let spike_classes =
            usize::try_from(expected["classes_de_mixin"].as_u64().expect("classes"))
                .expect("usize");
        assert!(
            spike_classes.abs_diff(classes) <= tolerance,
            "{pack}: {classes} × {spike_classes}"
        );
        if tolerance == 0 {
            for (name, count) in expected["anotacoes"].as_object().expect("anotações") {
                if name != "Mixin" {
                    assert_eq!(
                        annotations.get(name).copied().unwrap_or(0),
                        count.as_u64().expect("n"),
                        "{pack} {name}"
                    );
                }
            }
        }
        assert!(cold.as_secs_f64() < 3.0, "{pack}: {cold:?} sem cache");
        assert!(warm.as_millis() < 300, "{pack}: {warm:?} com cache");
    }
}
