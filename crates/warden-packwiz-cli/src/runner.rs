//! Execução do sidecar (ARCHITECTURE §6.3).
//!
//! [`Packwiz::run`] cria o processo com o argv de [`Invocation`], ambiente limpo, entrada
//! padrão controlada e sem janela; lê stdout e stderr em bytes enquanto espera, com
//! tempo-limite e cancelamento (que matam o processo e os filhos antes de voltar). As
//! funções por comando ([`Packwiz::refresh`], [`Packwiz::curseforge_add`]…) decidem o
//! resultado pelo código de saída **e** por uma pós-condição relida do disco: o packwiz
//! escreve erros no stdout e às vezes sai com 0 em falhas (R3 §1.4).

use std::collections::{BTreeMap, VecDeque};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Stdio};
use std::time::{Duration, Instant};

use secrecy::{ExposeSecret, SecretString};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::{Child, Command};
use tracing::{debug, info, warn};
use warden_core::{CancellationToken, NoProgress, Progress, ProgressSink, resolve_inside};
use warden_packwiz::hash::{hash_bytes, hash_matches};
use warden_packwiz::{
    HashFormat, METAFILE_SUFFIX, Metafile, PACK_FILE, PackIndex, PackManifest, Value,
};

use crate::command::{CURSEFORGE_KEY_ENV, ExportSide, Invocation, PackwizCommand, Stdin, Timeouts};
use crate::error::{DETAIL_LINES, Error, Result};
use crate::output::{
    Decoded, LineDecoder, failed_downloads, manual_downloads, mentions_missing_key, redact,
};
use crate::process::ProcessTree;
use crate::staging::snapshot;

/// Linhas guardadas por execução; além disso, as mais antigas são descartadas.
pub const MAX_LINES: usize = 100_000;

/// Primeira linha que o packwiz imprime quando lê o `--config`.
const CONFIG_BANNER: &str = "Using config file:";

/// Nome do executável junto do app instalado (o Tauri tira o sufixo do triplo).
#[cfg(windows)]
pub const BINARY_NAME: &str = "packwiz.exe";
/// Nome do executável junto do app instalado (o Tauri tira o sufixo do triplo).
#[cfg(not(windows))]
pub const BINARY_NAME: &str = "packwiz";

/// Variável que aponta o executável do packwiz (testes e desenvolvimento).
pub const BINARY_ENV: &str = "WARDEN_PACKWIZ_BIN";

/// O sidecar do packwiz e as pastas do Warden que ele usa.
#[derive(Debug, Clone)]
pub struct Packwiz {
    binary: PathBuf,
    cache_dir: PathBuf,
    config_file: PathBuf,
    timeouts: Timeouts,
}

/// Contexto de uma execução: cancelamento, progresso e quem quer ver cada linha.
#[derive(Clone, Copy)]
pub struct RunContext<'a> {
    /// Cancelar mata o processo e os filhos.
    pub cancel: &'a CancellationToken,
    /// Recebe a porcentagem da barra de progresso do packwiz.
    pub progress: &'a dyn ProgressSink,
    /// Recebe cada linha limpa (já sem a chave), na ordem em que chega.
    pub on_line: Option<&'a (dyn Fn(&str) + Sync)>,
}

impl<'a> RunContext<'a> {
    /// Só com o cancelamento.
    #[must_use]
    pub fn new(cancel: &'a CancellationToken) -> Self {
        Self {
            cancel,
            progress: &NoProgress,
            on_line: None,
        }
    }

    /// Com um receptor de progresso.
    #[must_use]
    pub fn with_progress(mut self, progress: &'a dyn ProgressSink) -> Self {
        self.progress = progress;
        self
    }

    /// Com um receptor de linhas.
    #[must_use]
    pub fn with_lines(mut self, on_line: &'a (dyn Fn(&str) + Sync)) -> Self {
        self.on_line = Some(on_line);
        self
    }
}

impl std::fmt::Debug for RunContext<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RunContext")
            .field("cancelled", &self.cancel.is_cancelled())
            .field("on_line", &self.on_line.is_some())
            .finish_non_exhaustive()
    }
}

/// O que uma execução produziu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunOutput {
    /// Código de saída (`None` se o processo morreu por sinal).
    pub exit_code: Option<i32>,
    /// Linhas limpas de stdout e stderr, na ordem de chegada, sem a chave.
    pub lines: Vec<String>,
    /// Se linhas antigas foram descartadas por passar de [`MAX_LINES`].
    pub truncated: bool,
    /// Duração.
    pub elapsed: Duration,
}

