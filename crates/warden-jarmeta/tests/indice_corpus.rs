//! Critérios da D-05 com jars reais: o corpus da P1-06 (`tests/corpus/corpus.json`) e os jars
//! que faltavam para a R5A §5.3 e para o Epic Fight (`tests/indice/extras.json`).
//!
//! Os jars de terceiros não ficam no repositório. Fica `tests/indice/esqueletos.json`: de cada
//! jar (e de cada embutido que o loader carrega), os descritores sem alteração, os pacotes com
//! classe e as configs de mixin presentes. `ca_d05_*` remonta um jar "esqueleto" a partir disso
//! (os descritores, uma classe vazia por pacote, as configs) e monta os índices de verdade.
//!
//! `rede_indice_jars_reais` baixa os jars reais (sha1 conferido), confere que o índice do jar
//! real é igual ao do esqueleto e que o esqueleto versionado é o que sai do jar real. Para
//! refazer depois de mudar as listas: `$env:WARDEN_JARMETA_ATUALIZAR_INDICE = '1'` e
//! `cargo xtask test-network`; depois revisar os dourados com `cargo insta review`.

// Auxiliares de teste fora de funções #[test]: falhar com mensagem é o comportamento certo.
#![allow(clippy::expect_used, clippy::panic)]

mod common;

use std::collections::BTreeMap;
use std::fs;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};

use common::JarBuilder;
use serde::{Deserialize, Serialize};
use warden_jarmeta::index::{
    JarIndex, JarIndexCache, Ownership, PackIndex, PackJar, build_pack_index, index_jar_bytes,
};
use warden_jarmeta::{DescriptorKind, JarMetadata, Limits, Loader, read_jar_bytes};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Entry {
    nome: String,
    loader_alvo: String,
    minecraft: String,
    url: String,
    sha1: String,
}

/// Conteúdo de um arquivo: texto quando é UTF-8, bytes quando não é (`mcmod.info` antigos).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
enum Content {
    Text(String),
    Bytes(Vec<u8>),
}

impl Content {
    fn new(bytes: Vec<u8>) -> Self {
        String::from_utf8(bytes).map_or_else(|e| Self::Bytes(e.into_bytes()), Self::Text)
    }

    fn bytes(&self) -> &[u8] {
        match self {
            Self::Text(text) => text.as_bytes(),
            Self::Bytes(bytes) => bytes,
        }
    }
}

/// O que o índice precisa de um jar.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Skeleton {
    /// Descritores, sem alteração.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    arquivos: BTreeMap<String, Content>,
    /// Pacotes com pelo menos uma classe.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pacotes: Vec<String>,
    /// Configs de mixin declaradas e presentes no jar.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    mixins: Vec<String>,
    /// Embutidos lidos, pelo caminho dentro do jar.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    aninhados: BTreeMap<String, Skeleton>,
}

const DESCRIPTORS: [DescriptorKind; 7] = [
    DescriptorKind::QuiltModJson,
    DescriptorKind::FabricModJson,
    DescriptorKind::NeoForgeModsToml,
    DescriptorKind::ModsToml,
    DescriptorKind::McmodInfo,
    DescriptorKind::JarJarMetadata,
    DescriptorKind::Manifest,
];

fn tests_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests")
}

fn skeletons_path() -> PathBuf {
    tests_dir().join("indice").join("esqueletos.json")
}

fn entries() -> Vec<Entry> {
    let mut out = Vec::new();
    for path in [
        tests_dir().join("corpus").join("corpus.json"),
        tests_dir().join("indice").join("extras.json"),
    ] {
        let text = fs::read_to_string(&path).expect("lista de jars");
        let list: Vec<Entry> = serde_json::from_str(&text).expect("lista válida");
        out.extend(list);
    }
    out
}

fn loader(name: &str) -> Loader {
    match name {
        "fabric" => Loader::Fabric,
        "quilt" => Loader::Quilt,
        "forge" => Loader::Forge,
        "neoforge" => Loader::NeoForge,
        other => panic!("loader desconhecido {other}"),
    }
}

/// Grupo = um pack possível (mesmo loader e mesma versão do jogo).
fn group(entry: &Entry) -> String {
    format!("{}-{}", entry.loader_alvo, entry.minecraft)
}

/// Item do pack para o jar.
fn item(entry: &Entry) -> String {
    format!("mods/{}.pw.toml", entry.nome)
}

