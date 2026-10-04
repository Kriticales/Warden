//! Critério 1 da P1-06: o corpus de jars reais produz o modelo esperado (dourado).
//!
//! Os jars de terceiros não ficam no repositório (QUALITY §4; regras da onda 1). Ficam:
//!
//! - `tests/corpus/corpus.json`: cada jar com projeto, versão, URL, sha1, licença e página;
//! - `tests/corpus/jars/<nome>/`: os arquivos de metadados extraídos do jar real, sem alteração,
//!   e `_estrutura.json` (versão de classe e jars embutidos, que viram subpastas curtas `e0`,
//!   `e1`... com o caminho real anotado);
//! - `tests/snapshots/corpus__<nome>.snap`: o modelo esperado (insta).
//!
//! `corpus_dourado` remonta cada jar na memória a partir da extração e compara com o dourado.
//! `rede_corpus_jars_reais` baixa os jars reais (sha1 conferido), confere que a extração
//! versionada é idêntica ao conteúdo real e compara com o **mesmo** dourado. Para refazer a
//! extração depois de mudar o `corpus.json`: `$env:WARDEN_JARMETA_ATUALIZAR_CORPUS = '1'` e
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
use warden_jarmeta::{ClassVersion, DescriptorKind, JarMetadata, Limits, Loader, read_jar_bytes};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CorpusEntry {
    nome: String,
    loader_alvo: String,
    minecraft: String,
    url: String,
    sha1: String,
    /// Motivo, quando o jar real não traz descritor que o loader leia (o modelo fica sem mods).
    #[serde(default)]
    sem_descritor: Option<String>,
}

/// O que não dá para guardar como arquivo extraído.
#[derive(Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Structure {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    class_version: Option<ClassVersion>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    nested: Vec<NestedDir>,
}

/// Jar embutido: caminho dentro do jar e pasta curta da extração (`e0`, `e1`...), para não
/// passar do limite de caminho do Windows com embutidos de embutidos.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NestedDir {
    path: String,
    dir: String,
}

const STRUCTURE_FILE: &str = "_estrutura.json";

const DESCRIPTORS: [DescriptorKind; 7] = [
    DescriptorKind::QuiltModJson,
    DescriptorKind::FabricModJson,
    DescriptorKind::NeoForgeModsToml,
    DescriptorKind::ModsToml,
    DescriptorKind::McmodInfo,
    DescriptorKind::JarJarMetadata,
    DescriptorKind::Manifest,
];

fn corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("corpus")
}

fn corpus() -> Vec<CorpusEntry> {
    let text = fs::read_to_string(corpus_dir().join("corpus.json")).expect("corpus.json");
    serde_json::from_str(&text).expect("corpus.json válido")
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

/// Remonta um jar a partir da pasta extraída.
fn rebuild(dir: &Path) -> Vec<u8> {
    let structure: Structure = fs::read_to_string(dir.join(STRUCTURE_FILE))
        .map(|t| serde_json::from_str(&t).expect("_estrutura.json válido"))
        .unwrap_or_default();
    let mut jar = JarBuilder::new();
    for kind in DESCRIPTORS {
        let path = dir.join(kind.path());
        if path.is_file() {
            jar = jar.file(kind.path(), fs::read(&path).expect("descritor"));
        }
    }
    for nested in &structure.nested {
        jar = jar.stored(&nested.path, rebuild(&dir.join(&nested.dir)));
    }
    if let Some(version) = structure.class_version {
        let mut header = vec![0xCA, 0xFE, 0xBA, 0xBE];
        header.extend_from_slice(&version.minor.to_be_bytes());
        header.extend_from_slice(&version.major.to_be_bytes());
        jar = jar.file("warden/Stub.class", header);
    }
    jar.build()
}

/// Arquivos que a extração de um jar real deve produzir (caminho relativo → bytes).
fn extract(bytes: &[u8], meta: &JarMetadata) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut out = BTreeMap::new();
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).expect("zip");
    for kind in DESCRIPTORS {
        if let Ok(mut file) = zip.by_name(kind.path()) {
            let mut content = Vec::new();
            file.read_to_end(&mut content).expect("descritor");
            out.insert(PathBuf::from(kind.path()), content);
        }
    }
    let mut structure = Structure {
        class_version: meta.class_version,
        nested: Vec::new(),
    };
    for nested in &meta.nested {
        let Some(nested_meta) = &nested.metadata else {
            continue;
        };
        let name = nested.path.trim_start_matches('/');
        let mut content = Vec::new();
        zip.by_name(name)
            .expect("embutido")
            .read_to_end(&mut content)
            .expect("embutido");
        let short = format!("e{}", structure.nested.len());
        for (path, data) in extract(&content, nested_meta) {
            out.insert(Path::new(&short).join(path), data);
        }
        structure.nested.push(NestedDir {
            path: name.to_owned(),
            dir: short,
        });
    }
    if structure != Structure::default() {
        let mut json = serde_json::to_vec_pretty(&structure).expect("json");
        json.push(b'\n');
        out.insert(PathBuf::from(STRUCTURE_FILE), json);
    }
    out
}