impl RunOutput {
    /// As últimas [`DETAIL_LINES`] linhas.
    #[must_use]
    pub fn tail(&self) -> Vec<String> {
        let start = self.lines.len().saturating_sub(DETAIL_LINES);
        self.lines[start..].to_vec()
    }
}

/// Resultado de um `refresh` conferido.
#[derive(Debug, Clone)]
pub struct RefreshReport {
    /// O índice relido depois do refresh.
    pub index: PackIndex,
    /// A saída do packwiz.
    pub output: RunOutput,
}

/// O que adicionar da CurseForge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CurseForgeTarget {
    /// Link de arquivo.
    Url(String),
    /// Projeto e arquivo escolhidos no Warden.
    Ids {
        /// ID do projeto.
        addon_id: u32,
        /// ID do arquivo.
        file_id: u32,
    },
}

/// Resultado de um `curseforge add` conferido.
#[derive(Debug, Clone)]
pub struct AddReport {
    /// Metafiles criados ou alterados (caminhos relativos, com `/`), em ordem.
    pub metafiles: Vec<String>,
    /// A saída do packwiz.
    pub output: RunOutput,
}

/// Resultado de uma exportação conferida.
#[derive(Debug, Clone)]
pub struct ExportReport {
    /// O arquivo gerado.
    pub path: PathBuf,
    /// A saída do packwiz.
    pub output: RunOutput,
}

impl Packwiz {
    /// O sidecar em `binary`, com o cache (`--cache`) e a config vazia (`--config`) do Warden
    /// (`AppPaths::packwiz_cache_dir` e `AppPaths::packwiz_config_file`).
    #[must_use]
    pub fn new(binary: PathBuf, cache_dir: PathBuf, config_file: PathBuf) -> Self {
        Self {
            binary,
            cache_dir,
            config_file,
            timeouts: Timeouts::default(),
        }
    }

    /// Troca os tempos-limite.
    #[must_use]
    pub fn with_timeouts(mut self, timeouts: Timeouts) -> Self {
        self.timeouts = timeouts;
        self
    }

    /// O executável.
    #[must_use]
    pub fn binary(&self) -> &Path {
        &self.binary
    }

    /// Onde o sidecar está: [`BINARY_ENV`], se definida e não vazia; senão, [`BINARY_NAME`]
    /// na pasta do executável do app (ARCHITECTURE §6.3).
    ///
    /// Erros: [`Error::BinaryNotFound`] se o arquivo não existir; [`Error::Internal`] se o
    /// sistema não informar o executável atual.
    pub fn locate_binary() -> Result<PathBuf> {
        let candidate = match std::env::var_os(BINARY_ENV).filter(|value| !value.is_empty()) {
            Some(path) => PathBuf::from(path),
            None => std::env::current_exe()
                .map_err(|error| Error::Internal(format!("executável atual: {error}")))?
                .with_file_name(BINARY_NAME),
        };
        if candidate.is_file() {
            Ok(candidate)
        } else {
            Err(Error::BinaryNotFound { path: candidate })
        }
    }

