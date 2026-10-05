//! `cargo xtask preview`: versão de teste do Warden para o dono, no Windows (F0-04; ADR-0048).
//!
//! - Sem opções: compila o sidecar do packwiz (se mudou), gera o instalador NSIS com
//!   `cargo tauri build --bundles nsis` e instala.
//! - `--from-ci [--branch <nome>]`: não compila; escolhe com o `gh` a execução mais recente da
//!   CI que terminou com sucesso e tem o artefato `warden-windows-<sha>`, baixa o instalador
//!   para `<target>/preview/` e instala.
//!
//! Nos dois casos: mostra a versão e o commit que vão ser instalados (e a versão instalada
//! agora), fecha o Warden aberto depois de pedir confirmação, instala só para o usuário atual
//! sem pedir administrador (`/S`), confere que os dados do app (`%APPDATA%` e `%LOCALAPPDATA%`
//! `\dev.kriticales.warden\`) não mudaram, confere o sidecar `packwiz.exe` ao lado do executável
//! e abre o Warden instalado. O passo a passo para o dono está em `docs/DEV-WINDOWS.md`.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io::{BufRead as _, Write as _};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime};

use anyhow::{Context as _, Result, bail, ensure};
use serde::Deserialize;

use crate::packwiz;
use crate::util::{Cmd, desktop_dir, target_dir, warn_long_target, workspace_root};

/// Nome do produto (`productName` do `tauri.conf.json`): prefixo do instalador e chave de
/// desinstalação.
pub const PRODUCT_NAME: &str = "Warden";

/// Identificador do app (`identifier` do `tauri.conf.json`): nome das pastas de dados.
pub const IDENTIFIER: &str = "dev.kriticales.warden";

/// Executável principal instalado (o binário `warden-app` do cargo).
pub const MAIN_BINARY: &str = "warden-app.exe";

/// Sidecar do packwiz instalado ao lado do executável (F0-03).
pub const SIDECAR: &str = "packwiz.exe";

/// Prefixo do artefato da CI Windows (`windows.yml`): `warden-windows-<sha>`.
pub const ARTIFACT_PREFIX: &str = "warden-windows-";

/// Chave de desinstalação que o instalador NSIS do Tauri grava para o usuário atual.
const UNINSTALL_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\Warden";

/// Quantas execuções da CI listar, e de quantas com sucesso conferir os artefatos.
const RUNS_TO_LIST: u32 = 30;
const RUNS_TO_INSPECT: usize = 10;

/// Quanto esperar o Warden fechar depois do pedido de fechamento.
const CLOSE_TIMEOUT: Duration = Duration::from_secs(20);

/// Frase para quando o `gh` não está conectado à conta do GitHub.
pub const NOT_AUTHENTICATED: &str = "O GitHub CLI (gh) não está conectado à sua conta do GitHub. \
     Abra o PowerShell, rode `gh auth login`, siga as instruções e tente de novo. Nada foi instalado.";

/// Frase para quando o `gh` não existe no `PATH`.
pub const GH_MISSING: &str = "O GitHub CLI (gh) não foi encontrado neste computador, e ele é \
     necessário para baixar a versão da CI. Peça ao orquestrador para conferir a instalação do gh, \
     ou gere a versão aqui mesmo com `cargo xtask preview` (mais demorado). Nada foi instalado.";

/// Opções do `cargo xtask preview`.
pub struct Options {
    /// Baixa o instalador da CI em vez de compilar.
    pub from_ci: bool,
    /// Branch da CI (padrão: `main`).
    pub branch: Option<String>,
    /// Pasta de instalação (padrão: a do instalador, `%LOCALAPPDATA%\Warden`, ou a da
    /// instalação anterior).
    pub install_dir: Option<PathBuf>,
}

/// Instalador pronto para instalar.
struct Installer {
    path: PathBuf,
    version: String,
    commit: String,
    origin: String,
}

