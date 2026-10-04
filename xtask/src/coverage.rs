//! `cargo xtask coverage`: cobertura de linhas com os mínimos da QUALITY §4.2.
//!
//! Rust: `cargo llvm-cov nextest` em todo o workspace (menos o próprio xtask), somado por
//! crate e comparado com `xtask/coverage.toml`. Frontend: `pnpm test:coverage`, com os
//! mínimos por pasta no `vitest.config.ts`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result, bail};
use serde::Deserialize;

use crate::util::{Cmd, desktop_dir, target_dir, workspace_root};

/// Mínimos por crate, lidos de `xtask/coverage.toml`.
#[derive(Debug, Deserialize)]
pub struct Config {
    rust: RustConfig,
}

#[derive(Debug, Deserialize)]
struct RustConfig {
    default: f64,
    #[serde(default)]
    crates: BTreeMap<String, f64>,
}

impl Config {
    /// Lê a configuração de um texto TOML.
    pub fn parse(text: &str) -> Result<Self> {
        toml::from_str(text).context("xtask/coverage.toml inválido")
    }

    fn minimum(&self, krate: &str) -> f64 {
        self.rust
            .crates
            .get(krate)
            .copied()
            .unwrap_or(self.rust.default)
    }
}

/// Linhas cobertas de uma crate.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Lines {
    /// Linhas com código.
    pub count: u64,
    /// Linhas executadas pelos testes.
    pub covered: u64,
}

impl Lines {
    /// Porcentagem coberta; `None` sem linhas.
    #[allow(clippy::cast_precision_loss)] // contagens de linhas cabem com folga num f64
    pub fn percent(self) -> Option<f64> {
        (self.count > 0).then(|| self.covered as f64 * 100.0 / self.count as f64)
    }
}

#[derive(Deserialize)]
struct Export {
    data: Vec<ExportData>,
}

#[derive(Deserialize)]
struct ExportData {
    files: Vec<ExportFile>,
}

#[derive(Deserialize)]
struct ExportFile {
    filename: PathBuf,
    summary: ExportSummary,
}

#[derive(Deserialize)]
struct ExportSummary {
    lines: ExportLines,
}

#[derive(Deserialize)]
struct ExportLines {
    count: u64,
    covered: u64,
}

/// Crate a que pertence um arquivo, pelo caminho relativo à raiz.
fn crate_of(relative: &Path) -> Option<String> {
    let parts: Vec<_> = relative
        .components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect();
    match parts.as_slice() {
        [first, name, ..] if first == "crates" => Some(name.to_string()),
        [apps, desktop, tauri, ..]
            if apps == "apps" && desktop == "desktop" && tauri == "src-tauri" =>
        {
            Some("warden-app".to_owned())
        }
        _ => None,
    }
}

/// Soma o JSON do `llvm-cov` por crate. Arquivos fora de `crates/` e da `warden-app` (como
/// dependências) são ignorados.
pub fn summarize(json: &str, root: &Path) -> Result<BTreeMap<String, Lines>> {
    let export: Export = serde_json::from_str(json).context("JSON do llvm-cov inválido")?;
    let mut totals: BTreeMap<String, Lines> = BTreeMap::new();
    for file in export.data.iter().flat_map(|data| &data.files) {
        let Ok(relative) = file.filename.strip_prefix(root) else {
            continue;
        };
        let Some(krate) = crate_of(relative) else {
            continue;
        };
        let entry = totals.entry(krate).or_default();
        entry.count += file.summary.lines.count;
        entry.covered += file.summary.lines.covered;
    }
    Ok(totals)
}

/// Resultado de uma crate.
#[derive(Debug, PartialEq)]
pub struct Verdict {
    /// Nome da crate.
    pub krate: String,
    /// Linhas.
    pub lines: Lines,
    /// Mínimo exigido.
    pub minimum: f64,
    /// Se passou (crate sem linhas passa).
    pub ok: bool,
}

/// Compara as somas com os mínimos. `crates` lista todas as crates do workspace, para que
/// as que não aparecem no relatório (sem código) também sejam mostradas.
pub fn evaluate(
    totals: &BTreeMap<String, Lines>,
    crates: &[String],
    config: &Config,
) -> Vec<Verdict> {
    crates
        .iter()
        .map(|krate| {
            let lines = totals.get(krate).copied().unwrap_or_default();
            let minimum = config.minimum(krate);
            let ok = lines
                .percent()
                .is_none_or(|percent| percent + 1e-9 >= minimum);
            Verdict {
                krate: krate.clone(),
                lines,
                minimum,
                ok,
            }
        })
        .collect()
}

fn workspace_crates() -> Result<Vec<String>> {
    let metadata = crate::deps::metadata(&workspace_root().join("Cargo.toml"))?;
    let mut names: Vec<String> = metadata
        .workspace_packages()
        .iter()
        .map(|package| package.name.to_string())
        .filter(|name| name != "xtask")
        .collect();
    names.sort();
    Ok(names)
}