    /// Executa `command` no pack em `pack_dir` e devolve a saída, qualquer que seja o código
    /// de saída. `key` só é usada pelos comandos que falam com a CurseForge.
    ///
    /// Volta só depois de o processo ter terminado; no cancelamento e no tempo-limite, o
    /// processo e os filhos são mortos antes.
    ///
    /// Erros: [`Error::PackFileMissing`], [`Error::BinaryNotFound`], [`Error::Spawn`],
    /// [`Error::Cancelled`], [`Error::Timeout`], [`Error::Io`].
    pub async fn run(
        &self,
        pack_dir: &Path,
        command: &PackwizCommand,
        key: Option<&SecretString>,
        ctx: RunContext<'_>,
    ) -> Result<RunOutput> {
        let label = command.label();
        if !pack_dir.join(PACK_FILE).is_file() {
            return Err(Error::PackFileMissing {
                dir: pack_dir.to_path_buf(),
            });
        }
        if !self.binary.is_file() {
            return Err(Error::BinaryNotFound {
                path: self.binary.clone(),
            });
        }
        self.prepare_dirs()?;
        let invocation = Invocation::new(
            command,
            pack_dir,
            &self.cache_dir,
            &self.config_file,
            &self.timeouts,
            |name| std::env::var_os(name),
        );
        if ctx.cancel.is_cancelled() {
            return Err(Error::Cancelled { command: label });
        }
        let secret = key
            .map(|key| key.expose_secret().trim())
            .filter(|key| !key.is_empty());
        let mut process = self.process(&invocation, secret);
        let mut tree = ProcessTree::new().map_err(|source| self.spawn_error(source))?;
        debug!(
            command = label,
            args = %invocation.display_args(),
            cwd = %invocation.cwd.display(),
            "iniciando o packwiz"
        );
        let started = Instant::now();
        let mut child = process.spawn().map_err(|source| self.spawn_error(source))?;
        if let Err(source) = tree.attach(&child) {
            let _ = child.start_kill();
            let _ = child.wait().await;
            return Err(self.spawn_error(source));
        }
        if let Stdin::Answers(answers) = invocation.stdin
            && let Some(mut stdin) = child.stdin.take()
        {
            // O processo pode sair sem ler: cano quebrado não é erro.
            let _ = stdin.write_all(answers).await;
            drop(stdin);
        }

        let mut collector = Collector::new(label, secret, ctx);
        let status = supervise(&mut child, &tree, invocation.timeout, &mut collector).await;
        let elapsed = started.elapsed();
        let exit_code = match status {
            Ok(status) => status.code(),
            Err(Stop::Cancelled) => {
                info!(
                    command = label,
                    elapsed_ms = millis(elapsed),
                    "packwiz cancelado"
                );
                return Err(Error::Cancelled { command: label });
            }
            Err(Stop::Timeout) => {
                let seconds = invocation.timeout.as_secs();
                warn!(command = label, seconds, "packwiz passou do tempo-limite");
                return Err(Error::Timeout {
                    command: label,
                    seconds,
                });
            }
            Err(Stop::Io(source)) => return Err(Error::io("esperar", &self.binary, source)),
        };
        info!(
            command = label,
            exit_code,
            elapsed_ms = millis(elapsed),
            "packwiz terminou"
        );
        let (lines, truncated) = collector.finish();
        Ok(RunOutput {
            exit_code,
            lines,
            truncated,
            elapsed,
        })
    }

    /// [`Self::run`] seguido da detecção de falha pela saída e pelo código: chave ausente,
    /// downloads manuais, código diferente de zero e downloads que falharam.
    ///
    /// Erros: os de [`Self::run`] e [`Error::CurseForgeKeyMissing`],
    /// [`Error::ManualDownloads`], [`Error::CommandFailed`], [`Error::DownloadFailed`].
    pub async fn run_checked(
        &self,
        pack_dir: &Path,
        command: &PackwizCommand,
        key: Option<&SecretString>,
        ctx: RunContext<'_>,
    ) -> Result<RunOutput> {
        let output = self.run(pack_dir, command, key, ctx).await?;
        check_output(command, output)
    }

    /// `packwiz refresh` (ou `refresh --build`) no pack, conferido: o `pack.toml` e o índice
    /// relidos são válidos, o hash do índice no `pack.toml` confere com o arquivo e todas
    /// as entradas existem.
    ///
    /// Erros: os de [`Self::run_checked`] e [`Error::Postcondition`].
    pub async fn refresh(
        &self,
        pack_dir: &Path,
        build: bool,
        ctx: RunContext<'_>,
    ) -> Result<RefreshReport> {
        let command = PackwizCommand::Refresh { build };
        let output = self.run_checked(pack_dir, &command, None, ctx).await?;
        match verify_index(pack_dir, build) {
            Ok(index) => Ok(RefreshReport { index, output }),
            Err(reason) => Err(postcondition(&command, reason, &output)),
        }
    }