fn versioned_skeletons() -> BTreeMap<String, Skeleton> {
    let text = fs::read_to_string(skeletons_path()).expect("esqueletos.json; rode o teste de rede");
    serde_json::from_str(&text).expect("esqueletos.json válido")
}

/// Esqueleto de um jar real, seguindo os mesmos embutidos que o índice segue.
fn skeleton(bytes: &[u8], meta: &JarMetadata) -> Skeleton {
    let declared: Vec<String> = meta
        .walk()
        .into_iter()
        .flat_map(|(_, jar)| {
            jar.mods
                .iter()
                .flat_map(|m| m.mixins.iter())
                .chain(jar.manifest.iter().flat_map(|m| m.mixin_configs.iter()))
                .map(|c| c.trim().to_owned())
                .filter(|c| !c.is_empty())
                .collect::<Vec<_>>()
        })
        .collect();
    skeleton_node(bytes, meta, &declared)
}

/// Esqueleto de um jar da árvore; `declared` são as configs declaradas em qualquer jar dela.
fn skeleton_node(bytes: &[u8], meta: &JarMetadata, declared: &[String]) -> Skeleton {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).expect("zip");
    let mut out = Skeleton::default();
    for kind in DESCRIPTORS {
        if let Ok(mut file) = zip.by_name(kind.path()) {
            let mut content = Vec::new();
            file.read_to_end(&mut content).expect("descritor");
            out.arquivos
                .insert(kind.path().to_owned(), Content::new(content));
        }
    }
    let mut packages: Vec<String> = zip
        .file_names()
        .filter_map(|name| {
            let stem = name.strip_suffix(".class")?;
            let stem = match stem.strip_prefix("META-INF/versions/") {
                Some(rest) => rest.split_once('/')?.1,
                None if stem.starts_with("META-INF/") => return None,
                None => stem,
            };
            let (package, _) = stem.rsplit_once('/')?;
            (!package.is_empty()).then(|| package.replace('/', "."))
        })
        .collect();
    packages.sort();
    packages.dedup();
    out.pacotes = packages;
    let mut mixins: Vec<String> = declared
        .iter()
        .filter(|c| zip.index_for_name(c.trim_start_matches('/')).is_some())
        .cloned()
        .collect();
    mixins.sort();
    mixins.dedup();
    out.mixins = mixins;
    for nested in &meta.nested {
        let Some(nested_meta) = nested.metadata.as_deref() else {
            continue;
        };
        let name = nested.path.trim_start_matches('/');
        let mut content = Vec::new();
        zip.by_name(name)
            .expect("embutido")
            .read_to_end(&mut content)
            .expect("embutido");
        out.aninhados.insert(
            name.to_owned(),
            skeleton_node(&content, nested_meta, declared),
        );
    }
    out
}

/// Remonta um jar a partir do esqueleto.
fn rebuild(skeleton: &Skeleton) -> Vec<u8> {
    let mut jar = JarBuilder::new();
    for (name, content) in &skeleton.arquivos {
        jar = jar.file(name, content.bytes());
    }
    for package in &skeleton.pacotes {
        jar = jar.class(
            &format!("{}/WardenIndice.class", package.replace('.', "/")),
            52,
        );
    }
    for config in &skeleton.mixins {
        jar = jar.file(config.trim_start_matches('/'), "{}");
    }
    for (path, nested) in &skeleton.aninhados {
        jar = jar.stored(path, rebuild(nested));
    }
    jar.build()
}

fn without_hash(mut index: JarIndex) -> JarIndex {
    index.sha256.clear();
    index
}

/// Índices dos esqueletos, por grupo, na ordem das listas.
fn indices_by_group() -> BTreeMap<String, (Loader, Vec<(Entry, JarIndex)>)> {
    let skeletons = versioned_skeletons();
    let mut groups: BTreeMap<String, (Loader, Vec<(Entry, JarIndex)>)> = BTreeMap::new();
    for entry in entries() {
        let skeleton = skeletons
            .get(&entry.nome)
            .unwrap_or_else(|| panic!("{}: sem esqueleto; rode o teste de rede", entry.nome));
        let index = index_jar_bytes(&rebuild(skeleton), &Limits::default())
            .unwrap_or_else(|e| panic!("{}: {e}", entry.nome));
        groups
            .entry(group(&entry))
            .or_insert_with(|| (loader(&entry.loader_alvo), Vec::new()))
            .1
            .push((entry, index));
    }
    groups
}

