//! `cargo xtask check-kits`: confere os dados dos mods iniciais e dos kits contra a API do
//! Modrinth (ADR-0033; ROADMAP P1-18).
//!
//! Lê `crates/warden-project/data/initial-mods.toml` e `kits.toml` (ou os arquivos de
//! `--initial-mods` e `--kits`) e, para cada item e cada faixa:
//!
//! - **erro** (a saída é diferente de 0): o ID do projeto não existe no Modrinth (um slug no
//!   lugar do ID, um projeto apagado) ou a consulta não pôde ser feita (sem rede);
//! - **aviso** (só entra no relatório): o projeto existe, mas não tem versão para o loader e a
//!   versão do Minecraft da faixa. O relatório diz quais.
//!
//! Itens da CurseForge só são conferidos com `CURSEFORGE_API_KEY` no ambiente ou no `.env`; sem a
//! chave, viram uma nota do relatório. O relatório vai para a saída, para `target/check-kits/relatorio.md` e,
//! na CI, para o resumo do job (`GITHUB_STEP_SUMMARY`).
//!
//! As consultas usam o `curl` (presente no Windows 10+ e na CI) para o xtask não ganhar um cliente
//! HTTP só para isto. A chave da CurseForge vai ao `curl` por um arquivo de configuração no
//! stdin, nunca na linha de comando.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use anyhow::{Context as _, Result, bail};
use serde::Deserialize;

use crate::env_file;
use crate::util;

const USER_AGENT: &str = "Kriticales/Warden-xtask (+https://github.com/Kriticales/Warden)";
const MODRINTH: &str = "https://api.modrinth.com/v2";
const CURSEFORGE: &str = "https://api.curseforge.com/v1";

/// Opções do subcomando.
#[derive(Debug, Default)]
pub struct Options {
    /// `initial-mods.toml` (padrão: o do repositório).
    pub initial_mods: Option<PathBuf>,
    /// `kits.toml` (padrão: o do repositório).
    pub kits: Option<PathBuf>,
}

// ---------------------------------------------------------------------------------------------
// Dados (só o que a conferência usa; a validação completa é dos testes da `warden-project`)
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct InitialFile {
    #[serde(default)]
    tool: Vec<ToolData>,
}

#[derive(Debug, Deserialize)]
struct ToolData {
    id: String,
    #[serde(default)]
    check: Vec<String>,
    #[serde(default)]
    band: Vec<BandData>,
}

#[derive(Debug, Deserialize)]
struct BandData {
    loaders: Vec<String>,
    from: Option<String>,
    to: Option<String>,
    source: String,
    project: String,
    #[serde(default)]
    with: Vec<ExtraData>,
}

#[derive(Debug, Deserialize)]
struct ExtraData {
    source: String,
    project: String,
    name: String,
}

#[derive(Debug, Deserialize)]
struct KitsFile {
    #[serde(default)]
    kit: Vec<KitData>,
}

#[derive(Debug, Deserialize)]
struct KitData {
    id: String,
    loader: String,
    from: String,
    to: Option<String>,
    #[serde(default)]
    check: Vec<String>,
    #[serde(default)]
    item: Vec<KitItemData>,
}

#[derive(Debug, Deserialize)]
struct KitItemData {
    source: String,
    project: String,
    name: String,
}

// ---------------------------------------------------------------------------------------------
// O que conferir
// ---------------------------------------------------------------------------------------------

/// Uma conferência: um projeto, em loaders e versões do Minecraft.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Target {
    /// De onde vem ("kit fabric-moderno", "spark").
    origin: String,
    /// Nome para o relatório.
    name: String,
    /// `modrinth` ou `curseforge`.
    source: String,
    project: String,
    loaders: Vec<String>,
    versions: Vec<String>,
}

/// Compara versões do Minecraft número a número (`1.20` = `1.20.0`); `None` se não forem numéricas.
fn compare(a: &str, b: &str) -> Option<std::cmp::Ordering> {
    let parse = |v: &str| -> Option<Vec<u64>> { v.split('.').map(|p| p.parse().ok()).collect() };
    let (a, b) = (parse(a)?, parse(b)?);
    for index in 0..a.len().max(b.len()) {
        let order = a
            .get(index)
            .copied()
            .unwrap_or(0)
            .cmp(&b.get(index).copied().unwrap_or(0));
        if order != std::cmp::Ordering::Equal {
            return Some(order);
        }
    }
    Some(std::cmp::Ordering::Equal)
}