    /// `packwiz curseforge add` no pack (que deve ser uma cópia de staging: o packwiz
    /// sobrescreve arquivos em silêncio, ARCHITECTURE §6.1), respondendo `n` à pergunta das
    /// dependências. Conferido: pelo menos um metafile novo ou alterado, todos válidos.
    ///
    /// Erros: os de [`Self::run_checked`] e [`Error::Postcondition`].
    pub async fn curseforge_add(
        &self,
        pack_dir: &Path,
        target: &CurseForgeTarget,
        key: Option<&SecretString>,
        ctx: RunContext<'_>,
    ) -> Result<AddReport> {
        let command = match target {
            CurseForgeTarget::Url(url) => PackwizCommand::CurseForgeAddUrl { url: url.clone() },
            CurseForgeTarget::Ids { addon_id, file_id } => PackwizCommand::CurseForgeAddIds {
                addon_id: *addon_id,
                file_id: *file_id,
            },
        };
        let is_metafile = |path: &str| path.ends_with(METAFILE_SUFFIX);
        let before = snapshot(pack_dir, is_metafile)?;
        let output = self.run_checked(pack_dir, &command, key, ctx).await?;
        let after = snapshot(pack_dir, is_metafile)?;
        match changed_metafiles(pack_dir, &before, &after) {
            Ok(metafiles) => Ok(AddReport { metafiles, output }),
            Err(reason) => Err(postcondition(&command, reason, &output)),
        }
    }

    /// `packwiz list`: os nomes dos metafiles, na ordem do packwiz (nome sem diferenciar
    /// maiúsculas).
    ///
    /// Erros: os de [`Self::run_checked`].
    pub async fn list(&self, pack_dir: &Path, ctx: RunContext<'_>) -> Result<Vec<String>> {
        let output = self
            .run_checked(pack_dir, &PackwizCommand::List, None, ctx)
            .await?;
        let mut lines = output.lines;
        if lines
            .first()
            .is_some_and(|line| line.starts_with(CONFIG_BANNER))
        {
            lines.remove(0);
        }
        Ok(lines)
    }

    /// `packwiz modrinth export` no pack (que deve ser uma cópia de staging: a exportação
    /// reescreve o índice). Um arquivo antigo em `output` é apagado antes. Conferido: o
    /// arquivo existe e é um zip com `modrinth.index.json`.
    ///
    /// Erros: os de [`Self::run_checked`], [`Error::Postcondition`] e [`Error::Io`].
    pub async fn modrinth_export(
        &self,
        pack_dir: &Path,
        output: &Path,
        key: Option<&SecretString>,
        ctx: RunContext<'_>,
    ) -> Result<ExportReport> {
        let command = PackwizCommand::ModrinthExport {
            output: output.to_path_buf(),
        };
        self.export(pack_dir, &command, output, "modrinth.index.json", key, ctx)
            .await
    }

    /// `packwiz curseforge export --side <lado>` no pack (cópia de staging). Conferido: o
    /// arquivo existe e é um zip com `manifest.json`.
    ///
    /// Erros: os de [`Self::run_checked`], [`Error::Postcondition`] e [`Error::Io`].
    pub async fn curseforge_export(
        &self,
        pack_dir: &Path,
        side: ExportSide,
        output: &Path,
        key: Option<&SecretString>,
        ctx: RunContext<'_>,
    ) -> Result<ExportReport> {
        let command = PackwizCommand::CurseForgeExport {
            side,
            output: output.to_path_buf(),
        };
        self.export(pack_dir, &command, output, "manifest.json", key, ctx)
            .await
    }

    async fn export(
        &self,
        pack_dir: &Path,
        command: &PackwizCommand,
        output: &Path,
        manifest: &str,
        key: Option<&SecretString>,
        ctx: RunContext<'_>,
    ) -> Result<ExportReport> {
        let path = pack_dir.join(output);
        match fs::remove_file(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(source) => return Err(Error::io("apagar", &path, source)),
        }
        let run = self.run_checked(pack_dir, command, key, ctx).await?;
        match verify_zip(&path, manifest) {
            Ok(()) => Ok(ExportReport { path, output: run }),
            Err(reason) => Err(postcondition(command, reason, &run)),
        }
    }