/// `cargo xtask preview`.
pub fn run(options: &Options) -> Result<()> {
    ensure!(
        cfg!(windows),
        "preview: a versão de teste é só para o Windows (ADR-0048)."
    );
    let installer = if options.from_ci {
        let branch = options.branch.as_deref().unwrap_or("main");
        installer_from_ci(branch)?
    } else {
        installer_built_here()?
    };

    println!();
    println!(
        "preview: vai instalar o Warden {}, commit {}",
        installer.version, installer.commit
    );
    println!("preview: origem: {}", installer.origin);
    println!("preview: instalador: {}", installer.path.display());
    match reg_value(UNINSTALL_KEY, "DisplayVersion") {
        Some(version) => println!("preview: versão instalada agora: {version}"),
        None => println!("preview: nenhuma versão do Warden instalada agora."),
    }
    println!();

    close_running_warden()?;

    let data_dirs = data_dirs()?;
    let before = data_dirs
        .iter()
        .map(|dir| snapshot(dir))
        .collect::<Result<Vec<_>>>()?;

    install(&installer.path, options.install_dir.as_deref())?;

    let mut changes = Vec::new();
    for (dir, before) in data_dirs.iter().zip(&before) {
        changes.extend(
            compare(before, &snapshot(dir)?)
                .into_iter()
                .map(|change| format!("{change} (em {})", dir.display())),
        );
    }
    ensure!(
        changes.is_empty(),
        "ATENÇÃO: a instalação mudou os dados do app, o que não devia acontecer. Não abra o \
         Warden e mande esta mensagem ao orquestrador:\n  {}",
        changes.join("\n  ")
    );
    let files: usize = before.iter().map(BTreeMap::len).sum();
    println!("preview: dados do app intactos ({files} arquivo(s)):");
    for dir in &data_dirs {
        println!("  {}", dir.display());
    }

    let dir = installed_dir(options.install_dir.as_deref())?;
    let exe = dir.join(MAIN_BINARY);
    ensure!(
        exe.is_file(),
        "o instalador terminou, mas {} não existe. Mande esta mensagem ao orquestrador.",
        exe.display()
    );
    ensure!(
        dir.join(SIDECAR).is_file(),
        "o Warden foi instalado sem o {SIDECAR} ao lado de {MAIN_BINARY} (em {}). Mande esta \
         mensagem ao orquestrador.",
        dir.display()
    );
    println!("preview: {MAIN_BINARY} e {SIDECAR} em {}", dir.display());

    open(&exe)?;
    println!();
    println!(
        "preview: pronto. O Warden {} (commit {}) está instalado e aberto.",
        installer.version, installer.commit
    );
    println!("preview: confira a versão e o commit na tela Sobre do app.");
    Ok(())
}

// --- Instalador compilado aqui -------------------------------------------------------------

/// Gera o instalador com o `tauri build` e devolve o que acabou de ser gerado.
fn installer_built_here() -> Result<Installer> {
    warn_long_target();
    ensure!(
        desktop_dir().join("node_modules").exists(),
        "preview: dependências do frontend ausentes. Rode `cargo xtask setup` antes."
    );
    packwiz::run(false)?;
    let started = SystemTime::now();
    Cmd::cargo()
        .cwd(desktop_dir())
        .args(["tauri", "build", "--bundles", "nsis"])
        .run()?;
    let bundle_dir = target_dir()?.join("release").join("bundle").join("nsis");
    let path = newest_installer(&bundle_dir, started)?;
    let version = installer_version(&path)?;

    let commit = Cmd::new("git")
        .args(["rev-parse", "--short=12", "HEAD"])
        .read()?
        .trim()
        .to_owned();
    let dirty = !Cmd::new("git")
        .args(["status", "--porcelain"])
        .read()?
        .trim()
        .is_empty();
    let branch = Cmd::new("git")
        .args(["branch", "--show-current"])
        .read()
        .map(|branch| branch.trim().to_owned())
        .unwrap_or_default();
    let mut origin = format!(
        "compilado neste computador a partir de {}",
        workspace_root().display()
    );
    if !branch.is_empty() {
        origin.push_str(", branch ");
        origin.push_str(&branch);
    }
    if dirty {
        origin.push_str(" (com alterações ainda sem commit)");
    }
    Ok(Installer {
        path,
        version,
        commit,
        origin,
    })
}

/// Versão lida do nome do instalador do Tauri: `Warden_<versão>_<arquitetura>-setup.exe`.
pub fn version_from_installer_name(name: &str) -> Option<String> {
    let rest = name
        .strip_prefix(PRODUCT_NAME)?
        .strip_prefix('_')?
        .strip_suffix("-setup.exe")?;
    let (version, arch) = rest.rsplit_once('_')?;
    (!version.is_empty() && !arch.is_empty()).then(|| version.to_owned())
}

fn installer_version(path: &Path) -> Result<String> {
    path.file_name()
        .and_then(|name| version_from_installer_name(&name.to_string_lossy()))
        .with_context(|| format!("nome de instalador inesperado: {}", path.display()))
}

/// Instaladores (`Warden_*-setup.exe`) de uma pasta.
fn installers_in(dir: &Path) -> Result<Vec<PathBuf>> {
    let entries =
        std::fs::read_dir(dir).with_context(|| format!("falha ao ler {}", dir.display()))?;
    let mut found = Vec::new();
    for entry in entries {
        let path = entry?.path();
        let is_installer = path
            .file_name()
            .is_some_and(|name| version_from_installer_name(&name.to_string_lossy()).is_some());
        if is_installer && path.is_file() {
            found.push(path);
        }
    }
    found.sort();
    Ok(found)
}

