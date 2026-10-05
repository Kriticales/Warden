//! Leitura de jars sintéticos: descritores, jars embutidos, versão de classe e limites.

// Auxiliares de teste fora de funções #[test]: falhar com mensagem é o comportamento certo.
#![allow(clippy::expect_used, clippy::panic)]

mod common;

use common::{JarBuilder, fabric_mod_json};
use warden_core::{CoreErrorCode, DomainCode, DomainError};
use warden_jarmeta::{
    ClassVersion, DependencyKind, DescriptorKind, Environment, Error, JarmetaErrorCode, Limits,
    Loader, VersionRange, WarningCode, read_jar_bytes, read_jar_file,
};

fn read(bytes: &[u8]) -> warden_jarmeta::JarMetadata {
    read_jar_bytes(bytes, &Limits::default()).expect("jar válido")
}

#[test]
fn jar_fabric_com_jars_embutidos_em_cascata() {
    let deep = JarBuilder::new()
        .file("fabric.mod.json", fabric_mod_json("deep", "1.0.0", &[]))
        .build();
    let api_module = JarBuilder::new()
        .file(
            "fabric.mod.json",
            fabric_mod_json("fabric-api-base", "0.4.31", &["META-INF/jars/deep.jar"]),
        )
        .stored("META-INF/jars/deep.jar", &deep)
        .build();
    let top = JarBuilder::new()
        .file(
            "fabric.mod.json",
            r#"{"schemaVersion": 1, "id": "fabric-api", "version": "0.92.2+1.20.1",
                "provides": ["fabric"], "depends": {"fabricloader": ">=0.14.21", "minecraft": "~1.20.1"},
                "jars": [{"file": "META-INF/jars/base.jar"}, {"file": "/META-INF/jars/base.jar"},
                         {"file": "META-INF/jars/ausente.jar"}]}"#,
        )
        .stored("META-INF/jars/base.jar", &api_module)
        .class("net/fabricmc/A.class", 61)
        .class("net/fabricmc/B.class", 52)
        .class("META-INF/versions/21/C.class", 65)
        .file("META-INF/MANIFEST.MF", "Manifest-Version: 1.0\n")
        .build();
    let meta = read(&top);
    assert_eq!(
        meta.descriptors,
        [DescriptorKind::FabricModJson, DescriptorKind::Manifest]
    );
    let module = &meta.mods[0];
    assert_eq!(module.id, "fabric-api");
    assert_eq!(module.provides, ["fabric"]);
    assert_eq!(module.minecraft, Some(VersionRange::fabric("~1.20.1")));
    // Empate entre 61 e 52 (uma classe cada): fica a maior; a de META-INF/versions não conta.
    assert_eq!(
        meta.class_version,
        Some(ClassVersion {
            major: 61,
            minor: 0
        })
    );
    assert_eq!(meta.max_class_version, None);
    assert_eq!(
        meta.class_version
            .and_then(ClassVersion::java_feature_version),
        Some(17)
    );
    assert_eq!(
        meta.nested.len(),
        2,
        "caminho repetido com '/' conta uma vez"
    );
    assert_eq!(meta.nested[0].declared_by, DescriptorKind::FabricModJson);
    let base = meta.nested[0].metadata.as_ref().expect("base lido");
    assert_eq!(base.mods[0].id, "fabric-api-base");
    let deep_meta = base.nested[0].metadata.as_ref().expect("deep lido");
    assert_eq!(deep_meta.mods[0].id, "deep");
    assert!(meta.nested[1].metadata.is_none());
    assert_eq!(meta.warnings.len(), 1);
    assert_eq!(meta.warnings[0].code, WarningCode::MissingNestedJar);
    assert_eq!(meta.warnings[0].path, "META-INF/jars/ausente.jar");

    let walked: Vec<String> = meta.walk().into_iter().map(|(path, _)| path).collect();
    assert_eq!(
        walked,
        [
            "",
            "META-INF/jars/base.jar",
            "META-INF/jars/base.jar!/META-INF/jars/deep.jar"
        ]
    );
    assert_eq!(meta.mods_for_loader(Loader::Fabric).len(), 1);
    assert_eq!(
        meta.mods_for_loader(Loader::Quilt).len(),
        1,
        "o Quilt lê fabric.mod.json"
    );
    assert_eq!(
        meta.mods_for_loader(Loader::NeoForge)[0].id,
        "fabric-api",
        "via Connector"
    );
    assert!(!meta.is_multi_loader());
}