    /// O comando do sistema para `invocation`: argv em vetor (nunca `cmd /c`), ambiente
    /// limpo, chave só quando o comando a usa, canos de saída e árvore de processos.
    fn process(&self, invocation: &Invocation, secret: Option<&str>) -> Command {
        let mut process = Command::new(&self.binary);
        process
            .args(&invocation.args)
            .current_dir(&invocation.cwd)
            .env_clear()
            .envs(invocation.env.iter().map(|(name, value)| (*name, value)))
            .stdin(match invocation.stdin {
                Stdin::Closed => Stdio::null(),
                Stdin::Answers(_) => Stdio::piped(),
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if invocation.pass_curseforge_key
            && let Some(secret) = secret
        {
            process.env(CURSEFORGE_KEY_ENV, secret);
        }
        ProcessTree::configure(&mut process);
        process
    }

    fn prepare_dirs(&self) -> Result<()> {
        fs::create_dir_all(&self.cache_dir)
            .map_err(|source| Error::io("criar", &self.cache_dir, source))?;
        if !self.config_file.is_file() {
            // Config vazia: isola o packwiz das configurações globais do usuário.
            if let Some(parent) = self.config_file.parent() {
                fs::create_dir_all(parent).map_err(|source| Error::io("criar", parent, source))?;
            }
            warden_core::atomic_write(&self.config_file, b"")
                .map_err(|error| Error::io("gravar", &self.config_file, io::Error::other(error)))?;
        }
        Ok(())
    }

    fn spawn_error(&self, source: io::Error) -> Error {
        Error::Spawn {
            binary: self.binary.clone(),
            source,
        }
    }
}

/// Por que a espera parou antes de o processo terminar sozinho.
enum Stop {
    Cancelled,
    Timeout,
    Io(io::Error),
}

/// Lê stdout e stderr enquanto espera o processo, até ele sair e os dois canos fecharem.
/// No cancelamento e no tempo-limite, mata a árvore e espera o processo antes de voltar.
async fn supervise(
    child: &mut Child,
    tree: &ProcessTree,
    timeout: Duration,
    collector: &mut Collector<'_>,
) -> std::result::Result<ExitStatus, Stop> {
    let (Some(mut stdout), Some(mut stderr)) = (child.stdout.take(), child.stderr.take()) else {
        kill(tree, child).await;
        return Err(Stop::Io(io::Error::other("saída do packwiz não capturada")));
    };
    let cancel = collector.ctx.cancel;
    let mut out_decoder = LineDecoder::new();
    let mut err_decoder = LineDecoder::new();
    // No heap: dois buffers de 8 KiB deixariam o future grande demais (`large_futures`).
    let mut out_buffer = vec![0_u8; 8192];
    let mut err_buffer = vec![0_u8; 8192];
    let (mut out_open, mut err_open) = (true, true);
    let mut status = None;
    let deadline = tokio::time::sleep(timeout);
    tokio::pin!(deadline);
    while out_open || err_open || status.is_none() {
        tokio::select! {
            biased;
            () = cancel.cancelled() => {
                kill(tree, child).await;
                return Err(Stop::Cancelled);
            }
            () = &mut deadline => {
                kill(tree, child).await;
                return Err(Stop::Timeout);
            }
            read = stdout.read(&mut out_buffer), if out_open => match read {
                Ok(0) | Err(_) => out_open = false,
                Ok(count) => collector.take(out_decoder.push(&out_buffer[..count])),
            },
            read = stderr.read(&mut err_buffer), if err_open => match read {
                Ok(0) | Err(_) => err_open = false,
                Ok(count) => collector.take(err_decoder.push(&err_buffer[..count])),
            },
            exit = child.wait(), if status.is_none() => {
                status = Some(exit.map_err(Stop::Io)?);
                // Um filho que sobrou seguraria os canos abertos: a árvore morre junto.
                tree.kill();
            }
        }
    }
    collector.take(out_decoder.finish());
    collector.take(err_decoder.finish());
    status.ok_or_else(|| Stop::Io(io::Error::other("processo sem código de saída")))
}

/// Mata a árvore e espera o processo principal.
async fn kill(tree: &ProcessTree, child: &mut Child) {
    tree.kill();
    let _ = child.start_kill();
    let _ = child.wait().await;
}

fn millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

/// Junta as linhas de stdout e stderr, apaga a chave, registra em `debug` e repassa ao
/// contexto.
struct Collector<'a> {
    label: &'static str,
    secret: Option<&'a str>,
    ctx: RunContext<'a>,
    lines: VecDeque<String>,
    truncated: bool,
}

impl<'a> Collector<'a> {
    fn new(label: &'static str, secret: Option<&'a str>, ctx: RunContext<'a>) -> Self {
        Self {
            label,
            secret,
            ctx,
            lines: VecDeque::new(),
            truncated: false,
        }
    }

    fn take(&mut self, decoded: Vec<Decoded>) {
        for item in decoded {
            match item {
                Decoded::Line(line) => {
                    let line = redact(&line, self.secret);
                    debug!(command = self.label, "packwiz: {line}");
                    if let Some(on_line) = self.ctx.on_line {
                        on_line(&line);
                    }
                    if self.lines.len() == MAX_LINES {
                        self.lines.pop_front();
                        self.truncated = true;
                    }
                    self.lines.push_back(line);
                }
                Decoded::Progress { percent, .. } => {
                    self.ctx
                        .progress
                        .progress(Progress::items(u64::from(percent), Some(100)));
                }
            }
        }
    }

