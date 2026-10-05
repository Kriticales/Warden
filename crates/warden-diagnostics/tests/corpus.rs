//! CA-T14-02: cada log real do corpus gera o diagnóstico esperado, com a linha de evidência
//! correta.
//!
//! Cada pasta de `tests/corpus/<caso>/` imita os arquivos de uma sessão (`output.log`,
//! `latest.log`, `crash-reports/…`) e tem um `esperado.toml` com os achados na ordem, cada um
//! com regra, padrão, arquivo, linha, parâmetros e itens. A origem de cada caso está no próprio
//! `esperado.toml` e em `tests/corpus/FIXTURES.md`.
//!
//! Para regravar os esperados depois de mudar o catálogo:
//! `$env:WARDEN_DIAGNOSTICS_ATUALIZAR_CORPUS = '1'; cargo nextest run -p warden-diagnostics --test corpus`
//! e revisar a diferença no git (o teste não passa em modo de atualização).

// Auxiliares de teste fora de funções #[test]: falhar com mensagem é o comportamento certo.
#![allow(clippy::expect_used, clippy::panic)]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use warden_diagnostics::{
    Analysis, AnalysisContext, Evidence, ItemRef, LogLoader, LogSource, SessionOutcome, SourceKind,
    analyze, postcrash::strip_formatting,
};