#[test]
fn jar_multi_loader_e_escolha_por_loader() {
    let jar = JarBuilder::new()
        .file("fabric.mod.json", fabric_mod_json("badoptimizations", "2.4.1", &[]))
        .file(
            "META-INF/mods.toml",
            "modLoader = \"javafml\"\nloaderVersion = \"[1,)\"\nlicense = \"MIT\"\n\
             [[mods]]\nmodId = \"badoptimizations\"\nversion = \"${file.jarVersion}\"\n\
             [[dependencies.badoptimizations]]\nmodId = \"forge\"\nmandatory = true\nversionRange = \"[47,)\"\n",
        )
        .file(
            "META-INF/neoforge.mods.toml",
            "modLoader = \"javafml\"\nloaderVersion = \"[1,)\"\nlicense = \"MIT\"\n\
             [[mods]]\nmodId = \"badoptimizations\"\nversion = \"2.4.1\"\n",
        )
        .file(
            "META-INF/MANIFEST.MF",
            "Manifest-Version: 1.0\r\nImplementation-Version: 2.4.1\r\nMixinConfigs: forge-badoptimizations.mixins.json\r\n",
        )
        .build();
    let meta = read(&jar);
    assert!(meta.is_multi_loader());
    assert_eq!(
        meta.descriptors,
        [
            DescriptorKind::FabricModJson,
            DescriptorKind::NeoForgeModsToml,
            DescriptorKind::ModsToml,
            DescriptorKind::Manifest
        ]
    );
    let forge = meta.mods_for_loader(Loader::Forge);
    assert_eq!(forge[0].source, DescriptorKind::ModsToml);
    assert_eq!(
        forge[0].version.as_deref(),
        Some("2.4.1"),
        "${{file.jarVersion}} resolvido"
    );
    assert_eq!(forge[0].loader_version, Some(VersionRange::maven("[47,)")));
    assert_eq!(
        meta.mods_for_loader(Loader::NeoForge)[0].source,
        DescriptorKind::NeoForgeModsToml
    );
    assert_eq!(
        meta.mods_for_loader(Loader::Fabric)[0].loader,
        Loader::Fabric
    );
    assert_eq!(
        meta.manifest.as_ref().map(|m| m.mixin_configs.clone()),
        Some(vec!["forge-badoptimizations.mixins.json".to_owned()])
    );
    assert!(meta.warnings.is_empty(), "{:?}", meta.warnings);
}

#[test]
fn jarjar_do_forge_e_contained_deps_do_1_12() {
    let lib = JarBuilder::new()
        .file(
            "META-INF/mods.toml",
            "modLoader=\"javafml\"\nloaderVersion=\"[1,)\"\nlicense=\"MIT\"\n[[mods]]\nmodId=\"flywheel\"\nversion=\"1.0.0\"\n",
        )
        .build();
    let jar = JarBuilder::new()
        .file(
            "META-INF/jarjar/metadata.json",
            r#"{"jars": [{"identifier": {"group": "dev.engine_room", "artifact": "flywheel"},
                "version": {"range": "[1.0,2.0)", "artifactVersion": "1.0.0"},
                "path": "META-INF/jarjar/flywheel.jar", "isObfuscated": false}]}"#,
        )
        .stored("META-INF/jarjar/flywheel.jar", &lib)
        .file(
            "META-INF/MANIFEST.MF",
            "Manifest-Version: 1.0\nContainedDeps: forgelin.jar\nFMLCorePlugin: x.Core\n",
        )
        .stored("META-INF/forgelin.jar", &lib)
        .file(
            "mcmod.info",
            r#"[{"modid": "create", "version": "0.5.1", "mcversion": "1.12.2",}]"#,
        )
        .build();
    let meta = read(&jar);
    assert_eq!(meta.nested.len(), 2);
    assert_eq!(meta.nested[0].declared_by, DescriptorKind::JarJarMetadata);
    assert_eq!(
        meta.nested[0].jarjar.as_ref().map(|j| j.artifact.as_str()),
        Some("flywheel")
    );
    assert_eq!(meta.nested[1].path, "META-INF/forgelin.jar");
    assert_eq!(meta.nested[1].declared_by, DescriptorKind::Manifest);
    assert!(meta.nested.iter().all(|n| n.metadata.is_some()));
    assert!(
        meta.manifest
            .as_ref()
            .is_some_and(warden_jarmeta::ManifestInfo::is_coremod)
    );
    assert_eq!(meta.mods_for_loader(Loader::Forge)[0].id, "create");
    assert_eq!(meta.warnings[0].code, WarningCode::LenientJson);
}