    fn finish(self) -> (Vec<String>, bool) {
        (self.lines.into(), self.truncated)
    }
}

/// Decide o resultado pela saída e pelo código (ordem: chave ausente, downloads manuais,
/// código, downloads que falharam).
pub(crate) fn check_output(command: &PackwizCommand, output: RunOutput) -> Result<RunOutput> {
    let label = command.label();
    if command.needs_curseforge_key() && mentions_missing_key(&output.lines) {
        return Err(Error::CurseForgeKeyMissing { command: label });
    }
    if let Some(files) = manual_downloads(&output.lines) {
        return Err(Error::ManualDownloads {
            command: label,
            files,
        });
    }
    if output.exit_code != Some(0) {
        return Err(Error::CommandFailed {
            command: label,
            exit_code: output.exit_code.unwrap_or(-1),
            tail: output.tail(),
        });
    }
    let failed = failed_downloads(&output.lines);
    if !failed.is_empty() {
        return Err(Error::DownloadFailed {
            command: label,
            files: failed,
            tail: output.tail(),
        });
    }
    Ok(output)
}

fn postcondition(command: &PackwizCommand, reason: String, output: &RunOutput) -> Error {
    Error::Postcondition {
        command: command.label(),
        reason,
        tail: output.tail(),
    }
}

/// Relê `pack.toml` e o índice e confere o hash do índice e a existência das entradas.
/// Com a opção `no-internal-hashes` e sem `--build`, o packwiz grava o hash do índice vazio
/// (`Pack.UpdateIndexHash`): é o que se espera nesse caso. Devolve o motivo em texto quando
/// algo não confere.
pub(crate) fn verify_index(pack_dir: &Path, build: bool) -> std::result::Result<PackIndex, String> {
    let pack_path = pack_dir.join(PACK_FILE);
    let pack_text =
        fs::read_to_string(&pack_path).map_err(|error| format!("pack.toml ilegível: {error}"))?;
    let manifest = PackManifest::parse(&pack_text)
        .map_err(|error| format!("pack.toml inválido: {error}"))?
        .value;
    let index_path = resolve_inside(pack_dir, &manifest.index.file)
        .map_err(|error| format!("caminho do índice inválido: {error}"))?;
    let index_bytes = fs::read(&index_path)
        .map_err(|error| format!("{} ilegível: {error}", manifest.index.file))?;
    let index_text = String::from_utf8(index_bytes)
        .map_err(|_| format!("{} não é UTF-8", manifest.index.file))?;
    let index = PackIndex::parse(&index_text)
        .map_err(|error| format!("{} inválido: {error}", manifest.index.file))?
        .value;
    let no_internal_hashes =
        manifest.option("no-internal-hashes") == Some(&Value::Boolean(true)) && !build;
    let format = HashFormat::parse(&manifest.index.hash_format)
        .map_err(|error| format!("formato do hash do índice: {error}"))?;
    let actual = hash_bytes(format, index_text.as_bytes());
    let expected_empty = no_internal_hashes && manifest.index.hash.is_empty();
    if !expected_empty && !hash_matches(format, &manifest.index.hash, &actual) {
        return Err(format!(
            "o hash do índice no pack.toml ({}) não confere com {} ({actual})",
            manifest.index.hash, manifest.index.file
        ));
    }
    let index_dir = index_path.parent().unwrap_or(pack_dir);
    for entry in &index.files {
        let exists = resolve_inside(index_dir, &entry.file).is_ok_and(|path| path.is_file());
        if !exists {
            return Err(format!(
                "a entrada {:?} do índice não existe no disco",
                entry.file
            ));
        }
    }
    Ok(index)
}

/// Metafiles novos ou alterados entre dois retratos; todos precisam ser válidos.
fn changed_metafiles(
    pack_dir: &Path,
    before: &BTreeMap<String, String>,
    after: &BTreeMap<String, String>,
) -> std::result::Result<Vec<String>, String> {
    let changed: Vec<String> = after
        .iter()
        .filter(|(path, hash)| before.get(*path) != Some(hash))
        .map(|(path, _)| path.clone())
        .collect();
    if changed.is_empty() {
        return Err("nenhum metafile foi criado".to_owned());
    }
    for path in &changed {
        let text = fs::read_to_string(pack_dir.join(path))
            .map_err(|error| format!("{path} ilegível: {error}"))?;
        Metafile::parse(&text).map_err(|error| format!("{path} inválido: {error}"))?;
    }
    Ok(changed)
}

/// O arquivo existe e é um zip com a entrada `manifest`.
fn verify_zip(path: &Path, manifest: &str) -> std::result::Result<(), String> {
    let file = fs::File::open(path)
        .map_err(|error| format!("o arquivo {} não foi gerado: {error}", path.display()))?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|error| format!("{} não é um zip válido: {error}", path.display()))?;
    archive
        .by_name(manifest)
        .map(|_| ())
        .map_err(|_| format!("{} não tem {manifest}", path.display()))
}

