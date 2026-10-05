//! Os 100 mods mais baixados de cada loader no Modrinth, na versão mais recente: todos são lidos
//! sem erro e identificados para o loader, exceto os que se carregam por código (serviço do
//! loader em `META-INF/services/` ou `TweakClass`, como o Essential), que não têm descritor. Nada é versionado; os jars ficam no cache da `target`
//! e o relatório com os números vai para `target/tmp/jarmeta-populares.json`.
//!
//! Roda com `cargo xtask test-network` (sem chave: a API do Modrinth é aberta).

// Auxiliares de teste fora de funções #[test]: falhar com mensagem é o comportamento certo.
#![allow(clippy::expect_used, clippy::panic)]

mod common;

use std::collections::BTreeMap;
use std::sync::Mutex;
use std::time::Instant;

use serde::Serialize;
use serde_json::Value;
use warden_jarmeta::{Limits, Loader, read_jar_bytes};

const PER_LOADER: usize = 100;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Outcome {
    loader: String,
    project: String,
    version: String,
    file: String,
    bytes: usize,
    read_ms: u128,
    mods: Vec<String>,
    nested_jars: usize,
    /// Jar que se carrega por código (serviço do loader ou `TweakClass`), sem descritor.
    self_loading: bool,
    warnings: Vec<String>,
    error: Option<String>,
}

/// GET na API do Modrinth; no limite de requisições (429), espera e tenta de novo (até ~2 min).
fn api(path: &str) -> Value {
    let url = format!("https://api.modrinth.com/v2/{path}");
    let mut last = String::new();
    for attempt in 1..=6u64 {
        match common::fetch(&url) {
            Ok(bytes) => return serde_json::from_slice(&bytes).expect("json do Modrinth"),
            Err(e) if e.contains("429") => {
                last = e;
                std::thread::sleep(std::time::Duration::from_secs(5 * attempt));
            }
            Err(e) => panic!("{url}: {e}"),
        }
    }
    panic!("{url}: {last}")
}