fn pack(loader: Loader, jars: &[(Entry, JarIndex)], only: Option<&[&str]>) -> PackIndex {
    let items = jars
        .iter()
        .filter(|(entry, _)| only.is_none_or(|names| names.contains(&entry.nome.as_str())))
        .map(|(entry, index)| (item(entry), index.clone()))
        .collect();
    PackIndex::new(items, Some(loader))
}

/// Resumo dourado de um grupo: pacotes ambíguos e o dono de cada config de mixin.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GroupSummary {
    jars: usize,
    pacotes: usize,
    ambiguos: Vec<AmbiguousSummary>,
    mixins: BTreeMap<String, String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AmbiguousSummary {
    pacote: String,
    escolhido: Option<String>,
    candidatos: Vec<String>,
}

fn describe(owner: warden_jarmeta::index::JarOwner<'_>) -> String {
    let jar = if owner.jar.is_empty() {
        String::new()
    } else {
        format!(" [{}]", owner.jar)
    };
    format!(
        "{}{jar} → {}",
        owner.item,
        owner.mod_id.unwrap_or("(sem mod)")
    )
}

fn summary(index: &PackIndex) -> GroupSummary {
    GroupSummary {
        jars: index.items().len(),
        pacotes: index.package_count(),
        ambiguos: index
            .ambiguous_packages()
            .into_iter()
            .map(|(package, ownership)| AmbiguousSummary {
                pacote: package.to_owned(),
                escolhido: ownership.owner().map(describe),
                candidatos: match ownership {
                    Ownership::Unique(owner) => vec![describe(owner)],
                    Ownership::Ambiguous { candidates, .. } => {
                        candidates.into_iter().map(describe).collect()
                    }
                },
            })
            .collect(),
        mixins: index
            .mixin_configs()
            .map(|config| {
                let owner = index.mixin_owner(config).map_or_else(
                    || "(sem dono)".to_owned(),
                    |o| {
                        let presence = if o.present { "" } else { " (ausente no jar)" };
                        format!("{}{presence}", describe(o.owner))
                    },
                );
                (config.to_owned(), owner)
            })
            .collect(),
    }
}

/// Os 5 jars Fabric da R5A §5.3.
const R5A_FABRIC: [&str; 5] = [
    "fabric-1.20.1-sodium",
    "fabric-1.20.1-lithium",
    "fabric-1.20.1-iris",
    "fabric-1.20.1-fabric-api",
    "fabric-1.20.1-modernfix",
];

#[test]
fn ca_d05_cinco_jars_fabric_tem_no_maximo_os_2_pacotes_ambiguos_da_fabric_api() {
    let groups = indices_by_group();
    let (loader, jars) = groups.get("fabric-1.20.1").expect("grupo Fabric 1.20.1");
    let index = pack(*loader, jars, Some(&R5A_FABRIC));
    assert_eq!(index.items().len(), 5);
    let ambiguous = index.ambiguous_packages();
    assert!(
        ambiguous.len() <= 2,
        "{} pacotes ambíguos: {:?}",
        ambiguous.len(),
        ambiguous.iter().map(|(p, _)| *p).collect::<Vec<_>>()
    );
    for (package, ownership) in &ambiguous {
        let Ownership::Ambiguous { chosen, candidates } = ownership else {
            panic!("{package}: deveria ser ambíguo");
        };
        assert!(
            candidates
                .iter()
                .all(|c| c.item == "mods/fabric-1.20.1-fabric-api.pw.toml"),
            "{package}: {candidates:?}"
        );
        let chosen = chosen.expect("o jar de topo resolve");
        assert_eq!(chosen.jar, "");
        assert_eq!(chosen.mod_id, Some("fabric-api"));
    }
    // Uma classe de cada mod cai no mod certo.
    for (class, item, mod_id) in [
        (
            "me.jellysquid.mods.sodium.client.SodiumClientMod",
            "fabric-1.20.1-sodium",
            "sodium",
        ),
        (
            "me.jellysquid.mods.lithium.common.LithiumMod",
            "fabric-1.20.1-lithium",
            "lithium",
        ),
        (
            "org.embeddedt.modernfix.ModernFix",
            "fabric-1.20.1-modernfix",
            "modernfix",
        ),
    ] {
        let owner = index
            .class_owner(class)
            .and_then(|o| o.owner())
            .unwrap_or_else(|| panic!("{class}: sem dono"));
        assert_eq!(owner.item, format!("mods/{item}.pw.toml"), "{class}");
        assert_eq!(owner.mod_id, Some(mod_id), "{class}");
    }
}