#[cfg(test)]
mod tests {
    use std::io::Write as _;

    use super::*;

    fn output(exit_code: Option<i32>, lines: &[&str]) -> RunOutput {
        RunOutput {
            exit_code,
            lines: lines.iter().map(|line| (*line).to_owned()).collect(),
            truncated: false,
            elapsed: Duration::ZERO,
        }
    }

    #[test]
    fn sucesso_com_codigo_zero() {
        let command = PackwizCommand::Refresh { build: false };
        assert!(check_output(&command, output(Some(0), &["Index refreshed!"])).is_ok());
    }

    #[test]
    fn codigo_diferente_de_zero_falha_com_as_linhas() {
        let command = PackwizCommand::Refresh { build: false };
        let error =
            check_output(&command, output(Some(1), &["Loading modpack...", "erro"])).unwrap_err();
        let Error::CommandFailed {
            exit_code, tail, ..
        } = &error
        else {
            panic!("{error}")
        };
        assert_eq!(*exit_code, 1);
        assert_eq!(tail, &["Loading modpack...", "erro"]);
        let error = check_output(&command, output(None, &[])).unwrap_err();
        assert!(matches!(error, Error::CommandFailed { exit_code: -1, .. }));
    }

    #[test]
    fn chave_ausente_so_nos_comandos_da_curseforge() {
        let line =
            "Failed: WARDEN_CURSEFORGE_API_KEY ausente: a chave da CurseForge não foi informada.";
        let add = PackwizCommand::CurseForgeAddUrl { url: "u".into() };
        let error = check_output(&add, output(Some(1), &[line])).unwrap_err();
        assert!(
            matches!(error, Error::CurseForgeKeyMissing { .. }),
            "{error}"
        );
        // Um refresh não fala com a CurseForge: a mesma linha é só uma falha comum.
        let refresh = PackwizCommand::Refresh { build: false };
        let error = check_output(&refresh, output(Some(1), &[line])).unwrap_err();
        assert!(matches!(error, Error::CommandFailed { .. }), "{error}");
    }

    #[test]
    fn downloads_manuais_e_downloads_que_falharam() {
        let export = PackwizCommand::ModrinthExport {
            output: "x.mrpack".into(),
        };
        let error = check_output(
            &export,
            output(
                Some(1),
                &[
                    "Found 1 manual downloads; these mods are unable to be downloaded by packwiz (due to API limitations) and must be manually downloaded:",
                    "OptiFine (OptiFine.jar) from https://www.curseforge.com/x",
                    "Once you have done so, place these files in C:\\c and re-run this command.",
                ],
            ),
        )
        .unwrap_err();
        assert!(matches!(&error, Error::ManualDownloads { files, .. } if files.len() == 1));
        // Exportação incompleta com código 0 (R3 §1.4) também é falha.
        let error = check_output(
            &export,
            output(
                Some(0),
                &[
                    "Download of A (a.jar) failed: 404",
                    "B (b.jar) added to zip",
                ],
            ),
        )
        .unwrap_err();
        assert!(matches!(&error, Error::DownloadFailed { files, .. } if files == &["A (a.jar)"]));
    }

    #[test]
    fn cauda_tem_no_maximo_200_linhas() {
        let lines: Vec<String> = (0..450).map(|n| n.to_string()).collect();
        let run = RunOutput {
            exit_code: Some(1),
            lines,
            truncated: false,
            elapsed: Duration::ZERO,
        };
        let tail = run.tail();
        assert_eq!(tail.len(), DETAIL_LINES);
        assert_eq!(tail[0], "250");
        assert_eq!(tail[199], "449");
    }