fn read_tree(dir: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn walk(base: &Path, dir: &Path, out: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in fs::read_dir(dir).expect("pasta") {
            let path = entry.expect("entrada").path();
            if path.is_dir() {
                walk(base, &path, out);
            } else {
                let relative = path.strip_prefix(base).expect("prefixo").to_path_buf();
                out.insert(relative, fs::read(&path).expect("arquivo"));
            }
        }
    }
    let mut out = BTreeMap::new();
    if dir.is_dir() {
        walk(dir, dir, &mut out);
    }
    out
}

fn snapshot(entry: &CorpusEntry, meta: &JarMetadata) {
    insta::with_settings!({
        description => format!("{} (Minecraft {}, loader {})", entry.nome, entry.minecraft, entry.loader_alvo),
        omit_expression => true,
        prepend_module_to_snapshot => true,
    }, {
        insta::assert_json_snapshot!(entry.nome.as_str(), meta);
    });
}

/// Cada jar do corpus tem os mods do loader a que se destina, salvo os marcados com
/// `semDescritor` (e esses precisam continuar sem).
fn identification_problem(entry: &CorpusEntry, meta: &JarMetadata) -> Option<String> {
    let found = !meta
        .effective_mods_for_loader(loader(&entry.loader_alvo))
        .is_empty();
    match (&entry.sem_descritor, found) {
        (None, false) => Some(format!(
            "{}: nenhum mod para o loader {}",
            entry.nome, entry.loader_alvo
        )),
        (Some(_), true) => Some(format!(
            "{}: marcado semDescritor, mas tem mods",
            entry.nome
        )),
        _ => None,
    }
}

#[test]
fn ca_p1_06_1_corpus_dourado() {
    let entries = corpus();
    assert!(
        entries.len() >= 30,
        "o corpus precisa de pelo menos 30 jars"
    );
    for entry in &entries {
        let dir = corpus_dir().join("jars").join(&entry.nome);
        assert!(
            dir.is_dir(),
            "{}: extração ausente; rode o teste de rede",
            entry.nome
        );
        let meta = read_jar_bytes(&rebuild(&dir), &Limits::default())
            .unwrap_or_else(|e| panic!("{}: {e}", entry.nome));
        if let Some(problem) = identification_problem(entry, &meta) {
            panic!("{problem}");
        }
        snapshot(entry, &meta);
    }
}

#[test]
fn corpus_cobre_os_loaders_e_versoes_pedidos() {
    let entries = corpus();
    let has = |loader: &str, mc: &str| {
        entries
            .iter()
            .any(|e| e.loader_alvo == loader && e.minecraft == mc)
    };
    for (loader, mc) in [
        ("fabric", "1.20.1"),
        ("quilt", "1.20.1"),
        ("forge", "1.7.10"),
        ("forge", "1.12.2"),
        ("forge", "1.16.5"),
        ("forge", "1.20.1"),
        ("neoforge", "1.21.1"),
    ] {
        assert!(has(loader, mc), "falta {loader} {mc} no corpus");
    }
    let mut names: Vec<&str> = entries.iter().map(|e| e.nome.as_str()).collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), entries.len(), "nomes repetidos no corpus");
}

#[test]
#[ignore = "rede"]
fn rede_corpus_jars_reais() {
    let update = std::env::var("WARDEN_JARMETA_ATUALIZAR_CORPUS").is_ok_and(|v| v == "1");
    let cache = common::cache_dir("jarmeta-corpus");
    let mut problems = Vec::new();
    for entry in corpus() {
        let bytes = common::download_cached(&entry.url, &entry.sha1, &cache)
            .unwrap_or_else(|e| panic!("{}: {e}", entry.nome));
        let meta = read_jar_bytes(&bytes, &Limits::default())
            .unwrap_or_else(|e| panic!("{}: {e}", entry.nome));
        let expected = extract(&bytes, &meta);
        let dir = corpus_dir().join("jars").join(&entry.nome);
        if update {
            if dir.exists() {
                fs::remove_dir_all(&dir).expect("limpa");
            }
            for (path, data) in &expected {
                let target = dir.join(path);
                fs::create_dir_all(target.parent().expect("pai")).expect("pasta");
                fs::write(&target, data).expect("grava");
            }
        }
        let versioned = read_tree(&dir);
        if versioned != expected {
            problems.push(format!(
                "{}: a extração versionada difere do jar real (rode com                  WARDEN_JARMETA_ATUALIZAR_CORPUS=1)",
                entry.nome
            ));
        }
        problems.extend(identification_problem(&entry, &meta));
        snapshot(&entry, &meta);
    }
    assert!(
        problems.is_empty(),
        "{}",
        problems.join(
            "
"
        )
    );
}
