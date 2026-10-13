//! Leitura dos jars (camadas A e B), embutidos, Sinytra Connector, jars ruins, cache e o
//! relatório de um mod, com jars sintéticos e configs reais do corpus do S-R5-1.

mod common;

use std::path::PathBuf;

use common::{JarBuilder, MixinClass, at, corpus, fabric_mod_json, injector, mixin_config};
use warden_jarmeta::{Limits, Loader};
use warden_mixin::cache::XrayCache;
use warden_mixin::model::{ChangeKind, IssueKind};
use warden_mixin::overlap::{CrossContext, Risk};
use warden_mixin::pack::{CacheSlot, PackJar, PackScan, PackXray, ScanStats, scan_pack};
use warden_mixin::scan::{scan_jar_bytes, scan_jar_file};

const TARGET: &str = "net/minecraft/world/level/Level";

fn class(name: &str) -> MixinClass {
    MixinClass::new(name, TARGET).method("i", "()V", injector("Inject", "tick", at("HEAD", "")))
}

fn scan(jar: &[u8], loader: Loader) -> warden_mixin::model::JarScan {
    scan_jar_bytes(jar, "x.jar", loader, &Limits::default())
}

/// Um jar com descritores dos três loaders e uma config para cada.
fn multi_loader_jar() -> Vec<u8> {
    let mods_toml = "modLoader=\"javafml\"\nloaderVersion=\"[47,)\"\nlicense=\"MIT\"\n[[mods]]\nmodId=\"multi\"\nversion=\"2.0\"\n";
    let neo_toml = format!("{mods_toml}[[mixins]]\nconfig=\"neo.mixins.json\"\n");
    JarBuilder::new()
        .file(
            "fabric.mod.json",
            fabric_mod_json("multi", "2.0", &["fabric.mixins.json"], &[], &[]),
        )
        .file("META-INF/mods.toml", mods_toml)
        .file("META-INF/neoforge.mods.toml", neo_toml)
        .file(
            "META-INF/MANIFEST.MF",
            "Manifest-Version: 1.0\r\nMixinConfigs: forge.mixins.json\r\n\r\n",
        )
        .file("fabric.mixins.json", mixin_config("x.f", &["F"], None))
        .file("forge.mixins.json", mixin_config("x.g", &["G"], None))
        .file("neo.mixins.json", mixin_config("x.n", &["N"], None))
        .class(&class("x/f/F"))
        .class(&class("x/g/G"))
        .class(&class("x/n/N"))
        .build()
}

#[test]
fn so_as_configs_do_loader_do_pack() {
    let jar = multi_loader_jar();
    let fabric = scan(&jar, Loader::Fabric);
    assert_eq!(fabric.mods[0].configs, vec!["fabric.mixins.json"]);
    assert!(!fabric.mods[0].connector);
    let forge = scan(&jar, Loader::Forge);
    assert_eq!(
        forge.mods[0].configs,
        vec!["forge.mixins.json"],
        "Forge: só o MixinConfigs do manifesto"
    );
    assert_eq!(forge.mods[0].mod_id, "multi");
    assert!(!forge.mods[0].connector);
    let neo = scan(&jar, Loader::NeoForge);
    assert_eq!(
        neo.mods[0].configs,
        vec!["neo.mixins.json", "forge.mixins.json"]
    );
    assert_eq!(neo.mods[0].classes.len(), 2);
}

#[test]
fn mod_fabric_em_pack_forge_e_do_connector() {
    let jar = JarBuilder::new()
        .file(
            "fabric.mod.json",
            fabric_mod_json("fab", "1.0", &["fab.mixins.json"], &[], &[]),
        )
        .file("fab.mixins.json", mixin_config("x.f", &["F"], None))
        .class(&class("x/f/F"))
        .build();
    for loader in [Loader::Forge, Loader::NeoForge] {
        let scanned = scan(&jar, loader);
        assert!(scanned.mods[0].connector, "{loader:?}");
        assert_eq!(scanned.mods[0].classes.len(), 1);
    }
    assert!(!scan(&jar, Loader::Fabric).mods[0].connector);
}

#[test]
fn config_de_servidor_fica_de_fora_e_config_ausente_nao_e_erro() {
    let descriptor = r#"{"schemaVersion":1,"id":"a","version":"1","mixins":["a.mixins.json",{"config":"a.server.mixins.json","environment":"server"},"outro.mixins.json"]}"#;
    let jar = JarBuilder::new()
        .file("fabric.mod.json", descriptor)
        .file("a.mixins.json", mixin_config("x.a", &["A"], None))
        .file("a.server.mixins.json", mixin_config("x.s", &["S"], None))
        .class(&class("x/a/A"))
        .class(&class("x/s/S"))
        .build();
    let scanned = scan(&jar, Loader::Fabric);
    let module = &scanned.mods[0];
    assert_eq!(module.configs, vec!["a.mixins.json"]);
    assert_eq!(module.classes.len(), 1);
    assert_eq!(module.issues.len(), 1);
    assert_eq!(module.issues[0].kind, IssueKind::MissingConfig);
}