#[test]
fn ca_d05_config_de_mixin_aponta_o_mod_certo_em_cada_grupo() {
    for (name, (loader, jars)) in indices_by_group() {
        let index = pack(loader, &jars, None);
        // Toda config declarada tem um dono, e é o mod do descritor que a declarou (ou, no
        // manifesto, o mod principal do jar).
        for config in index.mixin_configs() {
            let owner = index
                .mixin_owner(config)
                .unwrap_or_else(|| panic!("{name}: {config} sem dono"));
            assert!(
                owner.owner.mod_id.is_some(),
                "{name}: {config} sem mod ({owner:?})"
            );
        }
        insta::with_settings!({
            description => format!("Índice do grupo {name}: pacotes ambíguos e dono de cada config de mixin"),
            omit_expression => true,
            prepend_module_to_snapshot => true,
        }, {
            insta::assert_json_snapshot!(name.as_str(), summary(&index));
        });
    }
}

#[test]
fn ca_d05_configs_conhecidas() {
    let groups = indices_by_group();
    let cases = [
        (
            "fabric-1.20.1",
            "sodium.mixins.json",
            "fabric-1.20.1-sodium",
            "sodium",
        ),
        (
            "fabric-1.20.1",
            "lithium.mixins.json",
            "fabric-1.20.1-lithium",
            "lithium",
        ),
        (
            "fabric-1.20.1",
            "modernfix-fabric.mixins.json",
            "fabric-1.20.1-modernfix",
            "modernfix",
        ),
        (
            "neoforge-1.21.1",
            "epicfight.mixins.json",
            "neoforge-1.21.1-epic-fight",
            "epicfight",
        ),
        (
            "forge-1.20.1",
            "mixins.epicfight.json",
            "forge-1.20.1-epic-fight",
            "epicfight",
        ),
        // No Sodium para NeoForge os mixins estão no jar embutido.
        (
            "neoforge-1.21.1",
            "sodium-neoforge.mixins.json",
            "neoforge-1.21.1-sodium",
            "sodium",
        ),
    ];
    for (group_name, config, item_name, mod_id) in cases {
        let (loader, jars) = groups.get(group_name).expect("grupo");
        let index = pack(*loader, jars, None);
        let owner = index
            .mixin_owner(config)
            .unwrap_or_else(|| panic!("{config}: sem dono"));
        assert_eq!(
            owner.owner.item,
            item(&Entry {
                nome: item_name.to_owned(),
                loader_alvo: String::new(),
                minecraft: String::new(),
                url: String::new(),
                sha1: String::new(),
            }),
            "{config}"
        );
        assert_eq!(owner.owner.mod_id, Some(mod_id), "{config}");
        assert!(owner.present, "{config}");
    }
}

#[test]
fn ca_d05_segunda_passagem_so_usa_o_cache() {
    let skeletons = versioned_skeletons();
    let dir = tempfile::tempdir().expect("temporária");
    let mods = dir.path().join("mods");
    fs::create_dir_all(&mods).expect("mods");
    let mut jars = Vec::new();
    for entry in entries().iter().filter(|e| group(e) == "fabric-1.20.1") {
        let path = mods.join(format!("{}.jar", entry.nome));
        fs::write(&path, rebuild(&skeletons[&entry.nome])).expect("jar");
        jars.push(PackJar::new(item(entry), path));
    }
    let cache = JarIndexCache::in_cache_dir(&dir.path().join("cache"));
    let limits = Limits::default();
    let first = build_pack_index(&jars, Some(Loader::Fabric), Some(&cache), &limits);
    assert!(first.failures.is_empty(), "{:?}", first.failures);
    assert_eq!(first.stats.jars_opened, jars.len());
    assert_eq!(first.stats.cache_hits, 0);
    assert_eq!(first.stats.cache_write_failures, 0);
    assert_eq!(
        fs::read_dir(cache.dir()).expect("cache").count(),
        jars.len()
    );

    let second = build_pack_index(&jars, Some(Loader::Fabric), Some(&cache), &limits);
    assert_eq!(second.stats.jars_opened, 0, "a segunda passagem abriu jars");
    assert_eq!(second.stats.cache_hits, jars.len());
    assert_eq!(second.index, first.index);

    // Com o hash já conhecido (packwiz guarda sha256 de arquivos locais), nem o hash é lido.
    let known: Vec<PackJar> = jars
        .iter()
        .map(|jar| PackJar {
            sha256: Some(warden_jarmeta::index::sha256_hex(
                &fs::read(&jar.path).expect("jar"),
            )),
            path: dir.path().join("nao-existe.jar"),
            ..jar.clone()
        })
        .collect();
    let third = build_pack_index(&known, Some(Loader::Fabric), Some(&cache), &limits);
    assert!(third.failures.is_empty());
    assert_eq!(third.stats.jars_opened, 0);
    assert_eq!(third.index, first.index);
}