#[test]
fn servicos_do_loader_e_mods_efetivos() {
    let inner = JarBuilder::new()
        .file(
            "META-INF/mods.toml",
            "[[mods]]
modId=\"connectormod\"
version=\"1.0\"
",
        )
        .build();
    let jar = JarBuilder::new()
        .file(
            "META-INF/MANIFEST.MF",
            "Manifest-Version: 1.0
Embedded-Dependencies-Mod: META-INF/jarjar/mod.jar
",
        )
        .stored("META-INF/jarjar/mod.jar", &inner)
        .file(
            "META-INF/services/net.minecraftforge.forgespi.locating.IModLocator",
            "x.Y
",
        )
        .file(
            "META-INF/services/cpw.mods.modlauncher.api.ITransformationService",
            "x.Z
",
        )
        .file(
            "META-INF/services/java.sql.Driver",
            "nao.conta
",
        )
        .build();
    let meta = read(&jar);
    assert_eq!(
        meta.loader_services,
        [
            "cpw.mods.modlauncher.api.ITransformationService",
            "net.minecraftforge.forgespi.locating.IModLocator"
        ]
    );
    assert!(meta.mods_for_loader(Loader::Forge).is_empty());
    assert_eq!(
        meta.effective_mods_for_loader(Loader::Forge)[0].id,
        "connectormod"
    );
    assert!(meta.effective_mods_for_loader(Loader::Fabric).is_empty());

    let library = JarBuilder::new()
        .file(
            "META-INF/jarjar/metadata.json",
            r#"{"jars": [{"identifier": {"group": "g", "artifact": "a"}, "path": "META-INF/jarjar/a.jar"}]}"#,
        )
        .stored("META-INF/jarjar/a.jar", &inner)
        .build();
    let meta = read(&library);
    assert_eq!(meta.effective_mods_for_loader(Loader::NeoForge).len(), 1);
    assert!(meta.effective_mods_for_loader(Loader::Quilt).is_empty());
}

#[test]
fn jar_sem_descritor_e_jar_vazio() {
    let meta = read(&JarBuilder::new().class("a/B.class", 52).build());
    assert!(meta.mods.is_empty());
    assert!(meta.mods_for_loader(Loader::Forge).is_empty());
    assert_eq!(meta.class_version.map(|v| v.major), Some(52));
    let empty = read(&JarBuilder::new().build());
    assert_eq!(empty, warden_jarmeta::JarMetadata::default());
    let json = serde_json::to_string(&empty).expect("json");
    assert_eq!(json, "{}");
}

#[test]
fn classes_com_cabecalho_ruim_ou_curto_sao_ignoradas() {
    let meta = read(
        &JarBuilder::new()
            .file("A.class", [0xDE, 0xAD, 0xBE, 0xEF, 0, 0, 0, 99])
            .file("B.class", [0xCA, 0xFE])
            .file("C.CLASS", [0xCA, 0xFE, 0xBA, 0xBE, 0, 0, 0, 60])
            .build(),
    );
    assert_eq!(meta.class_version.map(|v| v.major), Some(60));
    let limits = Limits {
        max_class_headers: 1,
        ..Limits::default()
    };
    let meta = read_jar_bytes(
        &JarBuilder::new()
            .class("A.class", 52)
            .class("B.class", 65)
            .build(),
        &limits,
    )
    .expect("lido");
    assert_eq!(
        meta.class_version.map(|v| v.major),
        Some(52),
        "só a primeira classe"
    );
    // Uma classe perdida mais nova não muda a predominante, mas aparece como máxima.
    let meta = read(
        &JarBuilder::new()
            .class("a/A.class", 52)
            .class("a/B.class", 52)
            .class("java/x/Tags.class", 65)
            .build(),
    );
    assert_eq!(meta.class_version.map(|v| v.major), Some(52));
    assert_eq!(meta.max_class_version.map(|v| v.major), Some(65));
}