#[test]
fn jar_e_classes_ruins_nao_derrubam_a_analise() {
    // Zip quebrado.
    let broken = scan(b"isto nao e um zip", Loader::Fabric);
    assert_eq!(broken.mods[0].issues[0].kind, IssueKind::UnreadableJar);
    assert!(broken.mods[0].mod_id.starts_with('?'));

    // Config com JSON inválido, classe que falta, classe corrompida, classe sem @Mixin e uma
    // classe ofuscada (nomes curtos sem sentido) que é mixin de verdade.
    let jar = JarBuilder::new()
        .file(
            "fabric.mod.json",
            fabric_mod_json("a", "1", &["bad.mixins.json", "a.mixins.json"], &[], &[]),
        )
        .file("bad.mixins.json", "{ não é json")
        .file(
            "a.mixins.json",
            mixin_config("a", &["Falta", "Ruim", "Comum", "a"], None),
        )
        .file(
            "a/Ruim.class",
            [0xCA, 0xFE, 0xBA, 0xBE, 0, 0, 0, 61, 0xFF, 0xFF, 1, 2, 3],
        )
        .class(&MixinClass::plain("a/Comum"))
        .class(&class("a/a"))
        .build();
    let scanned = scan(&jar, Loader::Fabric);
    let module = &scanned.mods[0];
    let kinds: Vec<IssueKind> = module.issues.iter().map(|i| i.kind).collect();
    assert!(kinds.contains(&IssueKind::InvalidConfig));
    assert!(kinds.contains(&IssueKind::MissingClass));
    assert!(kinds.contains(&IssueKind::UnreadableClass));
    assert!(kinds.contains(&IssueKind::NotAMixin));
    assert_eq!(module.classes.len(), 1, "a classe boa continua lida");
    assert_eq!(module.classes[0].name, "a/a");

    // O pack com o jar quebrado no meio continua.
    let dir = tempfile::tempdir().expect("temp");
    let good = dir.path().join("good.jar");
    let bad = dir.path().join("bad.jar");
    std::fs::write(&good, &jar).expect("jar");
    std::fs::write(&bad, b"PK\x03\x04 quebrado").expect("jar");
    let jars = vec![
        PackJar {
            item: "mods/bad.pw.toml".into(),
            path: bad,
        },
        PackJar {
            item: "mods/good.pw.toml".into(),
            path: good,
        },
        PackJar {
            item: "mods/sumiu.pw.toml".into(),
            path: dir.path().join("sumiu.jar"),
        },
    ];
    let pack = scan_pack(&jars, Loader::Fabric, None, &Limits::default());
    assert_eq!(pack.items.len(), 3);
    assert_eq!(pack.items[1].1.mods[0].classes.len(), 1);
}

#[test]
fn embutidos_repetidos_ficam_com_a_maior_versao_e_no_empate_com_o_agregador() {
    let module = |version: &str| {
        JarBuilder::new()
            .file(
                "fabric.mod.json",
                fabric_mod_json("lib", version, &["lib.mixins.json"], &[], &[]),
            )
            .file("lib.mixins.json", mixin_config("x.lib", &["L"], None))
            .class(&class("x/lib/L"))
            .build()
    };
    let holder = |id: &str, nested: &[(&str, Vec<u8>)]| {
        let names: Vec<String> = nested
            .iter()
            .map(|(n, _)| format!("META-INF/jars/{n}"))
            .collect();
        let names: Vec<&str> = names.iter().map(String::as_str).collect();
        let mut jar = JarBuilder::new().file(
            "fabric.mod.json",
            fabric_mod_json(id, "1", &[], &[], &names),
        );
        for (n, bytes) in nested {
            jar = jar.stored(&format!("META-INF/jars/{n}"), bytes);
        }
        scan(&jar.build(), Loader::Fabric)
    };
    let small = holder("coroutil", &[("lib.jar", module("1.0"))]);
    let big = holder(
        "fabric-api",
        &[("lib.jar", module("1.0")), ("other.jar", module("0.1"))],
    );
    let newer = holder("newer", &[("lib.jar", module("1.1"))]);
    assert_eq!(small.mods.len(), 2);
    assert_eq!(small.mods[1].path, "META-INF/jars/lib.jar");

    let overlap_owner = |items: Vec<(String, warden_mixin::model::JarScan)>| {
        // Um terceiro mod altera o mesmo ponto: o dono da cópia escolhida aparece na sobreposição.
        let third = JarBuilder::new()
            .file(
                "fabric.mod.json",
                fabric_mod_json("z", "1", &["z.mixins.json"], &[], &[]),
            )
            .file("z.mixins.json", mixin_config("x.z", &["Z"], None))
            .class(&class("x/z/Z"))
            .build();
        let mut items = items;
        items.push(("mods/z.pw.toml".into(), scan(&third, Loader::Fabric)));
        let x = PackXray::build(
            PackScan {
                items,
                stats: ScanStats::default(),
            },
            &CrossContext::default(),
        );
        let owners: Vec<String> = x.overlaps[0].items.clone();
        owners
    };
    let owners = overlap_owner(vec![
        ("mods/coroutil.pw.toml".into(), small.clone()),
        ("mods/fabric-api.pw.toml".into(), big),
    ]);
    assert_eq!(
        owners,
        vec!["mods/fabric-api.pw.toml", "mods/z.pw.toml"],
        "empate: o agregador"
    );
    let owners = overlap_owner(vec![
        ("mods/coroutil.pw.toml".into(), small),
        ("mods/newer.pw.toml".into(), newer),
    ]);
    assert_eq!(
        owners,
        vec!["mods/newer.pw.toml", "mods/z.pw.toml"],
        "versão maior"
    );
}