/// O instalador mais novo da pasta, que precisa ter sido gravado a partir de `since` (com 2 s
/// de folga para a resolução do relógio do sistema de arquivos).
pub fn newest_installer(dir: &Path, since: SystemTime) -> Result<PathBuf> {
    let mut newest: Option<(SystemTime, PathBuf)> = None;
    for path in installers_in(dir)? {
        let modified = std::fs::metadata(&path)?.modified()?;
        if newest.as_ref().is_none_or(|(time, _)| modified > *time) {
            newest = Some((modified, path));
        }
    }
    let (modified, path) =
        newest.with_context(|| format!("nenhum instalador do Warden em {}", dir.display()))?;
    let limit = since.checked_sub(Duration::from_secs(2)).unwrap_or(since);
    ensure!(
        modified >= limit,
        "o instalador {} é de um build anterior; o `tauri build` não gerou um novo",
        path.display()
    );
    Ok(path)
}

// --- Instalador da CI ----------------------------------------------------------------------

/// Execução da CI, como o `gh run list --json` a descreve.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Run {
    /// Identificador da execução.
    pub database_id: u64,
    /// Commit da execução.
    pub head_sha: String,
    /// `queued`, `in_progress`, `completed`...
    pub status: String,
    /// `success`, `failure`, `cancelled`... (vazia enquanto não termina).
    #[serde(default)]
    pub conclusion: String,
    /// Nome do workflow (`CI`, `Nightly`...).
    #[serde(default)]
    pub workflow_name: String,
    /// Página da execução.
    #[serde(default)]
    pub url: String,
}

impl Run {
    fn succeeded(&self) -> bool {
        self.status == "completed" && self.conclusion == "success"
    }

    fn describe(&self) -> String {
        let state = if self.status == "completed" {
            match self.conclusion.as_str() {
                "success" => "terminou com sucesso",
                "failure" => "falhou",
                "cancelled" => "foi cancelada",
                other => other,
            }
        } else {
            "ainda está rodando"
        };
        format!(
            "{} do commit {} {state} ({})",
            self.workflow_name,
            short_sha(&self.head_sha),
            self.url
        )
    }
}

#[derive(Deserialize)]
struct ArtifactList {
    artifacts: Vec<Artifact>,
}

#[derive(Deserialize)]
struct Artifact {
    name: String,
    #[serde(default)]
    expired: bool,
}

/// Acesso ao GitHub pelo `gh` (simulado nos testes).
pub trait Gh {
    /// `gh auth status` terminou bem.
    fn authenticated(&self) -> Result<bool>;
    /// JSON do `gh run list` da branch, das mais novas para as mais antigas.
    fn run_list(&self, branch: &str, limit: u32) -> Result<String>;
    /// JSON da API de artefatos de uma execução.
    fn run_artifacts(&self, run_id: u64) -> Result<String>;
}

/// O `gh` de verdade, rodado na raiz do repositório (ele descobre o repositório pelo git).
struct GhCli {
    program: PathBuf,
}

impl Gh for GhCli {
    fn authenticated(&self) -> Result<bool> {
        // A saída (conta e token mascarado) não é mostrada; só o código de saída importa.
        let output = Command::new(&self.program)
            .args(["auth", "status"])
            .current_dir(workspace_root())
            .output()
            .context("não foi possível executar `gh auth status`")?;
        Ok(output.status.success())
    }

    fn run_list(&self, branch: &str, limit: u32) -> Result<String> {
        Cmd::new(&self.program)
            .args(["run", "list", "--branch", branch, "--limit"])
            .args([limit.to_string()])
            .args([
                "--json",
                "databaseId,headSha,status,conclusion,workflowName,url",
            ])
            .read()
            .context(
                "não foi possível consultar a CI no GitHub. Confira a internet e tente de novo; \
                 nada foi instalado",
            )
    }

    fn run_artifacts(&self, run_id: u64) -> Result<String> {
        Cmd::new(&self.program)
            .args(["api"])
            .args([format!(
                "repos/{{owner}}/{{repo}}/actions/runs/{run_id}/artifacts"
            )])
            .read()
            .context(
                "não foi possível consultar os artefatos da CI no GitHub. Confira a internet e \
                 tente de novo; nada foi instalado",
            )
    }
}

/// Execução escolhida e o nome do artefato com o instalador.
#[derive(Debug)]
pub struct CiChoice {
    /// Execução com sucesso mais recente que tem o artefato.
    pub run: Run,
    /// `warden-windows-<sha>`.
    pub artifact: String,
    /// Uma execução mais nova que ainda não terminou, se houver.
    pub newer_unfinished: Option<Run>,
}