/// As versões a consultar: `from`, `to` e as de `check` que estão na faixa.
fn versions_of(from: Option<&str>, to: Option<&str>, check: &[String]) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    let mut push = |version: &str| {
        if !found.iter().any(|known| known == version) {
            found.push(version.to_owned());
        }
    };
    if let Some(from) = from {
        push(from);
    }
    if let Some(to) = to {
        push(to);
    }
    for version in check {
        let in_range = from
            .is_none_or(|from| compare(version, from).is_some_and(std::cmp::Ordering::is_ge))
            && to.is_none_or(|to| compare(version, to).is_some_and(std::cmp::Ordering::is_le));
        if in_range {
            push(version);
        }
    }
    found
}

fn targets(initial: &InitialFile, kits: &KitsFile) -> Vec<Target> {
    let mut targets = Vec::new();
    for tool in &initial.tool {
        for band in &tool.band {
            let versions = versions_of(band.from.as_deref(), band.to.as_deref(), &tool.check);
            targets.push(Target {
                origin: tool.id.clone(),
                name: tool.id.clone(),
                source: band.source.clone(),
                project: band.project.clone(),
                loaders: band.loaders.clone(),
                versions: versions.clone(),
            });
            for extra in &band.with {
                targets.push(Target {
                    origin: format!("{} (junto)", tool.id),
                    name: extra.name.clone(),
                    source: extra.source.clone(),
                    project: extra.project.clone(),
                    loaders: band.loaders.clone(),
                    versions: versions.clone(),
                });
            }
        }
    }
    for kit in &kits.kit {
        let versions = versions_of(Some(&kit.from), kit.to.as_deref(), &kit.check);
        for item in &kit.item {
            targets.push(Target {
                origin: format!("kit {}", kit.id),
                name: item.name.clone(),
                source: item.source.clone(),
                project: item.project.clone(),
                loaders: vec![kit.loader.clone()],
                versions: versions.clone(),
            });
        }
    }
    targets
}

// ---------------------------------------------------------------------------------------------
// A API
// ---------------------------------------------------------------------------------------------

/// O que a conferência precisa da rede. A implementação real usa o `curl`; os testes usam uma
/// falsa.
trait Api {
    /// IDs de projetos do Modrinth que existem, entre os pedidos.
    fn modrinth_existing(&self, ids: &[String]) -> Result<BTreeSet<String>>;
    /// Se o projeto do Modrinth tem versão para o loader e a versão do Minecraft.
    fn modrinth_has_version(&self, project: &str, loader: &str, minecraft: &str) -> Result<bool>;
    /// Se o projeto da CurseForge existe; `None` quando não há chave para perguntar.
    fn curseforge_exists(&self, id: &str) -> Result<Option<bool>>;
}

struct Curl {
    curseforge_key: Option<String>,
}

impl Curl {
    /// `GET` com tentativas para 429 e 5xx; devolve o código e o corpo.
    fn get(&self, url: &str, curseforge: bool) -> Result<(u16, String)> {
        let mut last = String::new();
        for attempt in 0..4 {
            if attempt > 0 {
                std::thread::sleep(Duration::from_secs(2u64.pow(attempt)));
            }
            let mut command = Command::new("curl");
            command
                .args([
                    "--silent",
                    "--show-error",
                    "--max-time",
                    "30",
                    "--user-agent",
                ])
                .arg(USER_AGENT)
                .args(["--write-out", "\n%{http_code}"]);
            if curseforge {
                command.args(["--config", "-"]);
            }
            command
                .arg(url)
                .stdin(if curseforge {
                    Stdio::piped()
                } else {
                    Stdio::null()
                })
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            let mut child = command
                .spawn()
                .context("não foi possível executar o `curl` (necessário para o check-kits)")?;
            if curseforge && let Some(mut stdin) = child.stdin.take() {
                let key = self.curseforge_key.as_deref().unwrap_or_default();
                writeln!(stdin, "header = \"x-api-key: {key}\"")?;
            }
            let output = child.wait_with_output()?;
            if !output.status.success() {
                String::from_utf8_lossy(&output.stderr)
                    .trim()
                    .clone_into(&mut last);
                continue;
            }
            let text = String::from_utf8_lossy(&output.stdout).into_owned();
            let (body, code) = text.rsplit_once('\n').unwrap_or(("", "0"));
            let code: u16 = code.trim().parse().unwrap_or(0);
            if code == 429 || code >= 500 {
                last = format!("HTTP {code}");
                continue;
            }
            return Ok((code, body.to_owned()));
        }
        bail!("sem resposta de {url}: {last}")
    }
}