fn top_projects(loader: &str) -> Vec<String> {
    let facets = format!(r#"[["project_type:mod"],["categories:{loader}"]]"#);
    let query = format!(
        "search?index=downloads&limit={PER_LOADER}&facets={}",
        encode(&facets)
    );
    api(&query)["hits"]
        .as_array()
        .expect("hits")
        .iter()
        .filter_map(|h| h["project_id"].as_str().map(str::to_owned))
        .collect()
}

fn encode(text: &str) -> String {
    text.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
                char::from(b).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

fn latest_file(project: &str, loader: &str) -> Option<(String, String, String, String)> {
    let path = format!(
        "project/{project}/version?loaders={}",
        encode(&format!(r#"["{loader}"]"#))
    );
    let versions = api(&path);
    let version = versions.as_array()?.first()?;
    let files = version["files"].as_array()?;
    let file = files
        .iter()
        .find(|f| f["primary"].as_bool() == Some(true))
        .or_else(|| files.first())?;
    Some((
        version["version_number"].as_str()?.to_owned(),
        file["filename"].as_str()?.to_owned(),
        file["url"].as_str()?.to_owned(),
        file["hashes"]["sha1"].as_str()?.to_owned(),
    ))
}

fn check(loader_name: &str, loader: Loader, project: &str) -> Outcome {
    let mut outcome = Outcome {
        loader: loader_name.to_owned(),
        project: project.to_owned(),
        version: String::new(),
        file: String::new(),
        bytes: 0,
        read_ms: 0,
        mods: Vec::new(),
        nested_jars: 0,
        self_loading: false,
        warnings: Vec::new(),
        error: None,
    };
    let Some((version, file, url, sha1)) = latest_file(project, loader_name) else {
        outcome.error = Some("sem versão para o loader".into());
        return outcome;
    };
    outcome.version = version;
    outcome.file.clone_from(&file);
    if !std::path::Path::new(&file)
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("jar"))
    {
        outcome.error = Some("arquivo principal não é jar".into());
        return outcome;
    }
    let cache = common::cache_dir("jarmeta-populares");
    let bytes = match common::download_cached(&url, &sha1, &cache) {
        Ok(b) => b,
        Err(e) => {
            outcome.error = Some(format!("download: {e}"));
            return outcome;
        }
    };
    outcome.bytes = bytes.len();
    let start = Instant::now();
    let result = read_jar_bytes(&bytes, &Limits::default());
    outcome.read_ms = start.elapsed().as_millis();
    match result {
        Ok(meta) => {
            outcome.mods = meta
                .effective_mods_for_loader(loader)
                .iter()
                .map(|m| m.id.clone())
                .collect();
            outcome.nested_jars = meta.walk().len() - 1;
            outcome.self_loading = !meta.loader_services.is_empty()
                || meta
                    .manifest
                    .as_ref()
                    .is_some_and(|m| m.tweak_class.is_some());
            outcome.warnings = meta
                .walk()
                .iter()
                .flat_map(|(path, m)| {
                    m.warnings
                        .iter()
                        .map(move |w| format!("{path}!/{}: {:?} {}", w.path, w.code, w.detail))
                })
                .collect();
        }
        Err(e) => outcome.error = Some(e.to_string()),
    }
    outcome
}

#[test]
#[ignore = "rede"]
fn rede_cem_mais_baixados_por_loader() {
    let loaders = [
        ("fabric", Loader::Fabric),
        ("forge", Loader::Forge),
        ("neoforge", Loader::NeoForge),
        ("quilt", Loader::Quilt),
    ];
    let outcomes = Mutex::new(Vec::new());
    for (name, loader) in loaders {
        let projects = top_projects(name);
        assert!(
            projects.len() >= 50,
            "{name}: busca devolveu {}",
            projects.len()
        );
        let chunks: Vec<Vec<String>> = projects.chunks(25).map(<[String]>::to_vec).collect();
        std::thread::scope(|scope| {
            for chunk in &chunks {
                let outcomes = &outcomes;
                scope.spawn(move || {
                    for project in chunk {
                        let outcome = check(name, loader, project);
                        outcomes.lock().expect("trava").push(outcome);
                    }
                });
            }
        });
    }
    let outcomes = outcomes.into_inner().expect("trava");

    let mut summary: BTreeMap<String, BTreeMap<&str, u128>> = BTreeMap::new();
    for o in &outcomes {
        let s = summary.entry(o.loader.clone()).or_default();
        *s.entry("jars").or_default() += 1;
        *s.entry("identificados").or_default() += u128::from(!o.mods.is_empty());
        *s.entry("seCarregamSozinhos").or_default() +=
            u128::from(o.mods.is_empty() && o.self_loading);
        *s.entry("erros").or_default() += u128::from(o.error.is_some());
        *s.entry("comAvisos").or_default() += u128::from(!o.warnings.is_empty());
        *s.entry("jarsEmbutidos").or_default() += o.nested_jars as u128;
        *s.entry("msLeitura").or_default() += o.read_ms;
        let max = s.entry("msLeituraMaior").or_default();
        *max = (*max).max(o.read_ms);
        *s.entry("bytes").or_default() += o.bytes as u128;
    }
    // Segunda passada, sequencial e com os jars já na memória: só o tempo de leitura.
    let cache = common::cache_dir("jarmeta-populares");
    let mut sequential_ms: Vec<f64> = Vec::new();
    for entry in std::fs::read_dir(&cache).expect("cache") {
        let bytes = std::fs::read(entry.expect("entrada").path()).expect("jar");
        let start = Instant::now();
        let _ = read_jar_bytes(&bytes, &Limits::default());
        sequential_ms.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    sequential_ms.sort_by(f64::total_cmp);
    let percentile = |percent: usize| {
        let index = sequential_ms.len().saturating_sub(1) * percent / 100;
        sequential_ms.get(index).copied().unwrap_or_default()
    };
    let timing = serde_json::json!({
        "perfil": if cfg!(debug_assertions) { "debug" } else { "release" },
        "jars": sequential_ms.len(),
        "totalMs": sequential_ms.iter().sum::<f64>(),
        "medianaMs": percentile(50),
        "p95Ms": percentile(95),
        "maiorMs": sequential_ms.last().copied().unwrap_or_default(),
    });
    let report =
        serde_json::json!({ "resumo": summary, "tempoSequencial": timing, "jars": outcomes });
    let path = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("jarmeta-populares.json");
    std::fs::write(&path, serde_json::to_vec_pretty(&report).expect("json")).expect("relatório");

    let problems: Vec<String> = outcomes
        .iter()
        .filter(|o| {
            o.error.as_deref().is_some_and(|e| {
                !e.starts_with("sem versão") && !e.starts_with("arquivo principal")
            }) || (o.error.is_none() && o.mods.is_empty() && !o.self_loading)
        })
        .map(|o| format!("{} {} {}: {:?}", o.loader, o.project, o.file, o.error))
        .collect();
    assert!(
        problems.is_empty(),
        "relatório em {}:\n{}",
        path.display(),
        problems.join("\n")
    );
}
