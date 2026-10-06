//! Índices pacote → mod e config de mixin → mod com jars sintéticos: casos de borda que o
//! corpus real não cobre (D-05).

// Auxiliares de teste fora de funções #[test]: falhar com mensagem é o comportamento certo.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

mod common;

use std::fs;

use common::{JarBuilder, fabric_mod_json};
use proptest::prelude::*;
use warden_jarmeta::index::{
    INDEX_FORMAT, JarIndexCache, Ownership, PackIndex, PackJar, build_pack_index, index_jar_bytes,
};
use warden_jarmeta::{DescriptorKind, Limits, Loader};

fn index(item: &str, jar: &[u8]) -> (String, warden_jarmeta::index::JarIndex) {
    (
        item.to_owned(),
        index_jar_bytes(jar, &Limits::default()).unwrap(),
    )
}

fn fabric_with_mixins(id: &str, version: &str, mixins: &[&str], jars: &[&str]) -> String {
    let jars: Vec<String> = jars
        .iter()
        .map(|j| format!(r#"{{"file": "{j}"}}"#))
        .collect();
    let mixins: Vec<String> = mixins.iter().map(|m| format!("\"{m}\"")).collect();
    format!(
        r#"{{"schemaVersion": 1, "id": "{id}", "version": "{version}", "mixins": [{}], "jars": [{}]}}"#,
        mixins.join(","),
        jars.join(",")
    )
}

#[test]
fn pacotes_jars_embutidos_e_classe() {
    let nested = JarBuilder::new()
        .file(
            "fabric.mod.json",
            fabric_mod_json("biblioteca", "1.0.0", &[]),
        )
        .class("org/lib/Util.class", 61)
        .class("com/exemplo/api/Compartilhado.class", 61)
        .build();
    let top = JarBuilder::new()
        .file(
            "fabric.mod.json",
            fabric_mod_json("exemplo", "2.0.0", &["META-INF/jars/lib.jar"]),
        )
        .class("com/exemplo/Main.class", 61)
        .class("com/exemplo/Main$Interna.class", 61)
        .class("com/exemplo/api/Compartilhado.class", 61)
        .class("META-INF/versions/21/com/exemplo/v21/Nova.class", 65)
        .class("SemPacote.class", 61)
        .stored("META-INF/jars/lib.jar", nested)
        .build();
    let jar = index_jar_bytes(&top, &Limits::default()).unwrap();
    assert_eq!(jar.format, INDEX_FORMAT);
    assert_eq!(jar.sha256.len(), 64);
    assert_eq!(jar.jars.len(), 2);
    assert_eq!(
        jar.jars[0].packages,
        ["com.exemplo", "com.exemplo.api", "com.exemplo.v21"]
    );
    assert_eq!(jar.jars[1].path, "META-INF/jars/lib.jar");
    assert_eq!(jar.jars[1].parent, Some(0));
    assert_eq!(jar.jars[1].mods[0].id, "biblioteca");
    // O Forge e o NeoForge também leem `fabric.mod.json` (mod Fabric via Sinytra Connector).
    assert_eq!(
        jar.jars[1].mods[0].loaders,
        [
            Loader::Fabric,
            Loader::Quilt,
            Loader::Forge,
            Loader::NeoForge
        ]
    );

    let pack = PackIndex::new(
        vec![("mods/exemplo.pw.toml".into(), jar)],
        Some(Loader::Fabric),
    );
    let owner = pack.class_owner("com.exemplo.Main$Interna").unwrap();
    assert_eq!(owner.owner().unwrap().mod_id, Some("exemplo"));
    assert_eq!(
        pack.class_owner("org/lib/Util")
            .unwrap()
            .owner()
            .unwrap()
            .mod_id,
        Some("biblioteca")
    );
    assert_eq!(pack.class_owner("SemPacote"), None);
    assert_eq!(pack.class_owner("net.minecraft.client.Minecraft"), None);
    // Mesmo pacote no jar de cima e no embutido (mods diferentes do mesmo item): o jar de topo.
    let Some(Ownership::Ambiguous { chosen, candidates }) = pack.package_owner("com/exemplo/api")
    else {
        panic!("deveria ser ambíguo");
    };
    assert_eq!(candidates.len(), 2);
    let chosen = chosen.unwrap();
    assert_eq!((chosen.jar, chosen.mod_id), ("", Some("exemplo")));
    assert_eq!(pack.ambiguous_packages().len(), 1);
}

#[test]
fn copias_do_mesmo_mod_ficam_com_a_versao_maior() {
    let module = |version: &str| {
        JarBuilder::new()
            .file(
                "fabric.mod.json",
                fabric_with_mixins("modulo-base", version, &["modulo.mixins.json"], &[]),
            )
            .file("modulo.mixins.json", "{}")
            .class("net/base/Api.class", 61)
            .build()
    };
    let carrier = |id: &str, version: &str| {
        JarBuilder::new()
            .file(
                "fabric.mod.json",
                fabric_mod_json(id, "1.0", &["META-INF/jars/base.jar"]),
            )
            .class(&format!("{id}/Main.class"), 61)
            .stored("META-INF/jars/base.jar", module(version))
            .build()
    };
    let pack = PackIndex::new(
        vec![
            index("mods/a.pw.toml", &carrier("a", "0.4.31+1802ada577")),
            index("mods/b.pw.toml", &carrier("b", "0.4.42+d1308ded19")),
            index("mods/c.pw.toml", &carrier("c", "0.4.42+d1308ded19")),
        ],
        Some(Loader::Fabric),
    );
    let Some(Ownership::Unique(owner)) = pack.package_owner("net.base") else {
        panic!("cópias do mesmo mod não são ambiguidade");
    };
    // A maior versão; no empate, a primeira.
    assert_eq!(owner.item, "mods/b.pw.toml");
    assert_eq!(owner.mod_id, Some("modulo-base"));
    assert!(pack.ambiguous_packages().is_empty());
    let mixin = pack.mixin_owner("modulo.mixins.json").unwrap();
    assert_eq!(mixin.owner.item, "mods/b.pw.toml");
    assert_eq!(pack.mixin_owners("modulo.mixins.json").len(), 3);
}

#[test]
fn mods_diferentes_em_itens_diferentes_nao_tem_dono() {
    let jar = |id: &str| {
        JarBuilder::new()
            .file(
                "fabric.mod.json",
                fabric_with_mixins(id, "1.0", &["comum.mixins.json"], &[]),
            )
            .class("shaded/gson/Gson.class", 52)
            .build()
    };
    let pack = PackIndex::new(
        vec![
            index("mods/a.pw.toml", &jar("a")),
            index("mods/b.pw.toml", &jar("b")),
        ],
        Some(Loader::Fabric),
    );
    let ownership = pack.package_owner("shaded.gson").unwrap();
    assert!(matches!(
        &ownership,
        Ownership::Ambiguous { chosen: None, candidates } if candidates.len() == 2
    ));
    assert_eq!(ownership.owner(), None);
    assert_eq!(pack.mixin_owner("comum.mixins.json"), None);
    assert_eq!(pack.mixin_owners("comum.mixins.json").len(), 2);
}

#[test]
fn configs_de_mixin_de_cada_descritor() {
    let neoforge = r#"
modLoader = "javafml"
loaderVersion = "[1,)"
license = "MIT"
[[mods]]
modId = "epicfight"
version = "21.17.3.1"
[[mixins]]
config = "epicfight.mixins.json"
[[mixins]]
config = "ausente.mixins.json"
"#;
    let jar = JarBuilder::new()
        .file("META-INF/neoforge.mods.toml", neoforge)
        .file("epicfight.mixins.json", "{}")
        .class("yesman/epicfight/main/EpicFightMod.class", 65)
        .build();
    let forge = JarBuilder::new()
        .file(
            "META-INF/mods.toml",
            "modLoader=\"javafml\"\nloaderVersion=\"[47,)\"\nlicense=\"x\"\n[[mods]]\nmodId=\"antigo\"\n",
        )
        .file(
            "META-INF/MANIFEST.MF",
            "Manifest-Version: 1.0\r\nMixinConfigs: mixins.antigo.json\r\n",
        )
        .file("mixins.antigo.json", "{}")
        .build();
    let quilt = JarBuilder::new()
        .file(
            "quilt.mod.json",
            r#"{"schema_version": 1, "quilt_loader": {"group": "x", "id": "q", "version": "1.0"}, "mixin": "q.mixins.json"}"#,
        )
        .file("q.mixins.json", "{}")
        .build();

    let pack = PackIndex::new(
        vec![
            index("mods/epic-fight.pw.toml", &jar),
            index("mods/antigo.pw.toml", &forge),
            index("mods/q.pw.toml", &quilt),
        ],
        None,
    );
    let epic = pack.mixin_owner("epicfight.mixins.json").unwrap();
    assert_eq!(epic.owner.item, "mods/epic-fight.pw.toml");
    assert_eq!(epic.owner.mod_id, Some("epicfight"));
    assert_eq!(epic.declared_in, DescriptorKind::NeoForgeModsToml);
    assert!(epic.present);
    let absent = pack.mixin_owner("ausente.mixins.json").unwrap();
    assert!(!absent.present);
    assert_eq!(absent.owner.mod_id, Some("epicfight"));
    let manifest = pack.mixin_owner(" mixins.antigo.json ").unwrap();
    assert_eq!(manifest.declared_in, DescriptorKind::Manifest);
    assert_eq!(manifest.owner.mod_id, Some("antigo"));
    assert_eq!(
        pack.mixin_owner("q.mixins.json").unwrap().owner.mod_id,
        Some("q")
    );
    assert_eq!(pack.mixin_owner("nao-existe.mixins.json"), None);
    assert_eq!(pack.mixin_configs().count(), 4);
}

#[test]
fn jar_multi_loader_usa_o_descritor_do_loader_do_pack() {
    let jar = JarBuilder::new()
        .file(
            "fabric.mod.json",
            fabric_with_mixins("multi", "1.0", &["multi.fabric.mixins.json"], &[]),
        )
        .file(
            "META-INF/mods.toml",
            "modLoader=\"javafml\"\nloaderVersion=\"[47,)\"\nlicense=\"x\"\n[[mods]]\nmodId=\"multi_forge\"\n",
        )
        .file(
            "META-INF/MANIFEST.MF",
            "Manifest-Version: 1.0\r\nMixinConfigs: multi.forge.mixins.json\r\n",
        )
        .class("multi/Main.class", 61)
        .build();
    let forge = PackIndex::new(vec![index("mods/multi.pw.toml", &jar)], Some(Loader::Forge));
    assert_eq!(
        forge
            .class_owner("multi.Main")
            .unwrap()
            .owner()
            .unwrap()
            .mod_id,
        Some("multi_forge")
    );
    assert_eq!(
        forge
            .mixin_owner("multi.forge.mixins.json")
            .unwrap()
            .owner
            .mod_id,
        Some("multi_forge")
    );
    let fabric = PackIndex::new(
        vec![index("mods/multi.pw.toml", &jar)],
        Some(Loader::Fabric),
    );
    assert_eq!(
        fabric
            .class_owner("multi.Main")
            .unwrap()
            .owner()
            .unwrap()
            .mod_id,
        Some("multi")
    );
    // A config que só o Fabric declara continua com dono no Forge (não há outra).
    assert_eq!(
        forge
            .mixin_owner("multi.fabric.mixins.json")
            .unwrap()
            .owner
            .mod_id,
        Some("multi")
    );
}

#[test]
fn biblioteca_sem_mod_fica_com_o_mod_do_item_e_artefato_identifica_copias() {
    let library = |version: &str| {
        JarBuilder::new()
            .file(
                "META-INF/MANIFEST.MF",
                "Manifest-Version: 1.0\r\nMixinConfigs: lib.init.mixins.json\r\nFMLModType: GAMELIBRARY\r\n",
            )
            .file("lib.init.mixins.json", "{}")
            .class(&format!("org/lib/v{}/X.class", version.len()), 61)
            .class("org/lib/Comum.class", 61)
            .build()
    };
    let jarjar = |version: &str| {
        format!(
            r#"{{"jars": [{{"identifier": {{"group": "org.lib", "artifact": "lib"}}, "version": {{"range": "[1,)", "artifactVersion": "{version}"}}, "path": "META-INF/jarjar/lib.jar", "isObfuscated": false}}]}}"#
        )
    };
    let host = |id: &str, version: &str| {
        JarBuilder::new()
            .file(
                "META-INF/neoforge.mods.toml",
                format!(
                    "modLoader=\"javafml\"\nloaderVersion=\"[1,)\"\nlicense=\"x\"\n[[mods]]\nmodId=\"{id}\"\n"
                ),
            )
            .file("META-INF/jarjar/metadata.json", jarjar(version))
            .stored("META-INF/jarjar/lib.jar", library(version))
            .build()
    };
    let pack = PackIndex::new(
        vec![
            index("mods/a.pw.toml", &host("a", "1.2.0")),
            index("mods/b.pw.toml", &host("b", "1.10.0")),
        ],
        Some(Loader::NeoForge),
    );
    let Some(Ownership::Unique(owner)) = pack.package_owner("org.lib") else {
        panic!("o artefato do jarjar identifica as cópias");
    };
    assert_eq!(owner.item, "mods/b.pw.toml");
    assert_eq!(owner.jar, "META-INF/jarjar/lib.jar");
    assert_eq!(owner.mod_id, Some("b"));
    let mixin = pack.mixin_owner("lib.init.mixins.json").unwrap();
    assert_eq!(mixin.owner.item, "mods/b.pw.toml");
    assert_eq!(mixin.owner.mod_id, Some("b"));
}

#[test]
fn montagem_com_falhas_e_cache_que_nao_grava() {
    let dir = tempfile::tempdir().unwrap();
    let good = dir.path().join("bom.jar");
    fs::write(
        &good,
        JarBuilder::new()
            .file("fabric.mod.json", fabric_mod_json("bom", "1.0", &[]))
            .class("bom/Main.class", 61)
            .build(),
    )
    .unwrap();
    let broken = dir.path().join("quebrado.jar");
    fs::write(&broken, b"isto nao e um zip").unwrap();
    let jars = vec![
        PackJar::new("mods/bom.pw.toml", &good),
        PackJar::new("mods/quebrado.pw.toml", &broken),
        PackJar::new("mods/sumiu.pw.toml", dir.path().join("sumiu.jar")),
    ];
    // A pasta do cache é um arquivo: gravar falha, e o índice sai mesmo assim.
    let blocked = dir.path().join("bloqueado");
    fs::write(&blocked, b"").unwrap();
    let cache = JarIndexCache::new(blocked);
    let build = build_pack_index(
        &jars,
        Some(Loader::Fabric),
        Some(&cache),
        &Limits::default(),
    );
    assert_eq!(build.stats.jars, 3);
    assert_eq!(build.stats.jars_opened, 1);
    assert_eq!(build.stats.cache_write_failures, 1);
    let mut failed: Vec<&str> = build.failures.iter().map(|f| f.item.as_str()).collect();
    failed.sort_unstable();
    assert_eq!(failed, ["mods/quebrado.pw.toml", "mods/sumiu.pw.toml"]);
    assert_eq!(
        build
            .index
            .class_owner("bom.Main")
            .unwrap()
            .owner()
            .unwrap()
            .item,
        "mods/bom.pw.toml"
    );

    // Sem cache, sem hash: lê direto.
    let plain = build_pack_index(&jars[..1], None, None, &Limits::default());
    assert_eq!(plain.stats.jars_opened, 1);
    assert_eq!(plain.stats.cache_hits, 0);
    assert!(
        build_pack_index(&[], None, None, &Limits::default())
            .index
            .items()
            .is_empty()
    );
}

#[test]
fn limites_valem_para_o_indice() {
    let inner = JarBuilder::new()
        .file("fabric.mod.json", fabric_mod_json("fundo", "1.0", &[]))
        .class("fundo/X.class", 61)
        .build();
    let middle = JarBuilder::new()
        .file(
            "fabric.mod.json",
            fabric_mod_json("meio", "1.0", &["META-INF/jars/fundo.jar"]),
        )
        .stored("META-INF/jars/fundo.jar", inner)
        .build();
    let top = JarBuilder::new()
        .file(
            "fabric.mod.json",
            fabric_mod_json("topo", "1.0", &["META-INF/jars/meio.jar"]),
        )
        .stored("META-INF/jars/meio.jar", middle)
        .build();
    let shallow = Limits {
        max_depth: 1,
        ..Limits::default()
    };
    let index = index_jar_bytes(&top, &shallow).unwrap();
    assert_eq!(
        index.jars.len(),
        2,
        "o embutido além da profundidade fica fora"
    );
    let deep = index_jar_bytes(&top, &Limits::default()).unwrap();
    assert_eq!(deep.jars.len(), 3);
    assert_eq!(
        deep.jars[2].path,
        "META-INF/jars/meio.jar!/META-INF/jars/fundo.jar"
    );
    assert_eq!(deep.jars[2].parent, Some(1));

    let tiny = Limits {
        max_file_bytes: 10,
        ..Limits::default()
    };
    assert!(index_jar_bytes(&top, &tiny).is_err());
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("grande.jar");
    fs::write(&path, &top).unwrap();
    assert!(warden_jarmeta::index::index_jar_file(&path, &tiny).is_err());
    assert_eq!(
        warden_jarmeta::index::index_jar_file(&path, &Limits::default()).unwrap(),
        deep
    );
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn bytes_aleatorios_nao_derrubam(bytes in proptest::collection::vec(any::<u8>(), 0..2048)) {
        let _ = index_jar_bytes(&bytes, &Limits::default());
    }

    #[test]
    fn nomes_aleatorios_viram_pacotes_validos(
        names in proptest::collection::vec("[a-zA-Z0-9/$._-]{0,40}", 0..40),
        configs in proptest::collection::vec("[a-z./ ]{0,20}", 0..5),
    ) {
        let mut jar = JarBuilder::new().file(
            "fabric.mod.json",
            fabric_with_mixins("aleatorio", "1.0", &configs.iter().map(String::as_str).collect::<Vec<_>>(), &[]),
        );
        let mut unique = names.clone();
        unique.sort();
        unique.dedup();
        for name in &unique {
            if !name.is_empty() && name != "fabric.mod.json" && !name.ends_with('/') {
                jar = jar.file(name, b"x");
            }
        }
        let Ok(index) = index_jar_bytes(&jar.build(), &Limits::default()) else {
            return Ok(());
        };
        for package in &index.jars[0].packages {
            prop_assert!(!package.is_empty());
            prop_assert!(!package.contains('/'));
            prop_assert!(!package.starts_with('.') && !package.ends_with('.'));
            prop_assert!(!package.starts_with("META-INF"));
        }
        let pack = PackIndex::new(vec![("x".into(), index)], None);
        for config in pack.mixin_configs().map(str::to_owned).collect::<Vec<_>>() {
            prop_assert!(pack.mixin_owner(&config).is_some());
        }
    }
}