fn encode(raw: &str) -> String {
    let mut out = String::new();
    for byte in raw.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            out.push(char::from(byte));
        } else {
            let _ = write!(out, "%{byte:02X}");
        }
    }
    out
}

impl Api for Curl {
    fn modrinth_existing(&self, ids: &[String]) -> Result<BTreeSet<String>> {
        let list = serde_json::to_string(ids)?;
        let (code, body) =
            self.get(&format!("{MODRINTH}/projects?ids={}", encode(&list)), false)?;
        if code != 200 {
            bail!("o Modrinth respondeu HTTP {code} à consulta de projetos");
        }
        let projects: Vec<serde_json::Value> =
            serde_json::from_str(&body).context("resposta de /projects não é uma lista JSON")?;
        Ok(projects
            .iter()
            .filter_map(|project| project["id"].as_str().map(str::to_owned))
            .collect())
    }

    fn modrinth_has_version(&self, project: &str, loader: &str, minecraft: &str) -> Result<bool> {
        let url = format!(
            "{MODRINTH}/project/{project}/version?loaders={}&game_versions={}&include_changelog=false",
            encode(&format!("[\"{loader}\"]")),
            encode(&format!("[\"{minecraft}\"]")),
        );
        let (code, body) = self.get(&url, false)?;
        if code != 200 {
            bail!("o Modrinth respondeu HTTP {code} às versões de {project}");
        }
        let versions: Vec<serde_json::Value> =
            serde_json::from_str(&body).context("resposta de /version não é uma lista JSON")?;
        Ok(!versions.is_empty())
    }