/// Escolhe a execução mais recente da branch que terminou com sucesso e ainda tem o artefato
/// do instalador. Execuções sem o artefato (as das branches só rodam o Linux) são puladas.
pub fn choose_from_ci(gh: &dyn Gh, branch: &str) -> Result<CiChoice> {
    if !gh.authenticated()? {
        bail!(NOT_AUTHENTICATED);
    }
    let runs: Vec<Run> = serde_json::from_str(&gh.run_list(branch, RUNS_TO_LIST)?)
        .context("resposta inesperada do `gh run list`")?;
    let Some(latest) = runs.first() else {
        bail!(
            "A CI ainda não rodou nenhuma vez na branch `{branch}`, então não há instalador para \
             baixar. Confira o nome da branch, ou gere a versão aqui mesmo com \
             `cargo xtask preview`. Nada foi instalado."
        );
    };
    let mut inspected = 0;
    for (index, run) in runs.iter().enumerate() {
        if !run.succeeded() {
            continue;
        }
        if inspected == RUNS_TO_INSPECT {
            break;
        }
        inspected += 1;
        let list: ArtifactList = serde_json::from_str(&gh.run_artifacts(run.database_id)?)
            .context("resposta inesperada da API de artefatos do GitHub")?;
        let expected = format!("{ARTIFACT_PREFIX}{}", run.head_sha);
        if list
            .artifacts
            .iter()
            .any(|artifact| artifact.name == expected && !artifact.expired)
        {
            let newer_unfinished = runs[..index]
                .iter()
                .find(|newer| newer.status != "completed")
                .cloned();
            return Ok(CiChoice {
                run: run.clone(),
                artifact: expected,
                newer_unfinished,
            });
        }
    }
    bail!(
        "Nenhuma execução da CI terminada com sucesso na branch `{branch}` tem o instalador do \
         Windows (artefato `{ARTIFACT_PREFIX}<commit>`; ele só é gerado na `main`, em commits \
         com `[ci windows]` e à noite, e vence em 30 dias). A execução mais recente: {}. Espere \
         uma execução terminar com sucesso, ou gere a versão aqui mesmo com \
         `cargo xtask preview`. Nada foi instalado.",
        latest.describe()
    )
}

/// Baixa o instalador da CI para `<target>/preview/<artefato>/`.
fn installer_from_ci(branch: &str) -> Result<Installer> {
    let program = which::which("gh").map_err(|_| anyhow::anyhow!(GH_MISSING))?;
    let gh = GhCli { program };
    println!("preview: procurando o instalador mais recente da CI na branch `{branch}`...");
    let choice = choose_from_ci(&gh, branch)?;
    if let Some(newer) = &choice.newer_unfinished {
        println!(
            "preview: AVISO: há uma execução mais nova que ainda não terminou: {}. Vai ser \
             instalada a anterior.",
            newer.describe()
        );
    }

    let dir = target_dir()?.join("preview").join(&choice.artifact);
    if dir.exists() {
        std::fs::remove_dir_all(&dir)
            .with_context(|| format!("falha ao limpar {}", dir.display()))?;
    }
    std::fs::create_dir_all(&dir).with_context(|| format!("falha ao criar {}", dir.display()))?;
    Cmd::new(&gh.program)
        .args(["run", "download"])
        .args([choice.run.database_id.to_string()])
        .args(["--name", &choice.artifact, "--dir"])
        .args([&dir])
        .run()
        .context(
            "não foi possível baixar o instalador da CI. Confira a internet e tente de novo; \
             nada foi instalado",
        )?;
    let mut found = installers_in(&dir)?;
    ensure!(
        found.len() == 1,
        "o artefato {} devia ter um instalador do Warden e tem {}",
        choice.artifact,
        found.len()
    );
    let path = found.remove(0);
    let version = installer_version(&path)?;
    Ok(Installer {
        path,
        version,
        commit: short_sha(&choice.run.head_sha).to_owned(),
        origin: format!("CI do GitHub, branch `{branch}`, {}", choice.run.describe()),
    })
}

/// Os 12 primeiros caracteres do commit, como o app mostra (`build.rs` da `warden-app`).
pub fn short_sha(sha: &str) -> &str {
    sha.get(..12).unwrap_or(sha)
}

// --- Warden aberto ---------------------------------------------------------------------------

/// PIDs do executável na saída do `tasklist /FO CSV /NH`. Linhas de aviso (traduzidas pelo
/// Windows, como "INFORMAÇÕES: Não há tarefas...") não são CSV e ficam de fora.
pub fn parse_tasklist(output: &str, image: &str) -> Vec<u32> {
    output
        .lines()
        .filter_map(|line| {
            let mut fields = line.trim().strip_prefix('"')?.split("\",\"");
            let name = fields.next()?;
            let pid = fields.next()?.trim_end_matches('"');
            name.eq_ignore_ascii_case(image)
                .then(|| pid.parse().ok())
                .flatten()
        })
        .collect()
}

fn running_warden() -> Result<Vec<u32>> {
    let output = Command::new("tasklist")
        .args([
            "/FI",
            &format!("IMAGENAME eq {MAIN_BINARY}"),
            "/FO",
            "CSV",
            "/NH",
        ])
        .output()
        .context("não foi possível executar `tasklist`")?;
    // O `tasklist` escreve na página de código do console; só os nomes e PIDs importam.
    Ok(parse_tasklist(
        &String::from_utf8_lossy(&output.stdout),
        MAIN_BINARY,
    ))
}

/// Resposta afirmativa à pergunta de confirmação.
pub fn confirmed(answer: &str) -> bool {
    matches!(
        answer.trim().to_lowercase().as_str(),
        "s" | "sim" | "y" | "yes"
    )
}