#[test]
fn descritor_grande_demais_e_texto_em_windows_1252() {
    let limits = Limits {
        max_descriptor_bytes: 64,
        ..Limits::default()
    };
    let big = format!(
        r#"{{"schemaVersion": 1, "id": "x", "version": "1", "description": "{}"}}"#,
        "a".repeat(200)
    );
    let jar = JarBuilder::new()
        .file("fabric.mod.json", big)
        .file(
            "mcmod.info",
            b"[{\"modid\": \"velho\", \"name\": \"Jos\xE9\"}]",
        )
        .build();
    let meta = read_jar_bytes(&jar, &limits).expect("lido");
    let codes: Vec<_> = meta.warnings.iter().map(|w| w.code).collect();
    assert_eq!(codes, [WarningCode::EntryTooLarge, WarningCode::NotUtf8]);
    assert_eq!(
        meta.descriptors,
        [DescriptorKind::FabricModJson, DescriptorKind::McmodInfo]
    );
    assert_eq!(meta.mods[0].name.as_deref(), Some("José"));
}

/// Zip bomb: 128 MiB de espaços cabem em menos de 1 MB comprimidos. A leitura para no limite.
#[test]
fn zip_bomb_em_descritor_e_em_jar_embutido() {
    let zeros = vec![b' '; 128 * 1024 * 1024];
    let bomb_jar = JarBuilder::new().file("fabric.mod.json", &zeros).build();
    assert!(
        bomb_jar.len() < 2 * 1024 * 1024,
        "comprimido: {}",
        bomb_jar.len()
    );
    let start = std::time::Instant::now();
    let meta = read(&bomb_jar);
    assert_eq!(meta.warnings[0].code, WarningCode::EntryTooLarge);
    let top = JarBuilder::new()
        .file(
            "fabric.mod.json",
            fabric_mod_json("x", "1", &["META-INF/jars/bomb.jar"]),
        )
        .file("META-INF/jars/bomb.jar", &zeros)
        .build();
    drop(zeros);
    let limits = Limits {
        max_nested_jar_bytes: 8 * 1024 * 1024,
        ..Limits::default()
    };
    let meta = read_jar_bytes(&top, &limits).expect("lido");
    assert_eq!(meta.warnings[0].code, WarningCode::UnreadableNestedJar);
    assert!(meta.warnings[0].detail.contains("limite"));
    assert!(
        start.elapsed().as_secs() < 30,
        "leitura limitada: {:?}",
        start.elapsed()
    );
}

#[test]
fn profundidade_e_orcamento_de_embutidos() {
    // Cada nível embute o anterior: 6 níveis.
    let mut jar = JarBuilder::new()
        .file("fabric.mod.json", fabric_mod_json("n0", "1", &[]))
        .build();
    for level in 1..=6 {
        jar = JarBuilder::new()
            .file(
                "fabric.mod.json",
                fabric_mod_json(&format!("n{level}"), "1", &["META-INF/jars/inner.jar"]),
            )
            .stored("META-INF/jars/inner.jar", &jar)
            .build();
    }
    let all = read(&jar);
    assert_eq!(all.walk().len(), 7);
    let shallow = read_jar_bytes(
        &jar,
        &Limits {
            max_depth: 2,
            ..Limits::default()
        },
    )
    .expect("lido");
    assert_eq!(shallow.walk().len(), 3);
    let deepest = shallow.walk().pop().expect("último").1;
    assert_eq!(deepest.warnings[0].code, WarningCode::UnreadableNestedJar);
    assert!(deepest.warnings[0].detail.contains("profundidade"));

    let tight = read_jar_bytes(
        &jar,
        &Limits {
            max_total_nested_bytes: u64::try_from(jar.len()).expect("cabe") / 2,
            ..Limits::default()
        },
    )
    .expect("lido");
    assert!(tight.walk().len() < 7, "o orçamento total corta a descida");
}