    fn write_pack(dir: &Path, index_text: &str, index_hash: Option<&str>) {
        let mut manifest = PackManifest::new("Teste", "1.21.1");
        manifest.index.hash = index_hash.map_or_else(
            || hash_bytes(HashFormat::Sha256, index_text.as_bytes()),
            str::to_owned,
        );
        fs::write(dir.join("pack.toml"), manifest.to_toml_string()).unwrap();
        fs::write(dir.join("index.toml"), index_text).unwrap();
    }

    #[test]
    fn indice_conferido() {
        let temp = tempfile::tempdir().unwrap();
        fs::create_dir_all(temp.path().join("config")).unwrap();
        fs::write(temp.path().join("config/a.txt"), "a").unwrap();
        let mut index = PackIndex::default();
        index.files.push(warden_packwiz::IndexEntry::new(
            "config/a.txt",
            &hash_bytes(HashFormat::Sha256, b"a"),
        ));
        write_pack(temp.path(), &index.to_toml_string(), None);
        let read = verify_index(temp.path(), false).unwrap();
        assert_eq!(read.files.len(), 1);
    }

    #[test]
    fn indice_com_hash_errado_ou_entrada_faltando() {
        let temp = tempfile::tempdir().unwrap();
        let mut index = PackIndex::default();
        index
            .files
            .push(warden_packwiz::IndexEntry::new("config/sumiu.txt", "00"));
        let text = index.to_toml_string();
        write_pack(temp.path(), &text, None);
        let reason = verify_index(temp.path(), false).unwrap_err();
        assert!(reason.contains("config/sumiu.txt"), "{reason}");
        write_pack(temp.path(), &text, Some("ab"));
        let reason = verify_index(temp.path(), false).unwrap_err();
        assert!(reason.contains("não confere"), "{reason}");
        fs::write(temp.path().join("pack.toml"), "isto não é toml = = =").unwrap();
        assert!(
            verify_index(temp.path(), false)
                .unwrap_err()
                .contains("pack.toml")
        );
        fs::remove_file(temp.path().join("pack.toml")).unwrap();
        assert!(
            verify_index(temp.path(), false)
                .unwrap_err()
                .contains("pack.toml")
        );
    }

    #[test]
    fn metafiles_alterados() {
        let temp = tempfile::tempdir().unwrap();
        fs::create_dir_all(temp.path().join("mods")).unwrap();
        let before = BTreeMap::from([("mods/a.pw.toml".to_owned(), "1".to_owned())]);
        let same = before.clone();
        assert!(changed_metafiles(temp.path(), &before, &same).is_err());
        fs::write(temp.path().join("mods/b.pw.toml"), "name = \"B\"\nfilename = \"b.jar\"\nside = \"both\"\n\n[download]\nurl = \"https://x/b.jar\"\nhash-format = \"sha1\"\nhash = \"aa\"\n").unwrap();
        let after = BTreeMap::from([
            ("mods/a.pw.toml".to_owned(), "1".to_owned()),
            ("mods/b.pw.toml".to_owned(), "2".to_owned()),
        ]);
        assert_eq!(
            changed_metafiles(temp.path(), &before, &after).unwrap(),
            ["mods/b.pw.toml"]
        );
        fs::write(temp.path().join("mods/b.pw.toml"), "= quebrado").unwrap();
        assert!(
            changed_metafiles(temp.path(), &before, &after)
                .unwrap_err()
                .contains("inválido")
        );
    }

    #[test]
    fn zip_conferido() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("x.mrpack");
        assert!(
            verify_zip(&path, "modrinth.index.json")
                .unwrap_err()
                .contains("não foi gerado")
        );
        fs::write(&path, "não é zip").unwrap();
        assert!(
            verify_zip(&path, "modrinth.index.json")
                .unwrap_err()
                .contains("zip válido")
        );
        let mut writer = zip::ZipWriter::new(fs::File::create(&path).unwrap());
        writer
            .start_file(
                "modrinth.index.json",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
        writer.write_all(b"{}").unwrap();
        writer.finish().unwrap();
        assert!(verify_zip(&path, "modrinth.index.json").is_ok());
        assert!(
            verify_zip(&path, "manifest.json")
                .unwrap_err()
                .contains("não tem")
        );
    }
}