#[test]
fn configs_reais_do_corpus_sao_lidas() {
    // A config real do sodium-extra (S-R5-1) num jar com as classes que ela lista.
    let config = std::fs::read_to_string(corpus(
        "fabric-1.20.1/sodium-extra-0.5.9+mc1.20.1/sodium-extra.mixins.json",
    ))
    .expect("corpus");
    let json: serde_json::Value = serde_json::from_str(&config).expect("json");
    let package = json["package"].as_str().expect("package").replace('.', "/");
    let mut listed: Vec<String> = Vec::new();
    for key in ["mixins", "client"] {
        if let Some(list) = json[key].as_array() {
            listed.extend(list.iter().filter_map(|v| v.as_str()).map(str::to_owned));
        }
    }
    assert!(listed.len() > 10);
    let mut jar = JarBuilder::new()
        .file(
            "fabric.mod.json",
            fabric_mod_json(
                "sodium-extra",
                "0.5.9",
                &["sodium-extra.mixins.json"],
                &[],
                &[],
            ),
        )
        .file("sodium-extra.mixins.json", &config);
    for name in &listed {
        jar = jar.class(&class(&format!("{package}/{}", name.replace('.', "/"))));
    }
    let scanned = scan(&jar.build(), Loader::Fabric);
    assert_eq!(scanned.mods[0].classes.len(), listed.len());
    assert!(
        scanned.mods[0].issues.is_empty(),
        "{:?}",
        scanned.mods[0].issues
    );
    assert!(
        scanned.mods[0].classes.iter().all(|c| c.plugin),
        "a config real tem plugin"
    );
}

#[test]
fn cache_por_hash_reaproveita_e_percebe_mudancas() {
    let dir = tempfile::tempdir().expect("temp");
    let path = dir.path().join("a.jar");
    let jar = |n: &str| {
        JarBuilder::new()
            .file(
                "fabric.mod.json",
                fabric_mod_json("a", "1", &["a.mixins.json"], &[], &[]),
            )
            .file("a.mixins.json", mixin_config("x.a", &[n], None))
            .class(&class(&format!("x/a/{n}")))
            .build()
    };
    std::fs::write(&path, jar("A")).expect("jar");
    let cache = XrayCache::in_cache_dir(&dir.path().join("cache"));
    let slot = Some(CacheSlot {
        cache: &cache,
        key: "pack",
    });
    let jars = vec![PackJar {
        item: "mods/a.pw.toml".into(),
        path: path.clone(),
    }];
    let limits = Limits::default();

    let first = scan_pack(&jars, Loader::Fabric, slot, &limits);
    assert_eq!((first.stats.scanned, first.stats.hashed), (1, 1));
    assert!(
        dir.path().join("cache/xray/pack.bin").is_file(),
        "um arquivo binário só"
    );
    let second = scan_pack(&jars, Loader::Fabric, slot, &limits);
    assert_eq!(
        (second.stats.scanned, second.stats.hashed),
        (0, 0),
        "nada relido"
    );
    assert_eq!(first.items, second.items);

    // Mesmo conteúdo com data nova: refaz o hash, não relê.
    let bytes = std::fs::read(&path).expect("jar");
    std::thread::sleep(std::time::Duration::from_millis(20));
    std::fs::write(&path, &bytes).expect("jar");
    let touched = scan_pack(&jars, Loader::Fabric, slot, &limits);
    assert_eq!((touched.stats.scanned, touched.stats.hashed), (0, 1));

    // Conteúdo novo: relê.
    std::fs::write(&path, jar("B")).expect("jar");
    let changed = scan_pack(&jars, Loader::Fabric, slot, &limits);
    assert_eq!(changed.stats.scanned, 1);
    assert_eq!(changed.items[0].1.mods[0].classes[0].name, "x/a/B");

    // Outro loader não usa o cache do Fabric.
    let forge = scan_pack(&jars, Loader::Forge, slot, &limits);
    assert_eq!(forge.stats.scanned, 1);

    // Cache corrompido: relê tudo sem erro.
    std::fs::write(dir.path().join("cache/xray/pack.bin"), b"lixo").expect("cache");
    assert_eq!(
        scan_pack(&jars, Loader::Fabric, slot, &limits)
            .stats
            .scanned,
        1
    );
}