const UPDATE_ENV: &str = "WARDEN_DIAGNOSTICS_ATUALIZAR_CORPUS";

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Expected {
    /// De onde veio o log.
    origem: String,
    /// O que o caso mostra.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    nota: String,
    /// Contexto do pack passado à análise (senão, vem do log).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    contexto: Option<Context>,
    /// Simula o supervisor encerrando o jogo parado.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    parado: bool,
    /// Loader que a análise deve achar.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    loader: Option<String>,
    /// Versão do Minecraft que a análise deve achar.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    minecraft: Option<String>,
    /// Achados, na ordem.
    #[serde(default)]
    achado: Vec<ExpectedFinding>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Context {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    loader: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    minecraft: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExpectedFinding {
    regra: String,
    padrao: String,
    arquivo: String,
    linha: u32,
    #[serde(default)]
    params: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    itens: Vec<String>,
}

fn corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("corpus")
}

fn cases() -> Vec<PathBuf> {
    let mut cases: Vec<PathBuf> = fs::read_dir(corpus_dir())
        .expect("pasta do corpus")
        .map(|entry| entry.expect("entrada do corpus").path())
        .filter(|path| path.is_dir())
        .collect();
    cases.sort();
    cases
}

fn kind_of(relative: &str) -> SourceKind {
    let name = relative.rsplit('/').next().unwrap_or(relative);
    if relative.starts_with("crash-reports/") {
        SourceKind::CrashReport
    } else if name.starts_with("hs_err_pid") {
        SourceKind::JvmCrash
    } else if name == "output.log" {
        SourceKind::Output
    } else if name == "latest.log" {
        SourceKind::LatestLog
    } else if name == "debug.log" || name.starts_with("fml-") {
        SourceKind::DebugLog
    } else {
        SourceKind::Text
    }
}

fn walk(dir: &Path, base: &Path, out: &mut Vec<(String, PathBuf)>) {
    for entry in fs::read_dir(dir).expect("pasta do caso") {
        let path = entry.expect("entrada do caso").path();
        if path.is_dir() {
            walk(&path, base, out);
        } else {
            let relative = path
                .strip_prefix(base)
                .expect("dentro do caso")
                .to_string_lossy()
                .replace('\\', "/");
            if relative != "esperado.toml" && relative != ".gitattributes" {
                out.push((relative, path));
            }
        }
    }
}

fn sources_of(case: &Path) -> Vec<LogSource> {
    let mut files = Vec::new();
    walk(case, case, &mut files);
    files.sort();
    files
        .into_iter()
        .map(|(relative, path)| {
            let kind = kind_of(&relative);
            LogSource::read(&path, relative, kind).expect("log do corpus legível")
        })
        .collect()
}

fn parse_loader(text: &str) -> LogLoader {
    match text {
        "fabric" => LogLoader::Fabric,
        "quilt" => LogLoader::Quilt,
        "forge" => LogLoader::Forge,
        "neoforge" => LogLoader::NeoForge,
        other => panic!("loader desconhecido no esperado: {other}"),
    }
}

fn loader_name(loader: LogLoader) -> String {
    serde_json::to_value(loader)
        .expect("loader serializa")
        .as_str()
        .expect("texto")
        .to_owned()
}

fn item_text(item: &ItemRef) -> String {
    match item {
        ItemRef::ModId { id } => format!("mod:{id}"),
        ItemRef::JarFile { name } => format!("jar:{name}"),
        ItemRef::Metafile { path } => format!("metafile:{path}"),
    }
}

fn run(case: &Path, expected: &Expected) -> Analysis {
    let context = expected
        .contexto
        .clone()
        .map(|context| AnalysisContext {
            loader: context.loader.as_deref().map(parse_loader),
            minecraft: context.minecraft,
        })
        .unwrap_or_default();
    let outcome = SessionOutcome {
        froze: expected.parado,
        ..SessionOutcome::default()
    };
    analyze(&sources_of(case), &context, Some(&outcome)).expect("análise do caso")
}

fn actual_findings(analysis: &Analysis) -> Vec<ExpectedFinding> {
    analysis
        .findings
        .iter()
        .map(|entry| {
            let Evidence::Log { file, line, .. } = entry.finding.primary_evidence() else {
                panic!("achado pós-crash sem evidência de log: {entry:?}");
            };
            ExpectedFinding {
                regra: entry.finding.rule().to_string(),
                padrao: entry.pattern.clone(),
                arquivo: file.clone(),
                linha: *line,
                params: entry.finding.params().clone(),
                itens: entry.finding.items().iter().map(item_text).collect(),
            }
        })
        .collect()
}

/// Toda evidência aponta uma linha do arquivo que contém o trecho citado.
fn check_evidence(case: &Path, analysis: &Analysis, problems: &mut Vec<String>) {
    for entry in &analysis.findings {
        for evidence in entry.finding.evidence() {
            let Evidence::Log {
                file,
                line,
                excerpt,
            } = evidence
            else {
                problems.push(format!("{}: evidência que não é de log", case.display()));
                continue;
            };
            let path = case.join(file);
            let bytes = fs::read(&path).expect("arquivo da evidência");
            let raw = bytes
                .split(|&byte| byte == b'\n')
                .nth((*line as usize).saturating_sub(1))
                .map(|line| strip_formatting(&String::from_utf8_lossy(line)))
                .unwrap_or_default();
            let wanted = excerpt.trim_end_matches('…');
            if !raw.contains(wanted.trim()) {
                problems.push(format!(
                    "{}: a linha {line} de {file} não contém o trecho {wanted:?}",
                    case.display()
                ));
            }
        }
    }
}

#[test]
fn ca_t14_02_corpus_de_logs_reais() {
    let update = std::env::var_os(UPDATE_ENV).is_some();
    let cases = cases();
    let mut problems = Vec::new();
    let mut coverage = BTreeSet::new();
    for case in &cases {
        let expected_path = case.join("esperado.toml");
        let mut expected: Expected = match fs::read_to_string(&expected_path) {
            Ok(text) => toml::from_str(&text)
                .unwrap_or_else(|error| panic!("{}: {error}", expected_path.display())),
            Err(_) if update => Expected::default(),
            Err(error) => panic!("{}: {error}", expected_path.display()),
        };
        let analysis = run(case, &expected);
        check_evidence(case, &analysis, &mut problems);
        let actual = actual_findings(&analysis);
        let actual_loader = analysis.loader.map(loader_name);
        if update {
            expected.loader = actual_loader;
            expected.minecraft = analysis.minecraft.clone();
            expected.achado = actual;
            fs::write(
                &expected_path,
                toml::to_string(&expected).expect("esperado serializa"),
            )
            .expect("gravar esperado");
            continue;
        }
        if expected.origem.trim().is_empty() {
            problems.push(format!("{}: sem origem", case.display()));
        }
        if actual_loader != expected.loader || analysis.minecraft != expected.minecraft {
            problems.push(format!(
                "{}: contexto {:?}/{:?}, esperado {:?}/{:?}",
                case.display(),
                actual_loader,
                analysis.minecraft,
                expected.loader,
                expected.minecraft
            ));
        }
        if actual != expected.achado {
            let mut message = format!("{}: achados diferentes\n", case.display());
            for finding in &actual {
                let _ = writeln!(
                    message,
                    "  obtido   {} {} {}:{} {:?}",
                    finding.regra, finding.padrao, finding.arquivo, finding.linha, finding.params
                );
            }
            for finding in &expected.achado {
                let _ = writeln!(
                    message,
                    "  esperado {} {} {}:{} {:?}",
                    finding.regra, finding.padrao, finding.arquivo, finding.linha, finding.params
                );
            }
            problems.push(message);
        }
        if let (Some(loader), Some(minecraft)) = (&expected.loader, &expected.minecraft) {
            coverage.insert(coverage_bucket(loader, minecraft));
        }
    }
    assert!(
        !update,
        "esperados regravados; revise a diferença no git e rode sem {UPDATE_ENV}"
    );
    assert!(problems.is_empty(), "{}", problems.join("\n"));
    assert!(
        cases.len() >= 25,
        "o corpus precisa de pelo menos 25 logs reais: {}",
        cases.len()
    );
    for bucket in [
        "forge 1.7.10",
        "forge 1.12.2",
        "forge moderno",
        "neoforge",
        "fabric",
    ] {
        assert!(
            coverage.contains(bucket),
            "falta cobertura de {bucket}: {coverage:?}"
        );
    }
}

fn coverage_bucket(loader: &str, minecraft: &str) -> String {
    match loader {
        "forge" if minecraft == "1.7.10" => "forge 1.7.10".to_owned(),
        "forge" if minecraft == "1.12.2" => "forge 1.12.2".to_owned(),
        "forge" => "forge moderno".to_owned(),
        other => other.to_owned(),
    }
}

#[test]
fn todo_achado_do_corpus_tem_evidencia_de_log_e_causa_quando_e_erro() {
    // Propriedade sobre dados reais: nada sem evidência, e todo achado principal de erro
    // aponta um arquivo da própria sessão.
    for case in cases() {
        let analysis = run(&case, &Expected::default());
        let names: BTreeSet<String> = analysis
            .sources
            .iter()
            .map(|source| source.name.clone())
            .collect();
        for entry in &analysis.findings {
            assert!(!entry.finding.evidence().is_empty());
            for evidence in entry.finding.evidence() {
                let Evidence::Log { file, .. } = evidence else {
                    panic!("{}: evidência sem log", case.display());
                };
                assert!(
                    names.contains(file),
                    "{}: {file} não é da sessão",
                    case.display()
                );
            }
        }
    }
}