/// `cargo xtask coverage`.
pub fn run() -> Result<()> {
    let root = workspace_root();
    let config_path = root.join("xtask").join("coverage.toml");
    let config = Config::parse(
        &std::fs::read_to_string(&config_path)
            .with_context(|| format!("falha ao ler {}", config_path.display()))?,
    )?;
    let output_dir = target_dir()?.join("llvm-cov");
    std::fs::create_dir_all(&output_dir)
        .with_context(|| format!("falha ao criar {}", output_dir.display()))?;
    let output = output_dir.join("warden-summary.json");
    Cmd::cargo()
        .args([
            "llvm-cov",
            "nextest",
            "--workspace",
            "--exclude",
            "xtask",
            "--locked",
        ])
        .args([
            "--no-tests=warn",
            "--json",
            "--summary-only",
            "--output-path",
        ])
        .args([&output])
        .run()?;
    let json = std::fs::read_to_string(&output)
        .with_context(|| format!("falha ao ler {}", output.display()))?;
    let verdicts = evaluate(&summarize(&json, &root)?, &workspace_crates()?, &config);

    println!("\nCobertura de linhas por crate (QUALITY §4.2):");
    let mut failed = Vec::new();
    for verdict in &verdicts {
        let percent = verdict.lines.percent().map_or_else(
            || "sem código".to_owned(),
            |percent| format!("{percent:.1}%"),
        );
        let status = if verdict.ok {
            "ok"
        } else {
            "ABAIXO DO MÍNIMO"
        };
        println!(
            "  {:<22} {:>10}  mínimo {:>3}%  {status}",
            verdict.krate, percent, verdict.minimum
        );
        if !verdict.ok {
            failed.push(verdict.krate.as_str());
        }
    }

    let frontend = Cmd::pnpm()?
        .cwd(desktop_dir())
        .args(["run", "test:coverage"])
        .run();
    if !failed.is_empty() {
        bail!("cobertura abaixo do mínimo em: {}", failed.join(", "));
    }
    frontend.context("cobertura do frontend abaixo do mínimo ou testes falhando")
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONFIG: &str =
        "[rust]\ndefault = 75\n[rust.crates]\nwarden-packwiz = 85\nwarden-app = 0\n";

    fn json(root: &Path) -> String {
        let file = |path: &str, count: u64, covered: u64| {
            serde_json::json!({
                "filename": root.join(path),
                "summary": { "lines": { "count": count, "covered": covered, "percent": 0.0 } }
            })
        };
        serde_json::json!({
            "type": "llvm.coverage.json.export",
            "version": "2.0.1",
            "data": [{
                "files": [
                    file("crates/warden-packwiz/src/lib.rs", 100, 80),
                    file("crates/warden-packwiz/src/hash.rs", 100, 92),
                    file("crates/warden-core/src/lib.rs", 40, 30),
                    file("apps/desktop/src-tauri/src/lib.rs", 50, 1),
                    file("xtask/src/main.rs", 10, 0),
                ]
            }]
        })
        .to_string()
    }

    #[test]
    fn soma_por_crate_e_ignora_o_resto() {
        let root = PathBuf::from("/repo");
        let totals = summarize(&json(&root), &root).unwrap();
        assert_eq!(
            totals["warden-packwiz"],
            Lines {
                count: 200,
                covered: 172
            }
        );
        assert_eq!(
            totals["warden-core"],
            Lines {
                count: 40,
                covered: 30
            }
        );
        assert_eq!(
            totals["warden-app"],
            Lines {
                count: 50,
                covered: 1
            }
        );
        assert!(!totals.contains_key("xtask"));
    }

    #[test]
    fn aplica_minimos_por_crate_e_padrao() {
        let root = PathBuf::from("/repo");
        let totals = summarize(&json(&root), &root).unwrap();
        let config = Config::parse(CONFIG).unwrap();
        let crates: Vec<String> = ["warden-app", "warden-core", "warden-packwiz", "warden-perf"]
            .map(String::from)
            .to_vec();
        let verdicts = evaluate(&totals, &crates, &config);
        let summary: Vec<_> = verdicts
            .iter()
            .map(|v| (v.krate.as_str(), v.minimum, v.ok))
            .collect();
        assert_eq!(
            summary,
            [
                ("warden-app", 0.0, true),
                ("warden-core", 75.0, true),
                ("warden-packwiz", 85.0, true),
                ("warden-perf", 75.0, true),
            ]
        );
        assert_eq!(verdicts[3].lines.percent(), None);

        let strict =
            Config::parse("[rust]\ndefault = 76\n[rust.crates]\nwarden-packwiz = 87\n").unwrap();
        let failed: Vec<_> = evaluate(&totals, &crates, &strict)
            .into_iter()
            .filter(|v| !v.ok)
            .map(|v| v.krate)
            .collect();
        assert_eq!(failed, ["warden-app", "warden-core", "warden-packwiz"]);
    }

    #[test]
    fn o_arquivo_de_configuracao_versionado_e_valido() {
        let text = std::fs::read_to_string(workspace_root().join("xtask/coverage.toml")).unwrap();
        let config = Config::parse(&text).unwrap();
        assert!((config.minimum("warden-packwiz") - 85.0).abs() < f64::EPSILON);
        assert!((config.minimum("warden-http") - 75.0).abs() < f64::EPSILON);
        assert!(config.minimum("warden-app").abs() < f64::EPSILON);
    }
}