#[test]
fn relatorio_do_mod() {
    let call = "Lnet/minecraft/class_2;method_3()V";
    let a = JarBuilder::new()
        .file(
            "fabric.mod.json",
            fabric_mod_json("a", "1", &["a.mixins.json"], &[], &[]),
        )
        .file("a.mixins.json", mixin_config("x.a", &["A", "B"], None))
        .class(
            &MixinClass::new("x/a/A", TARGET)
                .method("r", "()V", injector("Redirect", "tick", at("INVOKE", call)))
                .method("h", "()V", injector("Inject", "save", at("HEAD", ""))),
        )
        .class(
            &MixinClass::new("x/a/B", "net/minecraft/Other")
                .overwrite("run")
                .priority(900),
        )
        .build();
    let b = JarBuilder::new()
        .file(
            "fabric.mod.json",
            fabric_mod_json("b", "1", &["b.mixins.json"], &[], &[]),
        )
        .file("b.mixins.json", mixin_config("x.b", &["A"], None))
        .class(&MixinClass::new("x/b/A", TARGET).method(
            "r",
            "()V",
            injector("Redirect", "tick", at("INVOKE", call)),
        ))
        .build();
    let pack = PackScan {
        items: vec![
            ("mods/a.pw.toml".into(), scan(&a, Loader::Fabric)),
            ("mods/b.pw.toml".into(), scan(&b, Loader::Fabric)),
        ],
        stats: ScanStats::default(),
    };
    let x = PackXray::build(pack, &CrossContext::default());
    let report = x.mod_report("mods/a.pw.toml").expect("relatório");
    assert_eq!(report.configs, vec!["a.mixins.json"]);
    assert_eq!(report.class_count, 2);
    assert_eq!(report.change_count, 3);
    assert_eq!(report.point_count, 3);
    assert_eq!(report.shared_point_count, 1);
    assert_eq!(report.priorities, vec![900, 1000]);
    assert_eq!(report.overlaps.len(), 1);
    assert_eq!(report.overlaps[0].risk, Risk::High);
    assert_eq!(
        report.points[0].others,
        vec!["mods/b.pw.toml"],
        "o ponto disputado vem primeiro"
    );
    assert_eq!(report.points[0].risk, Some(Risk::High));
    assert!(
        report
            .counts
            .iter()
            .any(|c| c.kind == ChangeKind::Overwrite && c.count == 1)
    );
    assert!(x.mod_report("mods/nao-existe.pw.toml").is_none());

    // Um mod sem mixins.
    let plain = JarBuilder::new()
        .file("fabric.mod.json", fabric_mod_json("p", "1", &[], &[], &[]))
        .build();
    let x = PackXray::build(
        PackScan {
            items: vec![("mods/p.pw.toml".into(), scan(&plain, Loader::Fabric))],
            stats: ScanStats::default(),
        },
        &CrossContext::default(),
    );
    let report = x.mod_report("mods/p.pw.toml").expect("relatório");
    assert_eq!((report.change_count, report.class_count), (0, 0));
}

#[test]
fn le_do_disco_igual_a_memoria() {
    let dir = tempfile::tempdir().expect("temp");
    let path: PathBuf = dir.path().join("m.jar");
    let jar = multi_loader_jar();
    std::fs::write(&path, &jar).expect("jar");
    let from_file = scan_jar_file(&path, Loader::NeoForge, &Limits::default());
    let from_bytes = scan_jar_bytes(&jar, "m.jar", Loader::NeoForge, &Limits::default());
    assert_eq!(from_file, from_bytes);
    assert_eq!(
        scan_jar_file(
            &dir.path().join("x.jar"),
            Loader::Fabric,
            &Limits::default()
        )
        .mods[0]
            .issues[0]
            .kind,
        IssueKind::UnreadableJar
    );
}
