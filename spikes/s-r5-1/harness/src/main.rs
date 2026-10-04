//! Spike S-R5-1: mede o protótipo da R5A refeito e o intermed (como biblioteca)
//! nos mesmos jars. Código descartável.
//!
//! Uso:
//!   s-r5-1-harness proto <fabric|forge|neoforge> <pasta-mods> <saida.json> [--threads N] [--cache PASTA] [--known ARQ]
//!   s-r5-1-harness intermed <pasta-mods> <saida.json> [--cache PASTA]

mod proto;

use std::path::{Path, PathBuf};
use std::time::Instant;

use serde_json::json;

fn peak_working_set_mb() -> f64 {
    use windows_sys::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
    use windows_sys::Win32::System::Threading::GetCurrentProcess;
    let mut c: PROCESS_MEMORY_COUNTERS = unsafe { std::mem::zeroed() };
    c.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
    // SAFETY: estrutura do tamanho certo, processo atual.
    let ok = unsafe { GetProcessMemoryInfo(GetCurrentProcess(), &mut c, c.cb) };
    if ok == 0 { -1.0 } else { c.PeakWorkingSetSize as f64 / (1024.0 * 1024.0) }
}

fn opt(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1).cloned())
}

fn load_known(path: Option<String>) -> Vec<(String, String)> {
    let Some(p) = path else { return Vec::new() };
    let text = std::fs::read_to_string(p).expect("lista curada");
    let v: toml::Value = toml::from_str(&text).expect("toml da lista curada");
    v.get("pair")
        .and_then(|x| x.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|e| {
                    let m = e.get("mods")?.as_array()?;
                    Some((m.first()?.as_str()?.to_string(), m.get(1)?.as_str()?.to_string()))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("proto") => run_proto(&args),
        Some("intermed") => run_intermed(&args),
        Some("dump") => {
            // dump <loader> <jar> [filtro]: todas as alterações de um jar (e aninhados).
            let loader = proto::Loader::parse(&args[2]).expect("loader");
            let f = std::fs::File::open(&args[3]).expect("jar");
            let mut units = Vec::new();
            proto::scan_jar_bytes(std::io::BufReader::new(f), &args[3], false, loader, &mut units);
            let filt = args.get(4).cloned().unwrap_or_default();
            for u in &units {
                for c in &u.classes {
                    for ch in &c.changes {
                        let line = format!("{} {} {} {}#{} {}:{}", u.modid, c.name, ch.kind, ch.target_class, ch.method, ch.at_value, ch.at_target);
                        if line.contains(&filt) {
                            println!("{line}");
                        }
                    }
                }
            }
        }
        _ => {
            eprintln!("uso: proto <loader> <mods> <saida.json> | intermed <mods> <saida.json>");
            std::process::exit(2);
        }
    }
}

fn run_proto(args: &[String]) {
    let loader = proto::Loader::parse(&args[2]).expect("loader");
    let dir = PathBuf::from(&args[3]);
    let out = PathBuf::from(&args[4]);
    let threads: usize = opt(args, "--threads").and_then(|s| s.parse().ok()).unwrap_or(1);
    let cache = opt(args, "--cache").map(PathBuf::from);
    if let Some(c) = &cache {
        std::fs::create_dir_all(c).unwrap();
    }
    let known = load_known(opt(args, "--known"));
    let config_dir = opt(args, "--config-dir").map(PathBuf::from);

    let t0 = Instant::now();
    let units = proto::scan_dir(&dir, loader, threads, cache.as_deref());
    let t_scan = t0.elapsed();
    let t1 = Instant::now();
    let none = std::collections::HashSet::new();
    let (ovs, shared_classes, _) = proto::overlaps(&units, &known, &proto::Mode { refined: false, disabled: &none });
    let t_cross = t1.elapsed();
    let total = t0.elapsed();
    // Modo refinado: opções desligadas pelos descritores dos mods e pelas configs do pack.
    let mut disabled: std::collections::HashSet<String> = units.iter().flat_map(|u| u.disables.iter().cloned()).collect();
    let from_descriptors = disabled.len();
    if let Some(d) = &config_dir {
        disabled.extend(proto::disabled_from_config_dir(d));
    }
    let t2 = Instant::now();
    let (ovs_ref, _, removed) = proto::overlaps(&units, &known, &proto::Mode { refined: true, disabled: &disabled });
    let t_cross_ref = t2.elapsed();

    let mut counts: std::collections::BTreeMap<String, u64> = Default::default();
    for u in &units {
        for (k, v) in &u.annotation_counts {
            *counts.entry(k.clone()).or_default() += *v as u64;
        }
    }
    let n_top = units.iter().filter(|u| !u.nested).count();
    let n_nested = units.len() - n_top;
    let n_configs: usize = units.iter().map(|u| u.configs.len()).sum();
    let n_classes: usize = units.iter().map(|u| u.classes.len()).sum();
    let n_changes: usize = units.iter().flat_map(|u| &u.classes).map(|c| c.changes.len()).sum();
    let errors: Vec<String> = units
        .iter()
        .flat_map(|u| u.errors.iter().map(move |e| format!("{}: {e}", u.jar)))
        .collect();
    let mixinsquared = units.iter().any(|u| u.modid == "mixinsquared");
    let tally = |f: &dyn Fn(&proto::Overlap) -> proto::Risk| {
        let mut m = std::collections::BTreeMap::new();
        for o in &ovs {
            *m.entry(format!("{:?}", f(o))).or_insert(0u32) += 1;
        }
        m
    };
    let summary = json!({
        "ferramenta": "prototipo-r5a",
        "pasta": dir.display().to_string(),
        "threads": threads,
        "cache": cache.as_ref().map(|c| c.display().to_string()),
        "tempo_ms": { "leitura": t_scan.as_secs_f64() * 1e3, "cruzamento": t_cross.as_secs_f64() * 1e3, "total": total.as_secs_f64() * 1e3 },
        "pico_memoria_mb": peak_working_set_mb(),
        "jars_topo": n_top,
        "jars_aninhados": n_nested,
        "configs": n_configs,
        "classes_de_mixin": n_classes,
        "alteracoes": n_changes,
        "anotacoes": counts,
        "erros": errors.len(),
        "mixinsquared_presente": mixinsquared,
        "classes_alteradas_por_2_mais": shared_classes,
        "metodos_alterados_por_2_mais": ovs.len(),
        "risco_base": tally(&|o| o.base),
        "risco_com_rebaixadores": tally(&|o| o.final_one),
        "risco_rebaixadores_somados": tally(&|o| o.final_stacked),
        "medio_ou_alto_com_require": ovs.iter().filter(|o| o.final_one >= proto::Risk::Medio && o.require_elevator).count(),
        "refinado": {
            "tempo_cruzamento_ms": t_cross_ref.as_secs_f64() * 1e3,
            "opcoes_desligadas": { "por_descritores": from_descriptors, "total": disabled.len(), "lista": disabled.iter().collect::<std::collections::BTreeSet<_>>() },
            "classes_de_mixin_removidas_por_opcao": removed,
            "metodos_alterados_por_2_mais": ovs_ref.len(),
            "risco_base": tally_of(&ovs_ref, |o| o.base),
            "risco_com_rebaixadores": tally_of(&ovs_ref, |o| o.final_one),
        },
    });
    let doc = json!({
        "resumo": summary,
        "erros": errors,
        "sobreposicoes": ovs.iter().filter(|o| o.base >= proto::Risk::Medio).collect::<Vec<_>>(),
        "sobreposicoes_refinadas": ovs_ref.iter().filter(|o| o.base >= proto::Risk::Medio).collect::<Vec<_>>(),
        "inventario": units.iter().filter(|u| !u.classes.is_empty()).map(|u| json!({
            "modid": u.modid, "jar": u.jar, "configs": u.configs.len(), "classes": u.classes.len(),
            "alteracoes": u.classes.iter().map(|c| c.changes.len()).sum::<usize>(),
            "plugin": u.classes.iter().any(|c| c.plugin),
            "nomes": u.classes.iter().map(|c| &c.name).collect::<Vec<_>>(),
            "config_nomes": &u.configs,
        })).collect::<Vec<_>>(),
    });
    std::fs::write(&out, serde_json::to_vec_pretty(&doc).unwrap()).unwrap();
    println!("{}", serde_json::to_string_pretty(&summary).unwrap());
}

fn run_intermed(args: &[String]) {
    let dir = PathBuf::from(&args[2]);
    let out = PathBuf::from(&args[3]);
    let cache = opt(args, "--cache").map(PathBuf::from);
    let jc = cache
        .as_ref()
        .map(|c| intermed_doctor_core::JarCache::new(true, Some(c.clone())).expect("cache do intermed"));

    let t0 = Instant::now();
    let scan = intermed_mixin_intel::scan_mods_dir_with_cache(Path::new(&dir), jc.as_ref()).expect("scan");
    let total = t0.elapsed();

    let cross: Vec<_> = scan.compositions.iter().filter(|c| c.cross_mod).collect();
    let mut by_class = std::collections::BTreeMap::new();
    for c in &cross {
        *by_class.entry(format!("{:?}", c.classification)).or_insert(0u32) += 1;
    }
    let mut edges = std::collections::BTreeMap::new();
    for e in &scan.conflict_edges {
        *edges.entry(format!("{:?}", e.edge_type)).or_insert(0u32) += 1;
    }
    let summary = json!({
        "ferramenta": "intermed-mixin-intel@7d90f2e",
        "pasta": dir.display().to_string(),
        "cache": cache.as_ref().map(|c| c.display().to_string()),
        "tempo_ms": total.as_secs_f64() * 1e3,
        "pico_memoria_mb": peak_working_set_mb(),
        "configs": scan.configs.len(),
        "configs_descobertas": scan.configs_discovered,
        "classes_de_mixin": scan.classes.len(),
        "sites": scan.application_sites.len(),
        "overlaps": scan.overlaps.len(),
        "high_risk_overwrites": scan.high_risk_overwrites.len(),
        "interactions": scan.interactions.len(),
        "conflict_edges": scan.conflict_edges.len(),
        "conflict_edges_por_tipo": edges,
        "priority_conflicts": scan.priority_conflicts.len(),
        "compositions_cross_mod": cross.len(),
        "compositions_cross_mod_por_classe": by_class,
        "risk_clusters": scan.risk_clusters.len(),
        "apply_failures": scan.apply_failures.len(),
        "falhas": scan.failures.len(),
    });
    let doc = json!({
        "resumo": summary,
        "falhas": scan.failures,
        "compositions_cross_mod": cross,
        "conflict_edges": scan.conflict_edges,
        "high_risk_overwrites": scan.high_risk_overwrites,
        "overlaps": scan.overlaps,
        "classes": scan.classes.iter().map(|c| json!({"mod": c.mod_id, "archive": c.archive, "class": c.class_name, "config": c.config, "targets": c.targets})).collect::<Vec<_>>(),
    });
    std::fs::write(&out, serde_json::to_vec_pretty(&doc).unwrap()).unwrap();
    println!("{}", serde_json::to_string_pretty(&summary).unwrap());
}

fn tally_of(ovs: &[proto::Overlap], f: impl Fn(&proto::Overlap) -> proto::Risk) -> std::collections::BTreeMap<String, u32> {
    let mut m = std::collections::BTreeMap::new();
    for o in ovs {
        *m.entry(format!("{:?}", f(o))).or_insert(0u32) += 1;
    }
    m
}