/// Fecha o Warden aberto, com confirmação. Sem confirmação, nada é instalado.
fn close_running_warden() -> Result<()> {
    let pids = running_warden()?;
    if pids.is_empty() {
        return Ok(());
    }
    println!(
        "preview: o Warden está aberto ({} cópia(s)). Para instalar, ele precisa ser fechado; \
         isso fecha todas as janelas do Warden, inclusive as de desenvolvimento.",
        pids.len()
    );
    print!("Fechar o Warden agora? [s/N] ");
    std::io::stdout().flush()?;
    let mut answer = String::new();
    std::io::stdin().lock().read_line(&mut answer)?;
    ensure!(
        confirmed(&answer),
        "Instalação cancelada: o Warden continua aberto. Nada foi instalado."
    );

    // Sem `/F`: o Windows pede para a janela fechar, como no botão X.
    let _ = Command::new("taskkill")
        .args(["/IM", MAIN_BINARY])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    let start = Instant::now();
    while start.elapsed() < CLOSE_TIMEOUT {
        if running_warden()?.is_empty() {
            println!("preview: Warden fechado.");
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    bail!(
        "O Warden não fechou em {} s (talvez esteja pedindo uma confirmação na janela dele). \
         Feche-o e rode de novo. Nada foi instalado.",
        CLOSE_TIMEOUT.as_secs()
    )
}

// --- Dados do app ----------------------------------------------------------------------------

/// Retrato de uma pasta: caminho relativo → (tamanho, data de modificação).
pub type Snapshot = BTreeMap<PathBuf, (u64, Option<SystemTime>)>;

/// Pastas de dados do app instalado (configuração e dados locais).
fn data_dirs() -> Result<Vec<PathBuf>> {
    ["APPDATA", "LOCALAPPDATA"]
        .into_iter()
        .map(|name| {
            let base = std::env::var_os(name).with_context(|| format!("{name} não definida"))?;
            Ok(PathBuf::from(base).join(IDENTIFIER))
        })
        .collect()
}

/// Retrato dos arquivos de uma pasta (vazio se ela não existe). Um arquivo que some entre a
/// listagem e a leitura (antivírus, QUALITY §13.9) não entra no retrato.
pub fn snapshot(root: &Path) -> Result<Snapshot> {
    let mut files = Snapshot::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        let entries = match std::fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(error).with_context(|| format!("falha ao ler {}", dir.display()));
            }
        };
        for entry in entries {
            let entry = entry?;
            let metadata = match entry.metadata() {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => return Err(error.into()),
            };
            let path = entry.path();
            if metadata.is_dir() {
                pending.push(path);
            } else {
                let relative = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
                files.insert(relative, (metadata.len(), metadata.modified().ok()));
            }
        }
    }
    Ok(files)
}

/// Diferenças entre dois retratos, uma frase por arquivo.
pub fn compare(before: &Snapshot, after: &Snapshot) -> Vec<String> {
    let mut changes = Vec::new();
    for (path, old) in before {
        match after.get(path) {
            None => changes.push(format!("apagado: {}", path.display())),
            Some(new) if new != old => changes.push(format!("alterado: {}", path.display())),
            Some(_) => {}
        }
    }
    for path in after.keys().filter(|path| !before.contains_key(*path)) {
        changes.push(format!("novo: {}", path.display()));
    }
    changes
}

// --- Instalação ------------------------------------------------------------------------------

/// Roda o instalador em modo silencioso, só para o usuário atual (sem administrador).
fn install(installer: &Path, dir: Option<&Path>) -> Result<()> {
    let mut command = Command::new(installer);
    command.arg("/S");
    let mut shown = format!("{} /S", installer.display());
    if let Some(dir) = dir {
        let dir = std::path::absolute(dir)
            .with_context(|| format!("pasta de instalação inválida: {}", dir.display()))?;
        let mut option = OsString::from("/D=");
        option.push(&dir);
        shown.push_str(" /D=");
        shown.push_str(&dir.to_string_lossy());
        // O NSIS exige o `/D=` por último e sem aspas, mesmo com espaços no caminho.
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt as _;
            command.raw_arg(&option);
        }
        #[cfg(not(windows))]
        command.arg(&option);
    }
    println!("> {shown}");
    let status = command
        .status()
        .with_context(|| format!("não foi possível executar {}", installer.display()))?;
    ensure!(
        status.success(),
        "o instalador terminou com {status}. Mande esta mensagem ao orquestrador."
    );
    Ok(())
}

/// Lê um valor da saída do `reg query` (`    InstallLocation    REG_SZ    "C:\x y"`), sem as
/// aspas que o instalador do Tauri põe em volta de caminhos.
pub fn parse_reg_value(output: &str, name: &str) -> Option<String> {
    output.lines().find_map(|line| {
        let line = line.trim();
        let rest = line.get(name.len()..)?;
        if !line[..name.len()].eq_ignore_ascii_case(name) || !rest.starts_with(char::is_whitespace)
        {
            return None;
        }
        let rest = rest.trim_start();
        let (kind, value) = rest.split_once(char::is_whitespace)?;
        kind.starts_with("REG_")
            .then(|| value.trim().trim_matches('"').to_owned())
            .filter(|value| !value.is_empty())
    })
}