#[test]
#[ignore = "rede"]
#[allow(clippy::print_stderr)] // O tempo medido com os jars reais vai para o relatório.
fn rede_indice_jars_reais() {
    let update = std::env::var("WARDEN_JARMETA_ATUALIZAR_INDICE").is_ok_and(|v| v == "1");
    let cache = common::cache_dir("jarmeta-corpus");
    let mut computed: BTreeMap<String, Skeleton> = BTreeMap::new();
    let mut problems = Vec::new();
    let mut real: BTreeMap<String, (Loader, Vec<(Entry, JarIndex)>)> = BTreeMap::new();
    let mut files = Vec::new();
    for entry in entries() {
        let bytes = common::download_cached(&entry.url, &entry.sha1, &cache)
            .unwrap_or_else(|e| panic!("{}: {e}", entry.nome));
        files.push(PackJar::new(
            item(&entry),
            cache.join(format!("{}.jar", entry.sha1)),
        ));
        let meta = read_jar_bytes(&bytes, &Limits::default())
            .unwrap_or_else(|e| panic!("{}: {e}", entry.nome));
        let index = index_jar_bytes(&bytes, &Limits::default())
            .unwrap_or_else(|e| panic!("{}: {e}", entry.nome));
        let skeleton = skeleton(&bytes, &meta);
        let rebuilt = index_jar_bytes(&rebuild(&skeleton), &Limits::default())
            .unwrap_or_else(|e| panic!("{}: esqueleto: {e}", entry.nome));
        if without_hash(rebuilt) != without_hash(index.clone()) {
            problems.push(format!(
                "{}: o índice do esqueleto difere do jar real",
                entry.nome
            ));
        }
        computed.insert(entry.nome.clone(), skeleton);
        real.entry(group(&entry))
            .or_insert_with(|| (loader(&entry.loader_alvo), Vec::new()))
            .1
            .push((entry, index));
    }
    if update {
        let mut json = serde_json::to_vec_pretty(&computed).expect("json");
        json.push(b'\n');
        fs::write(skeletons_path(), json).expect("grava esqueletos.json");
    }
    if versioned_skeletons() != computed {
        problems.push(
            "esqueletos.json difere dos jars reais (rode com WARDEN_JARMETA_ATUALIZAR_INDICE=1)"
                .to_owned(),
        );
    }
    // Os mesmos dourados dos esqueletos, agora com os jars reais.
    for (name, (loader, jars)) in &real {
        let index = pack(*loader, jars, None);
        insta::with_settings!({
            description => format!("Índice do grupo {name}: pacotes ambíguos e dono de cada config de mixin"),
            omit_expression => true,
            prepend_module_to_snapshot => true,
        }, {
            insta::assert_json_snapshot!(name.as_str(), summary(&index));
        });
    }
    // Tempo de montar o índice dos jars reais, sem cache e com cache.
    let index_cache = JarIndexCache::new(cache.join("jarindex-medicao"));
    let _ = fs::remove_dir_all(index_cache.dir());
    let start = std::time::Instant::now();
    let cold = build_pack_index(&files, None, Some(&index_cache), &Limits::default());
    let cold_time = start.elapsed();
    let start = std::time::Instant::now();
    let warm = build_pack_index(&files, None, Some(&index_cache), &Limits::default());
    eprintln!(
        "D-05: {} jars reais ({} com embutidos), sem cache {cold_time:?}, com cache {:?}",
        files.len(),
        cold.index.items().iter().map(|(_, i)| i.jars.len()).sum::<usize>(),
        start.elapsed()
    );
    assert!(cold.failures.is_empty());
    assert_eq!(warm.stats.jars_opened, 0);

    let (loader, jars) = &real["fabric-1.20.1"];
    let five = pack(*loader, jars, Some(&R5A_FABRIC));
    assert!(five.ambiguous_packages().len() <= 2);
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}