#[test]
fn jar_embutido_corrompido_vira_aviso() {
    let jar = JarBuilder::new()
        .file(
            "fabric.mod.json",
            fabric_mod_json("x", "1", &["META-INF/jars/ruim.jar"]),
        )
        .stored("META-INF/jars/ruim.jar", b"isto nao e um zip")
        .build();
    let meta = read(&jar);
    assert_eq!(meta.warnings[0].code, WarningCode::UnreadableNestedJar);
    assert_eq!(meta.nested[0].metadata, None);
}

#[test]
fn jar_de_cima_corrompido_e_limites_viram_erro() {
    let err = read_jar_bytes(b"PK\x03\x04 lixo", &Limits::default()).expect_err("inválido");
    assert_eq!(
        err.code(),
        DomainCode::Domain(JarmetaErrorCode::InvalidArchive)
    );
    let err = read_jar_bytes(b"", &Limits::default()).expect_err("vazio");
    assert_eq!(
        err.code(),
        DomainCode::Domain(JarmetaErrorCode::InvalidArchive)
    );

    let jar = JarBuilder::new().file("a", "1").file("b", "2").build();
    let err = read_jar_bytes(
        &jar,
        &Limits {
            max_entries: 1,
            ..Limits::default()
        },
    )
    .expect_err("entradas demais");
    assert_eq!(
        err.code(),
        DomainCode::Domain(JarmetaErrorCode::LimitExceeded)
    );
    let err = read_jar_bytes(
        &jar,
        &Limits {
            max_file_bytes: 10,
            ..Limits::default()
        },
    )
    .expect_err("grande demais");
    assert!(matches!(err, Error::LimitExceeded { .. }));
}

#[test]
fn leitura_do_disco() {
    let dir = std::env::temp_dir().join(format!("warden-jarmeta-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("pasta");
    let path = dir.join("mod.jar");
    let jar = JarBuilder::new()
        .file(
            "quilt.mod.json",
            r#"{"schema_version": 1, "quilt_loader": {"id": "q", "version": "1.0"},
                "minecraft": {"environment": "client"}}"#,
        )
        .build();
    std::fs::write(&path, &jar).expect("grava");
    let meta = read_jar_file(&path, &Limits::default()).expect("lido");
    assert_eq!(meta.mods[0].loader, Loader::Quilt);
    assert_eq!(meta.mods[0].environment, Environment::Client);
    assert_eq!(meta.mods_for_loader(Loader::Quilt)[0].id, "q");
    let err = read_jar_file(
        &path,
        &Limits {
            max_file_bytes: 1,
            ..Limits::default()
        },
    )
    .expect_err("limite");
    assert_eq!(
        err.code(),
        DomainCode::Domain(JarmetaErrorCode::LimitExceeded)
    );
    std::fs::write(&path, b"not a zip").expect("grava");
    let err = read_jar_file(&path, &Limits::default()).expect_err("inválido");
    assert_eq!(
        err.code(),
        DomainCode::Domain(JarmetaErrorCode::InvalidArchive)
    );
    let err = read_jar_file(&dir.join("nao-existe.jar"), &Limits::default()).expect_err("ausente");
    assert_eq!(err.code(), DomainCode::Core(CoreErrorCode::Io));
    assert!(
        err.params()
            .get("path")
            .is_some_and(|p| p.ends_with("nao-existe.jar"))
    );
    std::fs::remove_dir_all(&dir).expect("limpa");
}

#[test]
fn modelo_volta_igual_do_json() {
    let jar = JarBuilder::new()
        .file(
            "fabric.mod.json",
            r#"{"schemaVersion": 1, "id": "a", "version": "1", "environment": "server",
                "depends": {"minecraft": "1.20.x"}, "breaks": {"b": "*"}}"#,
        )
        .build();
    let meta = read(&jar);
    assert_eq!(meta.mods[0].dependencies[1].kind, DependencyKind::Breaks);
    let json = serde_json::to_string(&meta).expect("json");
    let back: warden_jarmeta::JarMetadata = serde_json::from_str(&json).expect("volta");
    assert_eq!(back, meta);
}