fn reg_value(key: &str, name: &str) -> Option<String> {
    let output = Command::new("reg")
        .args(["query", key, "/v", name])
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    parse_reg_value(&String::from_utf8_lossy(&output.stdout), name)
}

/// Pasta onde o Warden ficou instalado.
fn installed_dir(requested: Option<&Path>) -> Result<PathBuf> {
    if let Some(dir) = requested {
        return Ok(std::path::absolute(dir)?);
    }
    if let Some(location) = reg_value(UNINSTALL_KEY, "InstallLocation") {
        return Ok(PathBuf::from(location));
    }
    let local = std::env::var_os("LOCALAPPDATA").context("LOCALAPPDATA não definida")?;
    Ok(PathBuf::from(local).join(PRODUCT_NAME))
}

/// Abre o Warden instalado, desligado deste terminal (fechar a janela não fecha o app).
fn open(exe: &Path) -> Result<()> {
    let mut command = Command::new(exe);
    command
        .current_dir(exe.parent().unwrap_or(Path::new(".")))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt as _;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        command.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
    }
    command
        .spawn()
        .with_context(|| format!("não foi possível abrir {}", exe.display()))?;
    println!("preview: Warden aberto.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::HashMap;

    use super::*;

    #[test]
    fn versao_vem_do_nome_do_instalador() {
        assert_eq!(
            version_from_installer_name("Warden_0.1.0_x64-setup.exe").as_deref(),
            Some("0.1.0")
        );
        assert_eq!(
            version_from_installer_name("Warden_1.2.3-beta.1_x64-setup.exe").as_deref(),
            Some("1.2.3-beta.1")
        );
        for name in [
            "Warden_x64-setup.exe",
            "Warden__x64-setup.exe",
            "Warden_0.1.0_x64.msi",
            "Outro_0.1.0_x64-setup.exe",
            "Warden_0.1.0_-setup.exe",
        ] {
            assert_eq!(version_from_installer_name(name), None, "{name}");
        }
    }

    #[test]
    fn constantes_batem_com_a_configuracao_do_app() {
        let conf: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(desktop_dir().join("src-tauri").join("tauri.conf.json"))
                .unwrap(),
        )
        .unwrap();
        assert_eq!(conf["productName"], PRODUCT_NAME);
        assert_eq!(conf["identifier"], IDENTIFIER);
        assert!(
            conf["bundle"]["externalBin"]
                .as_array()
                .unwrap()
                .iter()
                .any(|bin| bin == "binaries/packwiz")
        );
        assert!(UNINSTALL_KEY.ends_with(&format!(r"\{PRODUCT_NAME}")));
        let manifest =
            std::fs::read_to_string(desktop_dir().join("src-tauri").join("Cargo.toml")).unwrap();
        let stem = MAIN_BINARY.strip_suffix(".exe").unwrap();
        assert!(manifest.contains(&format!("default-run = \"{stem}\"")));
        assert_eq!(SIDECAR, "packwiz.exe");
    }

    #[test]
    fn tasklist_so_conta_o_warden() {
        let output = "\"warden-app.exe\",\"1234\",\"Console\",\"1\",\"120.000 K\"\r\n\
                      \"WARDEN-APP.EXE\",\"88\",\"Console\",\"1\",\"9 K\"\r\n\
                      \"outro.exe\",\"5\",\"Console\",\"1\",\"9 K\"\r\n";
        assert_eq!(parse_tasklist(output, MAIN_BINARY), vec![1234, 88]);
        let none = "INFORMA\u{c7}\u{d5}ES: N\u{e3}o h\u{e1} tarefas em execu\u{e7}\u{e3}o.\r\n";
        assert!(parse_tasklist(none, MAIN_BINARY).is_empty());
        assert!(parse_tasklist("", MAIN_BINARY).is_empty());
    }

    #[test]
    fn confirmacao_so_com_sim() {
        for yes in ["s", "S\r\n", " sim ", "y", "YES"] {
            assert!(confirmed(yes), "{yes:?}");
        }
        for no in ["", "\n", "n", "não", "talvez", "ss"] {
            assert!(!confirmed(no), "{no:?}");
        }
    }

    #[test]
    fn valor_do_registro_com_espacos_e_aspas() {
        let output = "\r\nHKEY_CURRENT_USER\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\Warden\r\n    \
                      InstallLocation    REG_SZ    \"C:\\Users\\Fulano de Tal\\AppData\\Local\\Warden\"\r\n    \
                      DisplayVersion    REG_SZ    0.1.0\r\n";
        assert_eq!(
            parse_reg_value(output, "InstallLocation").as_deref(),
            Some(r"C:\Users\Fulano de Tal\AppData\Local\Warden")
        );
        assert_eq!(
            parse_reg_value(output, "displayversion").as_deref(),
            Some("0.1.0")
        );
        assert_eq!(parse_reg_value(output, "Display"), None);
        assert_eq!(parse_reg_value(output, "Publisher"), None);
        assert_eq!(parse_reg_value("", "DisplayVersion"), None);
    }

    #[test]
    fn retrato_acusa_arquivo_apagado_alterado_ou_novo() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join(IDENTIFIER);
        assert!(snapshot(&root).unwrap().is_empty(), "pasta ausente");

        std::fs::create_dir_all(root.join("logs")).unwrap();
        std::fs::write(root.join("settings.json"), "{}").unwrap();
        std::fs::write(root.join("logs").join("a.log"), "linha").unwrap();
        std::fs::write(root.join("apagar.txt"), "x").unwrap();
        let before = snapshot(&root).unwrap();
        assert_eq!(before.len(), 3);
        assert!(compare(&before, &snapshot(&root).unwrap()).is_empty());

        std::fs::write(root.join("settings.json"), "{\"a\":1}").unwrap();
        std::fs::remove_file(root.join("apagar.txt")).unwrap();
        std::fs::write(root.join("logs").join("b.log"), "").unwrap();
        let mut changes = compare(&before, &snapshot(&root).unwrap());
        changes.sort();
        let new_log = Path::new("logs").join("b.log");
        assert_eq!(
            changes,
            vec![
                "alterado: settings.json".to_owned(),
                "apagado: apagar.txt".to_owned(),
                format!("novo: {}", new_log.display()),
            ]
        );
    }

    #[test]
    fn instalador_mais_novo_e_recusa_o_antigo() {
        let dir = tempfile::tempdir().unwrap();
        assert!(newest_installer(dir.path(), SystemTime::now()).is_err());

        let old = dir.path().join("Warden_0.0.9_x64-setup.exe");
        std::fs::write(&old, "a").unwrap();
        let old_time = SystemTime::now() - Duration::from_secs(3600);
        std::fs::File::options()
            .write(true)
            .open(&old)
            .unwrap()
            .set_modified(old_time)
            .unwrap();
        std::fs::write(dir.path().join("leia-me.txt"), "b").unwrap();

        let since = SystemTime::now();
        let error = newest_installer(dir.path(), since).unwrap_err();
        assert!(error.to_string().contains("build anterior"), "{error}");

        let new = dir.path().join("Warden_0.1.0_x64-setup.exe");
        std::fs::write(&new, "c").unwrap();
        assert_eq!(newest_installer(dir.path(), since).unwrap(), new);
        assert_eq!(installer_version(&new).unwrap(), "0.1.0");
    }

    #[test]
    fn commit_curto_tem_doze_caracteres() {
        assert_eq!(
            short_sha("cb8197cd92f8df01d43acf3d185ae5ad87f1aad0"),
            "cb8197cd92f8"
        );
        assert_eq!(short_sha("abc"), "abc");
    }

    // --- `--from-ci` com respostas simuladas do `gh` ---

    struct FakeGh {
        authenticated: bool,
        runs: String,
        artifacts: HashMap<u64, String>,
        calls: RefCell<Vec<String>>,
    }

    impl FakeGh {
        fn new(runs: &str) -> Self {
            Self {
                authenticated: true,
                runs: runs.to_owned(),
                artifacts: HashMap::new(),
                calls: RefCell::new(Vec::new()),
            }
        }

        fn with_artifacts(mut self, run_id: u64, names: &[(&str, bool)]) -> Self {
            let list: Vec<_> = names
                .iter()
                .map(|(name, expired)| serde_json::json!({"name": name, "expired": expired}))
                .collect();
            self.artifacts.insert(
                run_id,
                serde_json::json!({"total_count": list.len(), "artifacts": list}).to_string(),
            );
            self
        }
    }

    impl Gh for FakeGh {
        fn authenticated(&self) -> Result<bool> {
            self.calls.borrow_mut().push("auth".into());
            Ok(self.authenticated)
        }

        fn run_list(&self, branch: &str, limit: u32) -> Result<String> {
            self.calls
                .borrow_mut()
                .push(format!("list {branch} {limit}"));
            Ok(self.runs.clone())
        }

        fn run_artifacts(&self, run_id: u64) -> Result<String> {
            self.calls.borrow_mut().push(format!("artifacts {run_id}"));
            Ok(self
                .artifacts
                .get(&run_id)
                .cloned()
                .unwrap_or_else(|| r#"{"total_count":0,"artifacts":[]}"#.into()))
        }
    }

    fn run_json(id: u64, sha: &str, status: &str, conclusion: &str) -> serde_json::Value {
        serde_json::json!({
            "databaseId": id,
            "headSha": sha,
            "status": status,
            "conclusion": conclusion,
            "workflowName": "CI",
            "url": format!("https://github.com/Kriticales/Warden/actions/runs/{id}"),
        })
    }

    fn runs(list: &[serde_json::Value]) -> String {
        serde_json::Value::Array(list.to_vec()).to_string()
    }

    const SHA_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const SHA_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const SHA_C: &str = "cccccccccccccccccccccccccccccccccccccccc";
    const SHA_D: &str = "dddddddddddddddddddddddddddddddddddddddd";

    #[test]
    fn sem_gh_autenticado_diz_o_que_fazer_e_nao_consulta_nada() {
        let mut gh = FakeGh::new("[]");
        gh.authenticated = false;
        let error = choose_from_ci(&gh, "main").unwrap_err().to_string();
        assert!(error.contains("gh auth login"), "{error}");
        assert!(error.contains("Nada foi instalado"), "{error}");
        assert_eq!(*gh.calls.borrow(), vec!["auth".to_owned()]);
    }

    #[test]
    fn branch_sem_execucoes() {
        let gh = FakeGh::new("[]");
        let error = choose_from_ci(&gh, "minha-branch").unwrap_err().to_string();
        assert!(error.contains("`minha-branch`"), "{error}");
        assert!(error.contains("cargo xtask preview"), "{error}");
        assert!(error.contains("Nada foi instalado"), "{error}");
    }

    #[test]
    fn so_execucoes_sem_sucesso_nao_instala_nada() {
        let gh = FakeGh::new(&runs(&[
            run_json(3, SHA_C, "in_progress", ""),
            run_json(2, SHA_B, "completed", "failure"),
            run_json(1, SHA_A, "completed", "cancelled"),
        ]))
        // Artefato de uma execução que falhou não serve.
        .with_artifacts(2, &[(&format!("{ARTIFACT_PREFIX}{SHA_B}"), false)]);
        let error = choose_from_ci(&gh, "main").unwrap_err().to_string();
        assert!(error.contains("ainda está rodando"), "{error}");
        assert!(error.contains("cccccccccccc"), "{error}");
        assert!(error.contains("Nada foi instalado"), "{error}");
        assert!(
            !gh.calls
                .borrow()
                .iter()
                .any(|call| call.starts_with("artifacts")),
            "não consulta artefatos de execuções sem sucesso"
        );
    }

    #[test]
    fn escolhe_a_execucao_com_sucesso_mais_recente_que_tem_o_instalador() {
        let gh = FakeGh::new(&runs(&[
            run_json(4, SHA_D, "in_progress", ""),
            // Sucesso, mas só com o Linux (sem artefato).
            run_json(3, SHA_C, "completed", "success"),
            // Sucesso, artefato vencido.
            run_json(2, SHA_B, "completed", "success"),
            run_json(1, SHA_A, "completed", "success"),
        ]))
        .with_artifacts(2, &[(&format!("{ARTIFACT_PREFIX}{SHA_B}"), true)])
        .with_artifacts(
            1,
            &[
                (&format!("{ARTIFACT_PREFIX}{SHA_B}"), false),
                (&format!("{ARTIFACT_PREFIX}{SHA_A}"), false),
            ],
        );
        let choice = choose_from_ci(&gh, "main").unwrap();
        assert_eq!(choice.run.database_id, 1);
        assert_eq!(choice.artifact, format!("{ARTIFACT_PREFIX}{SHA_A}"));
        assert_eq!(choice.newer_unfinished.map(|run| run.database_id), Some(4));
        assert_eq!(
            *gh.calls.borrow(),
            vec![
                "auth".to_owned(),
                format!("list main {RUNS_TO_LIST}"),
                "artifacts 3".to_owned(),
                "artifacts 2".to_owned(),
                "artifacts 1".to_owned(),
            ]
        );
    }

    #[test]
    fn artefato_de_outro_commit_nao_serve() {
        let gh = FakeGh::new(&runs(&[run_json(1, SHA_A, "completed", "success")]))
            .with_artifacts(1, &[(&format!("{ARTIFACT_PREFIX}{SHA_B}"), false)]);
        let error = choose_from_ci(&gh, "main").unwrap_err().to_string();
        assert!(error.contains("terminou com sucesso"), "{error}");
    }

    #[test]
    fn confere_no_maximo_algumas_execucoes() {
        let list: Vec<_> = (1..=20)
            .rev()
            .map(|id| run_json(id, SHA_A, "completed", "success"))
            .collect();
        let gh = FakeGh::new(&runs(&list));
        assert!(choose_from_ci(&gh, "main").is_err());
        let consulted = gh
            .calls
            .borrow()
            .iter()
            .filter(|call| call.starts_with("artifacts"))
            .count();
        assert_eq!(consulted, RUNS_TO_INSPECT);
    }

    #[test]
    fn resposta_estranha_do_gh_vira_erro() {
        let gh = FakeGh::new("isto não é JSON");
        let error = format!("{:#}", choose_from_ci(&gh, "main").unwrap_err());
        assert!(error.contains("gh run list"), "{error}");
    }
}
