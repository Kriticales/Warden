//! Mede o raio-x numa pasta `mods/` de verdade (D-10: tempo sem e com cache, quantos avisos).
//!
//! ```text
//! cargo run --release -p warden-mixin --example medir -- <fabric|forge|neoforge> <pasta mods> \
//!     [pasta config] [pasta do cache] [mixin-known-compatible.toml]
//! ```
//!
//! Pastas e arquivos opcionais podem ser `-` (não usar).
//!
//! Imprime o tempo da leitura, os números do cruzamento por risco e as sobreposições de risco
//! médio ou alto agrupadas por par de mods (como o aviso `W_MIXIN_OVERLAP` agrupa).
#![allow(clippy::print_stdout, clippy::print_stderr, clippy::too_many_lines, missing_docs)]

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Instant;

use warden_jarmeta::{Limits, Loader};
use warden_mixin::cache::XrayCache;
use warden_mixin::options::{disabled_by_pack_configs, options_data};
use warden_mixin::overlap::{CrossContext, CuratedPair, Risk};
use warden_mixin::pack::{CacheSlot, PackJar, PackXray, scan_pack};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (Some(loader), Some(mods)) = (args.first(), args.get(1)) else {
        eprintln!(
            "uso: medir <fabric|forge|neoforge> <pasta mods> [pasta config] [pasta do cache]"
        );
        std::process::exit(2);
    };
    let loader = match loader.as_str() {
        "fabric" => Loader::Fabric,
        "forge" => Loader::Forge,
        "neoforge" => Loader::NeoForge,
        _ => Loader::Quilt,
    };
    let mut jars: Vec<PackJar> = std::fs::read_dir(mods)
        .map(|dir| {
            dir.filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("jar")))
                .map(|path| PackJar {
                    item: format!(
                        "mods/{}",
                        path.file_name().unwrap_or_default().to_string_lossy()
                    ),
                    path,
                })
                .collect()
        })
        .unwrap_or_default();
    jars.sort_by(|a, b| a.item.cmp(&b.item));
    let given = |i: usize| args.get(i).filter(|a| a.as_str() != "-");
    let cache = given(3).map(|dir| XrayCache::new(PathBuf::from(dir)));
    let slot = cache.as_ref().map(|cache| CacheSlot {
        cache,
        key: "medir",
    });
    let mut context = CrossContext::default();
    if let Some(config) = given(2) {
        context.disabled = disabled_by_pack_configs(&PathBuf::from(config), options_data());
    }
    if let Some(known) = given(4) {
        context.known_compatible = known_pairs(&std::fs::read_to_string(known).unwrap_or_default());
    }
    let started = Instant::now();
    let scan = scan_pack(&jars, loader, slot, &Limits::default());
    let read = started.elapsed();
    let stats = scan.stats;
    let xray = PackXray::build(scan, &context);
    let total = started.elapsed();
    let mods_count: usize = xray.scan.items.iter().map(|(_, s)| s.mods.len()).sum();
    let classes: usize = xray
        .scan
        .items
        .iter()
        .flat_map(|(_, s)| s.mods.iter())
        .map(|m| m.classes.len())
        .sum();
    let unreadable: usize = xray
        .scan
        .items
        .iter()
        .flat_map(|(_, s)| s.mods.iter())
        .flat_map(|m| m.issues.iter())
        .filter(|i| matches!(i.kind, warden_mixin::model::IssueKind::UnreadableClass))
        .count();
    println!(
        "jars {} (lidos agora {}, hash agora {}), mods com embutidos {mods_count}, classes {classes}, classes ilegíveis {unreadable}",
        stats.jars, stats.scanned, stats.hashed
    );
    println!("tempo: leitura {read:?}, total {total:?}");
    let mut annotations: BTreeMap<String, u32> = BTreeMap::new();
    let mut configs = 0usize;
    let mut changes = 0usize;
    for m in xray.scan.items.iter().flat_map(|(_, s)| s.mods.iter()) {
        configs += m.configs.len();
        changes += m.change_count();
        for (name, count) in &m.other_annotations {
            *annotations.entry(name.clone()).or_default() += count;
        }
    }
    println!("configs {configs}, alterações {changes}, anotações {annotations:?}");
    let mut by_risk: BTreeMap<Risk, usize> = BTreeMap::new();
    for o in &xray.overlaps {
        *by_risk.entry(o.risk).or_default() += 1;
    }
    println!("sobreposições por risco: {by_risk:?}");
    let mut groups: BTreeMap<Vec<String>, (Risk, usize)> = BTreeMap::new();
    for o in xray.overlaps.iter().filter(|o| o.risk >= Risk::Medium) {
        let mut key: Vec<String> = o.participants.iter().map(|p| p.item.clone()).collect();
        key.sort();
        key.dedup();
        let entry = groups.entry(key).or_insert((Risk::Low, 0));
        entry.0 = entry.0.max(o.risk);
        entry.1 += 1;
    }
    let high = groups.values().filter(|(r, _)| *r == Risk::High).count();
    println!(
        "avisos (médio ou alto, por grupo de mods): {} (altos: {high})",
        groups.len()
    );
    for o in xray.overlaps.iter().filter(|o| o.risk >= Risk::Medium) {
        println!(
            "  {:?} (base {:?}, {:?}) {}#{} [{}] {:?} {:?}",
            o.risk,
            o.base,
            o.rule,
            if o.class_named.is_empty() {
                &o.target_class
            } else {
                &o.class_named
            },
            if o.method_named.is_empty() {
                &o.method
            } else {
                &o.method_named
            },
            o.participants
                .iter()
                .map(|p| format!("{}:{:?}@{}", p.mod_id, p.kind, p.priority))
                .collect::<Vec<_>>()
                .join(" + "),
            o.demoters,
            o.elevators,
        );
    }
}

/// Os pares de um `mixin-known-compatible.toml` (o formato é validado na `warden-diagnostics`).
fn known_pairs(text: &str) -> Vec<CuratedPair> {
    let Ok(value) = text.parse::<toml::Table>() else {
        return Vec::new();
    };
    let ids = |v: Option<&toml::Value>| -> Vec<String> {
        v.and_then(toml::Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default()
    };
    let mut out = Vec::new();
    for pair in value
        .get("par")
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
    {
        for first in ids(pair.get("primeiro")) {
            for second in ids(pair.get("segundo")) {
                out.push(CuratedPair {
                    first: first.clone(),
                    second,
                    label: "lista curada".into(),
                });
            }
        }
    }
    out
}