    fn curseforge_exists(&self, id: &str) -> Result<Option<bool>> {
        if self.curseforge_key.is_none() {
            return Ok(None);
        }
        let (code, _) = self.get(&format!("{CURSEFORGE}/mods/{id}"), true)?;
        match code {
            200 => Ok(Some(true)),
            404 => Ok(Some(false)),
            other => bail!("a CurseForge respondeu HTTP {other} ao projeto {id}"),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Relatório
// ---------------------------------------------------------------------------------------------

/// Resultado da conferência.
#[derive(Debug, Default, PartialEq, Eq)]
struct Report {
    errors: Vec<String>,
    warnings: Vec<String>,
    notes: Vec<String>,
    checked: usize,
}

impl Report {
    fn markdown(&self) -> String {
        let mut text = String::from("# check-kits\n\n");
        let _ = writeln!(
            text,
            "{} consultas ao Modrinth · {} erros · {} avisos\n",
            self.checked,
            self.errors.len(),
            self.warnings.len()
        );
        for (title, lines) in [
            ("Erros (o ID não existe ou a consulta falhou)", &self.errors),
            ("Avisos (sem versão para a faixa)", &self.warnings),
            ("Notas", &self.notes),
        ] {
            if lines.is_empty() {
                continue;
            }
            let _ = writeln!(text, "## {title}\n");
            for line in lines {
                let _ = writeln!(text, "- {line}");
            }
            text.push('\n');
        }
        if self.errors.is_empty() && self.warnings.is_empty() {
            text.push_str("Tudo certo: todos os itens têm versão em todas as faixas.\n");
        }
        text
    }
}

/// Confere todos os alvos.
fn check(targets: &[Target], api: &dyn Api) -> Result<Report> {
    let mut report = Report::default();
    let modrinth: BTreeSet<String> = targets
        .iter()
        .filter(|target| target.source == "modrinth")
        .map(|target| target.project.clone())
        .collect();
    let existing = if modrinth.is_empty() {
        BTreeSet::new()
    } else {
        api.modrinth_existing(&modrinth.iter().cloned().collect::<Vec<_>>())?
    };
    let mut curseforge_seen = BTreeSet::new();
    for target in targets {
        match target.source.as_str() {
            "modrinth" => {
                if !existing.contains(&target.project) {
                    report.errors.push(format!(
                        "{}: {} (`{}`) não existe no Modrinth (confira se é o ID, não o slug)",
                        target.origin, target.name, target.project
                    ));
                    continue;
                }
                for loader in &target.loaders {
                    for version in &target.versions {
                        report.checked += 1;
                        if !api.modrinth_has_version(&target.project, loader, version)? {
                            report.warnings.push(format!(
                                "{}: {} não tem versão para {loader} {version}",
                                target.origin, target.name
                            ));
                        }
                    }
                }
            }
            "curseforge" => {
                if !curseforge_seen.insert(target.project.clone()) {
                    continue;
                }
                match api.curseforge_exists(&target.project)? {
                    Some(true) => {}
                    Some(false) => report.errors.push(format!(
                        "{}: {} (CurseForge `{}`) não existe",
                        target.origin, target.name, target.project
                    )),
                    None => report.notes.push(format!(
                        "{}: {} (CurseForge `{}`) não foi conferido: falta CURSEFORGE_API_KEY",
                        target.origin, target.name, target.project
                    )),
                }
            }
            other => report.errors.push(format!(
                "{}: {} tem a fonte desconhecida `{other}`",
                target.origin, target.name
            )),
        }
    }
    Ok(report)
}

fn read_toml<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("não foi possível ler {}", path.display()))?;
    toml::from_str(&text).with_context(|| format!("{} é inválido", path.display()))
}

/// A chave da CurseForge: `CURSEFORGE_API_KEY` no ambiente ou, na máquina de desenvolvimento, no
/// `.env` do repositório principal. Sem ela, os itens da CurseForge viram nota. Nunca é impressa.
fn curseforge_key() -> Option<String> {
    let from_env = std::env::var("CURSEFORGE_API_KEY")
        .ok()
        .filter(|key| !key.trim().is_empty());
    if from_env.is_some() {
        return from_env;
    }
    let vars = env_file::load(&env_file::main_repo_env_path().ok()?).ok()??;
    vars.iter()
        .find(|(name, value)| *name == "CURSEFORGE_API_KEY" && !value.trim().is_empty())
        .map(|(_, value)| value.to_owned())
}

/// `cargo xtask check-kits`.
pub fn run(options: &Options) -> Result<()> {
    let data = util::workspace_root().join("crates/warden-project/data");
    let initial_path = options
        .initial_mods
        .clone()
        .unwrap_or_else(|| data.join("initial-mods.toml"));
    let kits_path = options
        .kits
        .clone()
        .unwrap_or_else(|| data.join("kits.toml"));
    let initial: InitialFile = read_toml(&initial_path)?;
    let kits: KitsFile = read_toml(&kits_path)?;
    let targets = targets(&initial, &kits);
    println!(
        "check-kits: {} itens em {} ferramentas e {} kits ({} e {})",
        targets.len(),
        initial.tool.len(),
        kits.kit.len(),
        initial_path.display(),
        kits_path.display()
    );
    let key = curseforge_key();
    let report = check(
        &targets,
        &Curl {
            curseforge_key: key,
        },
    )?;
    let markdown = report.markdown();
    println!("\n{markdown}");
    let out = util::target_dir()?.join("check-kits");
    std::fs::create_dir_all(&out)?;
    std::fs::write(out.join("relatorio.md"), &markdown)?;
    if let Some(summary) = std::env::var_os("GITHUB_STEP_SUMMARY") {
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(summary)?;
        file.write_all(markdown.as_bytes())?;
    }
    if !report.errors.is_empty() {
        bail!(
            "check-kits: {} erro(s) nos dados; veja o relatório acima",
            report.errors.len()
        );
    }
    println!("check-kits: ok ({} avisos)", report.warnings.len());
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    /// API falsa: projetos que existem e versões por (projeto, loader, Minecraft).
    struct Fake {
        projects: BTreeSet<String>,
        versions: BTreeMap<(String, String, String), bool>,
        curseforge: Option<bool>,
    }

    impl Api for Fake {
        fn modrinth_existing(&self, ids: &[String]) -> Result<BTreeSet<String>> {
            Ok(ids
                .iter()
                .filter(|id| self.projects.contains(*id))
                .cloned()
                .collect())
        }
        fn modrinth_has_version(&self, p: &str, l: &str, m: &str) -> Result<bool> {
            Ok(self
                .versions
                .get(&(p.to_owned(), l.to_owned(), m.to_owned()))
                .copied()
                .unwrap_or(false))
        }
        fn curseforge_exists(&self, _: &str) -> Result<Option<bool>> {
            Ok(self.curseforge)
        }
    }

    const KITS: &str = r#"
        schema = 1
        [[kit]]
        id = "teste"
        loader = "fabric"
        from = "1.21"
        check = ["1.21.1", "1.20.1"]
          [[kit.item]]
          source = "modrinth"
          project = "AANobbMI"
          name = "Sodium"
          [[kit.item]]
          source = "modrinth"
          project = "ZZZZZZZZ"
          name = "Inexistente"
    "#;

    const INITIAL: &str = r#"
        schema = 1
        [[tool]]
        id = "spark"
        check = ["1.21.1"]
          [[tool.band]]
          loaders = ["fabric"]
          from = "1.17.1"
          source = "modrinth"
          project = "l6YH9Als"
          with = [{ source = "modrinth", project = "P7dR8mSH", name = "Fabric API" }]
          [[tool.band]]
          loaders = ["forge"]
          from = "1.12.2"
          to = "1.12.2"
          source = "curseforge"
          project = "361579"
    "#;

    fn fake(curseforge: Option<bool>) -> Fake {
        let mut versions = BTreeMap::new();
        for (project, mc) in [
            ("AANobbMI", "1.21"),
            ("AANobbMI", "1.21.1"),
            ("l6YH9Als", "1.17.1"),
            ("l6YH9Als", "1.21.1"),
            ("P7dR8mSH", "1.21"),
            ("P7dR8mSH", "1.17.1"),
            ("P7dR8mSH", "1.21.1"),
        ] {
            versions.insert((project.into(), "fabric".into(), mc.into()), true);
        }
        Fake {
            projects: ["AANobbMI", "l6YH9Als", "P7dR8mSH"]
                .into_iter()
                .map(String::from)
                .collect(),
            versions,
            curseforge,
        }
    }

    fn run_on(initial: &str, kits: &str, api: &Fake) -> Report {
        let initial: InitialFile = toml::from_str(initial).unwrap();
        let kits: KitsFile = toml::from_str(kits).unwrap();
        check(&targets(&initial, &kits), api).unwrap()
    }

    #[test]
    fn versoes_consultadas_ficam_na_faixa() {
        let check = vec!["1.20.1".to_owned(), "1.21.1".to_owned(), "26.3".to_owned()];
        assert_eq!(
            versions_of(Some("1.21"), None, &check),
            ["1.21", "1.21.1", "26.3"]
        );
        assert_eq!(
            versions_of(Some("1.12.2"), Some("1.12.2"), &check),
            ["1.12.2"]
        );
    }

    #[test]
    fn id_inexistente_e_erro_e_item_sem_versao_e_aviso() {
        let report = run_on(INITIAL, KITS, &fake(Some(true)));
        assert_eq!(report.errors.len(), 1, "{:?}", report.errors);
        assert!(report.errors[0].contains("ZZZZZZZZ"));
        // O Sodium não tem versão para 1.21.1? Tem. Aviso só do que falta: nenhum aqui.
        assert!(report.warnings.is_empty(), "{:?}", report.warnings);
        assert!(report.markdown().contains("1 erros"));
    }

    #[test]
    fn dados_corretos_passam_e_aviso_nao_derruba() {
        let mut api = fake(Some(true));
        api.versions
            .remove(&("AANobbMI".into(), "fabric".into(), "1.21.1".into()));
        let kits = KITS.replace("ZZZZZZZZ", "P7dR8mSH");
        let report = run_on(INITIAL, &kits, &api);
        assert!(report.errors.is_empty(), "{:?}", report.errors);
        assert_eq!(report.warnings.len(), 1);
        assert!(report.warnings[0].contains("Sodium"));
        assert!(report.warnings[0].contains("fabric 1.21.1"));
    }

    #[test]
    fn curseforge_sem_chave_vira_nota_e_com_chave_confere() {
        let kits = KITS.replace("ZZZZZZZZ", "P7dR8mSH");
        let without = run_on(INITIAL, &kits, &fake(None));
        assert!(without.errors.is_empty());
        assert_eq!(without.notes.len(), 1);
        assert!(without.notes[0].contains("CURSEFORGE_API_KEY"));
        let missing = run_on(INITIAL, &kits, &fake(Some(false)));
        assert_eq!(missing.errors.len(), 1);
        assert!(missing.errors[0].contains("361579"));
    }

    #[test]
    fn codificacao_da_url() {
        assert_eq!(encode("[\"fabric\"]"), "%5B%22fabric%22%5D");
        assert_eq!(encode("1.21.1"), "1.21.1");
    }
}
