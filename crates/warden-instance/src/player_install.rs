//! "Testar como o jogador recebe" (SPEC T13; ROADMAP L-07; ARCHITECTURE §8.5): instalar o
//! pack do zero pelo **link do pack**, com o packwiz-installer-bootstrap real, e dizer em
//! linguagem simples o que o jogador recebeu de diferente do pack.
//!
//! As peças, na ordem em que o teste usa:
//!
//! - [`LocalPackServer`]: pack ainda não publicado. O Warden exporta o estado atual e serve a
//!   pasta só em `127.0.0.1`, numa porta livre, enquanto o teste dura.
//! - [`InstallerCommand`]: `java -jar bootstrap --bootstrap-no-update --bootstrap-main-jar
//!   <installer> -g -s client --pack-folder <instância> <link>`, com o Java do teste e sem
//!   janela. O `--bootstrap-no-update` impede a consulta à API do GitHub.
//! - [`blocked_notices`]: mods da CurseForge bloqueados para apps de terceiros. Sem janela, o
//!   installer lista cada um ("This mod is excluded from the CurseForge API…") e desiste da
//!   instalação inteira; o jogador, com a janela, veria a mesma lista e teria de baixar à mão.
//!   [`supply_blocked`] faz o que o jogador faria: põe na instância os arquivos que o Warden já
//!   tem no cache de downloads (os dos downloads manuais da T20), e o installer roda de novo.
//! - [`Received`]: o `packwiz.json` que o installer grava na instância (hash do `pack.toml`
//!   recebido e cada arquivo instalado).
//! - [`compare`]: o que o jogador recebeu × o pack ([`PlayerReport`]): arquivo faltando,
//!   bloqueado, lado errado, opcional, a mais e o Crash Assistant (decisão D16).

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt as _, AsyncReadExt as _, AsyncWriteExt as _, BufReader};
use tokio::net::{TcpListener, TcpStream};
use warden_core::CancellationToken;
use warden_http::{ExpectedHash, Url};
use warden_packwiz::{HashFormat, Metafile, Side, clean_path};

use crate::blocked::BlockedFile;
use crate::downloads::DownloadCache;
use crate::error::{Error, Result};
use crate::installer_cache::InstallerJars;
use crate::player_tools::{PlayerToolsMode, excluded_paths};

/// Etapa "Instalando pelo link do pack, como um jogador".
pub const STAGE_INSTALL: &str = "test.player.install";
/// Texto da etapa.
pub const STAGE_INSTALL_LABEL: &str = "jogador:etapa.instalar";
/// Etapa "Exportando o pack para servir neste computador" (pack não publicado).
pub const STAGE_EXPORT: &str = "test.player.export";
/// Texto da etapa.
pub const STAGE_EXPORT_LABEL: &str = "jogador:etapa.exportar";

/// Arquivo de estado que o installer grava na pasta do jogo.
pub const RECEIVED_FILE: &str = "packwiz.json";

/// Linhas guardadas da saída do installer (um pack grande tem uma linha por arquivo).
const MAX_OUTPUT_LINES: usize = 5_000;

// -------------------------------------------------------------------------------------------
// O relatório

/// De onde o jogador recebeu o pack.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum PlayerSource {
    /// O link do pack publicado no GitHub.
    Published {
        /// O link (`…/main/pack.toml`).
        link: String,
        /// A última versão publicada.
        version: Option<String>,
    },
    /// O pack exportado agora, servido só neste computador (ainda não publicado).
    Local,
}

/// Uma diferença entre o que o jogador recebeu e o pack.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "camelCase")]
pub enum PlayerItemKind {
    /// Mod da CurseForge que o autor não deixa outros apps baixarem: não baixa sozinho para os
    /// jogadores (eles precisam baixar à mão).
    Blocked,
    /// Está no pack que o jogador recebeu, mas o download falhou.
    NotDownloaded,
    /// Está no pack, mas não no que o jogador recebeu (versão publicada mais antiga).
    NotPublished,
    /// O pack marca para o jogador, mas o que ele recebeu marca só para o servidor.
    ServerOnly,
    /// Opcional: o jogador escolhe na instalação.
    Optional,
    /// O jogador recebe, mas não está mais no pack.
    Extra,
}

/// Uma linha do resultado.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PlayerItem {
    /// O que aconteceu.
    pub kind: PlayerItemKind,
    /// Nome do item (o `name` do metafile ou o nome do arquivo).
    pub name: String,
    /// Caminho do arquivo na pasta do jogo (`mods/sodium.jar`).
    pub path: String,
    /// Opcionais: se vem marcado para o jogador.
    pub default_on: Option<bool>,
}

/// O resultado do "Testar como o jogador recebe", gravado na sessão (`player` no
/// `session.json`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PlayerReport {
    /// De onde veio.
    pub source: PlayerSource,
    /// `version` do `pack.toml` recebido.
    pub received_version: Option<String>,
    /// O GitHub entregou um `pack.toml` diferente do da última publicação (cache de até 5
    /// minutos do `raw.githubusercontent.com`).
    pub outdated: bool,
    /// Arquivos que chegaram à pasta do jogo.
    pub received_files: u32,
    /// O Crash Assistant chegou ao jogador (`None`: o pack não marca o Crash Assistant).
    pub crash_assistant: Option<bool>,
    /// As diferenças, na ordem de importância.
    pub items: Vec<PlayerItem>,
}

impl PlayerReport {
    /// Quantos itens de um tipo.
    #[must_use]
    pub fn count(&self, kind: PlayerItemKind) -> usize {
        self.items.iter().filter(|item| item.kind == kind).count()
    }
}

// -------------------------------------------------------------------------------------------
// O installer

/// A linha de comando do bootstrap.
#[derive(Debug, Clone)]
pub struct InstallerCommand {
    /// O `java` do teste (o de console, para ler a saída).
    pub java: PathBuf,
    /// Os jars do cache.
    pub jars: InstallerJars,
    /// O link do `pack.toml`.
    pub link: Url,
    /// A pasta do jogo da instância limpa.
    pub pack_folder: PathBuf,
    /// Pasta de trabalho do processo.
    pub work_dir: PathBuf,
    /// Argumentos extras da JVM (só testes: um proxy fechado garante que nada sai para a
    /// internet).
    pub extra_jvm_args: Vec<String>,
    /// `--bootstrap-update-url` (só testes: o servidor local conta se o bootstrap tentou
    /// atualizar). No app, `None`: com `--bootstrap-no-update` ele não consulta nada.
    pub update_url: Option<String>,
}

/// Como o installer terminou.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallerRun {
    /// Saiu com sucesso ("Finished successfully!").
    pub success: bool,
    /// As linhas da saída (até [`MAX_OUTPUT_LINES`], as últimas).
    pub lines: Vec<String>,
}

impl InstallerCommand {
    /// Os argumentos depois do `java`.
    #[must_use]
    pub fn args(&self) -> Vec<OsString> {
        let mut args: Vec<OsString> = vec![
            // Sem janela, e a saída em UTF-8 (nomes de mods com acentos).
            "-Djava.awt.headless=true".into(),
            "-Dfile.encoding=UTF-8".into(),
            "-Dstdout.encoding=UTF-8".into(),
            "-Dstderr.encoding=UTF-8".into(),
        ];
        args.extend(self.extra_jvm_args.iter().map(OsString::from));
        args.push("-jar".into());
        args.push(self.jars.bootstrap.clone().into());
        args.push("--bootstrap-no-update".into());
        if let Some(url) = &self.update_url {
            args.push("--bootstrap-update-url".into());
            args.push(url.into());
        }
        args.push("--bootstrap-main-jar".into());
        args.push(self.jars.installer.clone().into());
        for arg in ["-g", "-s", "client", "--pack-folder"] {
            args.push(arg.into());
        }
        args.push(self.pack_folder.clone().into());
        args.push(self.link.as_str().into());
        args
    }

    /// Roda o installer até ele sair, mandando cada linha a `on_line`. Cancelar mata o
    /// processo.
    ///
    /// Erros: [`Error::Cancelled`]; [`Error::PlayerInstallFailed`] se o Java não abrir.
    pub async fn run(
        &self,
        cancel: &CancellationToken,
        on_line: &(dyn Fn(&str) + Send + Sync),
    ) -> Result<InstallerRun> {
        let mut command = tokio::process::Command::new(&self.java);
        command
            .args(self.args())
            .current_dir(&self.work_dir)
            .env_remove("JAVA_TOOL_OPTIONS")
            .env_remove("_JAVA_OPTIONS")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        #[cfg(windows)]
        {
            // CREATE_NO_WINDOW: o `java.exe` de console não abre uma janela preta.
            command.creation_flags(0x0800_0000);
        }
        let mut child = command
            .spawn()
            .map_err(|error| Error::PlayerInstallFailed {
                reason: format!("o Java não abriu: {}", error.kind()),
            })?;
        let (Some(stdout), Some(stderr)) = (child.stdout.take(), child.stderr.take()) else {
            return Err(Error::Internal("saída do installer não capturada".into()));
        };
        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel::<String>();
        let out_sender = sender.clone();
        let readers = [
            tokio::spawn(read_lines(stdout, out_sender)),
            tokio::spawn(read_lines(stderr, sender)),
        ];
        let mut lines = Vec::new();
        let status = loop {
            tokio::select! {
                () = cancel.cancelled() => {
                    let _ = child.kill().await;
                    return Err(Error::Cancelled);
                }
                line = receiver.recv() => match line {
                    Some(line) => {
                        on_line(&line);
                        lines.push(line);
                        if lines.len() > MAX_OUTPUT_LINES {
                            lines.remove(0);
                        }
                    }
                    None => break child.wait().await.map_err(|error| {
                        Error::Internal(format!("esperar o installer: {error}"))
                    })?,
                },
            }
        };
        for reader in readers {
            let _ = reader.await;
        }
        let finished = lines
            .iter()
            .any(|line| line.contains("Finished successfully!"));
        Ok(InstallerRun {
            success: status.success() && finished,
            lines,
        })
    }
}

async fn read_lines(
    stream: impl tokio::io::AsyncRead + Unpin,
    sender: tokio::sync::mpsc::UnboundedSender<String>,
) {
    let mut reader = BufReader::new(stream);
    let mut buffer = Vec::new();
    loop {
        buffer.clear();
        match reader.read_until(b'\n', &mut buffer).await {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                let line = String::from_utf8_lossy(&buffer)
                    .trim_end_matches(['\r', '\n'])
                    .to_owned();
                if sender.send(line).is_err() {
                    break;
                }
            }
        }
    }
}

/// Um mod da CurseForge que o installer não conseguiu baixar porque o autor bloqueou apps de
/// terceiros.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockedNotice {
    /// Caminho do arquivo na pasta do jogo (`mods/entityculling.jar`).
    pub path: String,
    /// Página do arquivo no site da CurseForge.
    pub page_url: Option<String>,
}

/// Os mods bloqueados que o installer listou ("Please go to <página> and save this file to
/// <caminho>"). `pack_folder` é a pasta do jogo passada ao installer.
#[must_use]
pub fn blocked_notices(lines: &[String], pack_folder: &Path) -> Vec<BlockedNotice> {
    const GO: &str = "Please go to ";
    const SAVE: &str = " and save this file to ";
    let folder = slashes(&pack_folder.to_string_lossy());
    let mut found: Vec<BlockedNotice> = Vec::new();
    for line in lines {
        let Some(start) = line.find(GO) else {
            continue;
        };
        let rest = &line[start + GO.len()..];
        let Some(split) = rest.find(SAVE) else {
            continue;
        };
        let page = rest[..split].trim();
        let target = slashes(rest[split + SAVE.len()..].trim());
        let path = relative_to(&target, &folder);
        if found.iter().any(|notice| notice.path == path) {
            continue;
        }
        found.push(BlockedNotice {
            path,
            page_url: page.starts_with("https://").then(|| page.to_owned()),
        });
    }
    found
}

fn slashes(text: &str) -> String {
    text.replace('\\', "/")
}

/// `target` relativo a `folder` (com `/`); fora dela, só o nome do arquivo.
fn relative_to(target: &str, folder: &str) -> String {
    let folder = folder.trim_end_matches('/');
    let inside = if cfg!(windows) {
        target
            .get(..folder.len())
            .filter(|head| head.eq_ignore_ascii_case(folder))
            .map(|_| &target[folder.len()..])
    } else {
        target.strip_prefix(folder)
    };
    match inside.and_then(|rest| rest.strip_prefix('/')) {
        Some(rest) => clean_path(rest),
        None => target.rsplit('/').next().unwrap_or(target).to_owned(),
    }
}

/// Uma frase curta do motivo da falha, para a mensagem (a primeira exceção depois de
/// "[FATAL]" ou "Failed to download modpack"), sem caminhos da máquina.
#[must_use]
pub fn failure_reason(lines: &[String], pack_folder: &Path) -> String {
    let start = lines
        .iter()
        .rposition(|line| {
            line.contains("[FATAL]") || line.starts_with("Failed to download modpack")
        })
        .unwrap_or(0);
    let pick = lines[start..]
        .iter()
        .find(|line| {
            let line = line.trim_start();
            !line.starts_with("at ") && (line.contains("Exception") || line.contains("Error"))
        })
        .or_else(|| lines[start..].iter().find(|line| !line.trim().is_empty()))
        .map_or("", String::as_str);
    let folder = slashes(&pack_folder.to_string_lossy());
    let mut text = slashes(pick.trim());
    if !folder.is_empty() {
        text = text.replace(folder.trim_end_matches('/'), ".");
    }
    for home in ["USERPROFILE", "HOME"] {
        if let Some(home) = std::env::var_os(home).filter(|value| value.len() > 1) {
            text = text.replace(slashes(&home.to_string_lossy()).as_str(), "<usuário>");
        }
    }
    text.chars().take(300).collect()
}

// -------------------------------------------------------------------------------------------
// O que o jogador recebeu

/// Um arquivo no `packwiz.json`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct CachedFile {
    cached_location: Option<String>,
    is_optional: bool,
    option_value: Option<bool>,
    only_other_side: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct ManifestJson {
    pack_file_hash: Option<HashJson>,
    cached_files: BTreeMap<String, CachedFile>,
}

#[derive(Debug, Clone, Deserialize)]
struct HashJson {
    #[serde(rename = "type")]
    kind: String,
    value: String,
}

/// O que o installer deixou na pasta do jogo.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Received {
    /// SHA-256 do `pack.toml` recebido (o installer só grava quando a instalação terminou sem
    /// erros).
    pub pack_sha256: Option<String>,
    files: BTreeMap<String, CachedFile>,
}

impl Received {
    /// Lê o `packwiz.json` da pasta do jogo (ausente: nada recebido).
    ///
    /// Erros: os de disco.
    pub fn read(game_dir: &Path) -> Result<Self> {
        let path = game_dir.join(RECEIVED_FILE);
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::default());
            }
            Err(error) => return Err(Error::io("ler", RECEIVED_FILE, error)),
        };
        let manifest: ManifestJson = serde_json::from_slice(&bytes).unwrap_or_else(|error| {
            tracing::warn!(%error, "packwiz.json ilegível; tratado como nada recebido");
            ManifestJson::default()
        });
        Ok(Self {
            pack_sha256: manifest
                .pack_file_hash
                .filter(|hash| hash.kind.eq_ignore_ascii_case("sha256"))
                .map(|hash| hash.value.to_ascii_lowercase()),
            files: manifest.cached_files,
        })
    }
}

// -------------------------------------------------------------------------------------------
// O pack, como o Warden o vê

/// Um item do pack.
#[derive(Debug, Clone)]
struct PackItem {
    name: String,
    /// Caminho do arquivo na pasta do jogo.
    destination: String,
    side: Side,
    option: Option<bool>,
    metafile: Option<Metafile>,
}

/// O pack: caminho no índice → item.
fn pack_items(pack_root: &Path) -> Result<BTreeMap<String, PackItem>> {
    let read = warden_packwiz::read_pack(pack_root).map_err(Error::Pack)?;
    let index = read.index.map_err(Error::Pack)?;
    let metafiles: BTreeMap<String, Metafile> = read
        .metafiles
        .into_iter()
        .filter_map(|metafile| Some((metafile.path, metafile.result.ok()?.value)))
        .collect();
    let mut items = BTreeMap::new();
    for entry in &index.value.files {
        let path = clean_path(&entry.file.replace('\\', "/"));
        let item = match metafiles.get(path.as_str()) {
            Some(metafile) if entry.metafile => PackItem {
                name: metafile.name.clone(),
                destination: destination_of(&path, &metafile.filename),
                side: metafile.side.clone(),
                option: metafile
                    .option
                    .as_ref()
                    .filter(|option| option.optional)
                    .map(|option| option.default),
                metafile: Some(metafile.clone()),
            },
            _ => PackItem {
                name: path.rsplit('/').next().unwrap_or(&path).to_owned(),
                destination: path.clone(),
                side: Side::Both,
                option: None,
                metafile: None,
            },
        };
        items.insert(path, item);
    }
    Ok(items)
}

/// Destino do arquivo de um metafile: a pasta do metafile mais o `filename`.
fn destination_of(metafile_path: &str, filename: &str) -> String {
    match metafile_path.rsplit_once('/') {
        Some((dir, _)) => clean_path(&format!("{dir}/{filename}")),
        None => clean_path(filename),
    }
}

fn for_client(side: &Side) -> bool {
    matches!(side, Side::Unset | Side::Both | Side::Client)
}

// -------------------------------------------------------------------------------------------
// Bloqueados: o que o jogador faria à mão

/// O que deu para pôr na instância no lugar do jogador.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Supplied {
    /// Copiados do cache de downloads (caminhos na pasta do jogo).
    pub copied: Vec<String>,
    /// Ainda faltam no cache: os downloads manuais da T20 resolvem.
    pub waiting: Vec<BlockedFile>,
    /// Sem item no pack que corresponda (a versão recebida é outra): só viram resultado.
    pub unknown: Vec<String>,
}

/// Para cada mod bloqueado, copia para a pasta do jogo o arquivo que o Warden já tem no cache
/// de downloads (conferido pelo hash do metafile do pack), como o jogador faria depois de
/// baixar pelo site.
///
/// Erros: os de disco e do cache.
pub fn supply_blocked(
    notices: &[BlockedNotice],
    pack_root: &Path,
    cache: &DownloadCache,
    game_dir: &Path,
) -> Result<Supplied> {
    let items = pack_items(pack_root)?;
    let mut supplied = Supplied::default();
    for notice in notices {
        let found = items.iter().find_map(|(path, item)| {
            let metafile = item.metafile.as_ref()?;
            (item.destination == notice.path).then_some((path, item, metafile))
        });
        let Some((path, item, metafile)) = found else {
            supplied.unknown.push(notice.path.clone());
            continue;
        };
        let Ok(format) = HashFormat::parse(&metafile.download.hash_format) else {
            supplied.unknown.push(notice.path.clone());
            continue;
        };
        let expected = ExpectedHash::new(format, metafile.download.hash.clone());
        if let Some(cached) = cache.find(&expected)? {
            let target = warden_core::resolve_inside(game_dir, &notice.path)?;
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| Error::io("criar a pasta", notice.path.as_str(), e))?;
            }
            fs::copy(&cached.path, &target)
                .map_err(|e| Error::io("copiar", notice.path.as_str(), e))?;
            supplied.copied.push(notice.path.clone());
            continue;
        }
        let (project, file) = metafile.curseforge_ids().unwrap_or_default();
        supplied.waiting.push(BlockedFile {
            metafile: path.clone(),
            name: item.name.clone(),
            file_name: metafile.filename.clone(),
            project_id: u64::from(project),
            file_id: u64::from(file),
            page_url: notice.page_url.clone(),
            hash_format: metafile.download.hash_format.clone(),
            hash: metafile.download.hash.clone(),
        });
    }
    Ok(supplied)
}

// -------------------------------------------------------------------------------------------
// A comparação

/// O que entra na comparação.
#[derive(Debug, Clone)]
pub struct CompareInput<'a> {
    /// A pasta do pack no Warden.
    pub pack_root: &'a Path,
    /// A pasta do jogo da instância limpa.
    pub game_dir: &'a Path,
    /// O que o installer gravou.
    pub received: &'a Received,
    /// Os bloqueados que ele listou (em qualquer rodada).
    pub blocked: &'a [BlockedNotice],
    /// De onde veio.
    pub source: PlayerSource,
    /// `version` do `pack.toml` recebido.
    pub received_version: Option<String>,
    /// SHA-256 do `pack.toml` da última publicação (`None`: pack servido daqui).
    pub published_sha256: Option<String>,
}

/// O que o jogador recebeu × o pack.
///
/// Erros: o pack ilegível.
pub fn compare(input: &CompareInput<'_>) -> Result<PlayerReport> {
    let pack = pack_items(input.pack_root)?;
    let received = &input.received.files;
    let present = |path: &str| {
        warden_core::resolve_inside(input.game_dir, path).is_ok_and(|file| file.is_file())
    };
    let blocked: BTreeSet<&str> = input
        .blocked
        .iter()
        .map(|notice| notice.path.as_str())
        .collect();
    let mut items = Vec::new();
    let mut push = |kind, name: &str, path: &str, default_on| {
        items.push(PlayerItem {
            kind,
            name: name.to_owned(),
            path: path.to_owned(),
            default_on,
        });
    };

    for (index_path, item) in &pack {
        if !for_client(&item.side) {
            continue;
        }
        let destination = item.destination.as_str();
        if blocked.contains(destination) {
            push(PlayerItemKind::Blocked, &item.name, destination, None);
            continue;
        }
        match received.get(index_path) {
            None if received.is_empty() && !present(destination) => {
                // Nada foi instalado (o installer desistiu): a falha vem na mensagem do teste.
                push(PlayerItemKind::NotDownloaded, &item.name, destination, None);
            }
            None => push(PlayerItemKind::NotPublished, &item.name, destination, None),
            Some(file) if file.only_other_side => {
                push(PlayerItemKind::ServerOnly, &item.name, destination, None);
            }
            Some(file) => {
                let location = file.cached_location.as_deref().unwrap_or(destination);
                if !present(location) {
                    push(PlayerItemKind::NotDownloaded, &item.name, location, None);
                } else if file.is_optional {
                    push(
                        PlayerItemKind::Optional,
                        &item.name,
                        location,
                        Some(item.option.unwrap_or(true)),
                    );
                }
            }
        }
    }
    for (index_path, file) in received {
        if pack.contains_key(index_path) || file.only_other_side {
            continue;
        }
        if let Some(location) = file.cached_location.as_deref().filter(|path| present(path)) {
            let name = location.rsplit('/').next().unwrap_or(location);
            push(PlayerItemKind::Extra, name, location, None);
        }
    }
    items.sort_by(|a, b| a.kind.cmp(&b.kind).then_with(|| a.path.cmp(&b.path)));

    let received_files = received
        .values()
        .filter_map(|file| file.cached_location.as_deref())
        .filter(|path| present(path))
        .count();
    let crash_assistant = {
        let marked = excluded_paths(input.pack_root, &PlayerToolsMode::Normal);
        let destinations: Vec<&str> = marked
            .iter()
            .filter_map(|path| pack.get(path))
            .map(|item| item.destination.as_str())
            .collect();
        (!destinations.is_empty()).then(|| destinations.iter().all(|path| present(path)))
    };
    let outdated = match (&input.published_sha256, &input.received.pack_sha256) {
        (Some(published), Some(received)) => !published.eq_ignore_ascii_case(received),
        _ => false,
    };
    Ok(PlayerReport {
        source: input.source.clone(),
        received_version: input.received_version.clone(),
        outdated,
        received_files: u32::try_from(received_files).unwrap_or(u32::MAX),
        crash_assistant,
        items,
    })
}

// -------------------------------------------------------------------------------------------
// O servidor local

/// Serve uma pasta (o pack exportado) só em `127.0.0.1`, numa porta livre, enquanto viver.
/// Só `GET`, só arquivos dentro da pasta.
#[derive(Debug)]
pub struct LocalPackServer {
    url: Url,
    requests: Arc<AtomicUsize>,
    stop: CancellationToken,
}

impl LocalPackServer {
    /// Começa a servir `root`.
    ///
    /// Erros: a porta não abriu.
    pub async fn start(root: PathBuf) -> Result<Self> {
        let listener = TcpListener::bind(("127.0.0.1", 0))
            .await
            .map_err(|e| Error::io("abrir o servidor local", "127.0.0.1", e))?;
        let port = listener
            .local_addr()
            .map_err(|e| Error::io("abrir o servidor local", "127.0.0.1", e))?
            .port();
        let url = Url::parse(&format!("http://127.0.0.1:{port}/"))
            .map_err(|error| Error::Internal(error.to_string()))?;
        let requests = Arc::new(AtomicUsize::new(0));
        let stop = CancellationToken::new();
        let root = Arc::new(root);
        tokio::spawn({
            let (stop, requests) = (stop.clone(), Arc::clone(&requests));
            async move {
                loop {
                    tokio::select! {
                        () = stop.cancelled() => break,
                        accepted = listener.accept() => {
                            let Ok((stream, _)) = accepted else { continue };
                            requests.fetch_add(1, Ordering::Relaxed);
                            tokio::spawn(serve_one(stream, Arc::clone(&root)));
                        }
                    }
                }
            }
        });
        Ok(Self {
            url,
            requests,
            stop,
        })
    }

    /// O link de um arquivo servido (`pack.toml`).
    ///
    /// Erros: nome que não cabe numa URL.
    pub fn link(&self, path: &str) -> Result<Url> {
        self.url
            .join(path)
            .map_err(|error| Error::Internal(error.to_string()))
    }

    /// Quantas conexões chegaram (testes).
    #[must_use]
    pub fn requests(&self) -> usize {
        self.requests.load(Ordering::Relaxed)
    }
}

impl Drop for LocalPackServer {
    fn drop(&mut self) {
        self.stop.cancel();
    }
}

async fn serve_one(mut stream: TcpStream, root: Arc<PathBuf>) {
    let mut head = Vec::new();
    let mut buffer = [0_u8; 1024];
    while !head.windows(4).any(|window| window == b"\r\n\r\n") {
        match stream.read(&mut buffer).await {
            Ok(0) | Err(_) => return,
            Ok(read) => head.extend_from_slice(&buffer[..read]),
        }
        if head.len() > 16 * 1024 {
            return;
        }
    }
    let text = String::from_utf8_lossy(&head);
    let mut parts = text.lines().next().unwrap_or_default().split_whitespace();
    let (method, target) = (
        parts.next().unwrap_or_default(),
        parts.next().unwrap_or("/"),
    );
    let response = if method == "GET" {
        match file_for(&root, target) {
            Some(bytes) => (200, bytes),
            None => (404, Vec::new()),
        }
    } else {
        (405, Vec::new())
    };
    let reason = match response.0 {
        200 => "OK",
        404 => "Not Found",
        _ => "Method Not Allowed",
    };
    let header = format!(
        "HTTP/1.1 {} {reason}\r\nContent-Length: {}\r\nContent-Type: application/octet-stream\r\nConnection: close\r\n\r\n",
        response.0,
        response.1.len()
    );
    let _ = stream.write_all(header.as_bytes()).await;
    let _ = stream.write_all(&response.1).await;
    let _ = stream.shutdown().await;
}

/// O arquivo pedido, só se estiver dentro de `root`.
fn file_for(root: &Path, target: &str) -> Option<Vec<u8>> {
    let path = target.split(['?', '#']).next()?.trim_start_matches('/');
    let decoded = percent_decode(path)?;
    let file = warden_core::resolve_inside(root, &decoded).ok()?;
    fs::read(file).ok()
}

fn percent_decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let hex = text.get(index + 1..index + 3)?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            index += 3;
        } else {
            out.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(out).ok()
}

#[cfg(test)]
mod tests {
    use std::fmt::Write as _;

    use sha2::{Digest as _, Sha256};
    use warden_http::{HttpClient, HttpConfig};

    use super::*;
    use crate::test_support::temp_dir;

    fn sha256(bytes: &[u8]) -> String {
        hex::encode(Sha256::digest(bytes))
    }

    fn write(root: &Path, path: &str, text: &str) {
        let file = root.join(path);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(file, text).unwrap();
    }

    /// Um pack com um mod normal, um só do servidor, um opcional desligado, um da CurseForge e
    /// uma config; `[player-tools]` marca o `crash.pw.toml`.
    fn pack(root: &Path) {
        let metafile = |name: &str, file: &str, side: &str, extra: &str| {
            format!(
                "name = \"{name}\"\nfilename = \"{file}\"\nside = \"{side}\"\n\n[download]\nurl = \"https://cdn.modrinth.com/{file}\"\nhash-format = \"sha256\"\nhash = \"{}\"\n{extra}",
                sha256(file.as_bytes())
            )
        };
        let files = [
            ("mods/sodium.pw.toml", metafile("Sodium", "sodium.jar", "client", "")),
            ("mods/lithium.pw.toml", metafile("Lithium", "lithium.jar", "both", "")),
            ("mods/servidor.pw.toml", metafile("Só Servidor", "servidor.jar", "server", "")),
            (
                "mods/zoom.pw.toml",
                metafile("Zoom", "zoom.jar", "client", "\n[option]\noptional = true\ndefault = false\n"),
            ),
            ("mods/crash.pw.toml", metafile("Crash Assistant", "crash.jar", "client", "")),
            (
                "mods/culling.pw.toml",
                "name = \"Entity Culling\"\nfilename = \"culling.jar\"\nside = \"client\"\n\n[download]\nhash-format = \"sha1\"\nhash = \"0beec7b5ea3f0fdbc95d0dd47f3c5bc275da8a33\"\nmode = \"metadata:curseforge\"\n\n[update]\n[update.curseforge]\nfile-id = 4712345\nproject-id = 448233\n".to_owned(),
            ),
            ("config/a.toml", "a = 1\n".to_owned()),
        ];
        let mut index = "hash-format = \"sha256\"\n".to_owned();
        for (path, text) in &files {
            write(root, path, text);
            let _ = write!(
                index,
                "\n[[files]]\nfile = \"{path}\"\nhash = \"{}\"\n{}",
                sha256(text.as_bytes()),
                if path.ends_with(".pw.toml") {
                    "metafile = true\n"
                } else {
                    ""
                }
            );
        }
        write(root, "index.toml", &index);
        write(
            root,
            "pack.toml",
            &format!(
                "name = \"Vale\"\nversion = \"1.0.0\"\npack-format = \"packwiz:1.1.0\"\n\n[index]\nfile = \"index.toml\"\nhash-format = \"sha256\"\nhash = \"{}\"\n\n[versions]\nminecraft = \"1.20.1\"\n",
                sha256(index.as_bytes())
            ),
        );
        write(
            root,
            ".warden/project.toml",
            "[player-tools]\ncrash-assistant = \"mods/crash.pw.toml\"\n",
        );
    }

    fn received(json: &str, game: &Path, files: &[&str]) -> Received {
        write(game, RECEIVED_FILE, json);
        for file in files {
            write(game, file, "x");
        }
        Received::read(game).unwrap()
    }

    fn input<'a>(
        pack: &'a Path,
        game: &'a Path,
        received: &'a Received,
        blocked: &'a [BlockedNotice],
    ) -> CompareInput<'a> {
        CompareInput {
            pack_root: pack,
            game_dir: game,
            received,
            blocked,
            source: PlayerSource::Local,
            received_version: Some("1.0.0".into()),
            published_sha256: None,
        }
    }

    fn kinds(report: &PlayerReport) -> Vec<(PlayerItemKind, &str)> {
        report
            .items
            .iter()
            .map(|item| (item.kind, item.path.as_str()))
            .collect()
    }

    const RECEIVED: &str = r#"{"packFileHash":{"type":"sha256","value":"ABC"},"cachedFiles":{
        "mods/sodium.pw.toml":{"cachedLocation":"mods/sodium.jar","optionValue":true},
        "mods/lithium.pw.toml":{"cachedLocation":"mods/lithium.jar","optionValue":true},
        "mods/servidor.pw.toml":{"optionValue":true,"onlyOtherSide":true},
        "mods/zoom.pw.toml":{"cachedLocation":"mods/zoom.jar","isOptional":true,"optionValue":true},
        "mods/crash.pw.toml":{"cachedLocation":"mods/crash.jar","optionValue":true},
        "mods/culling.pw.toml":{"cachedLocation":"mods/culling.jar","optionValue":true},
        "config/a.toml":{"cachedLocation":"config/a.toml","optionValue":true}}}"#;

    #[test]
    fn tudo_recebido_mostra_so_o_opcional_e_o_crash_assistant() {
        let dir = temp_dir();
        let (root, game) = (dir.path().join("pack"), dir.path().join("jogo"));
        pack(&root);
        let all = [
            "mods/sodium.jar",
            "mods/lithium.jar",
            "mods/zoom.jar",
            "mods/crash.jar",
            "mods/culling.jar",
            "config/a.toml",
        ];
        let got = received(RECEIVED, &game, &all);
        assert_eq!(got.pack_sha256.as_deref(), Some("abc"));
        let report = compare(&input(&root, &game, &got, &[])).unwrap();
        assert_eq!(
            kinds(&report),
            [(PlayerItemKind::Optional, "mods/zoom.jar")]
        );
        assert_eq!(report.items[0].default_on, Some(false));
        assert_eq!(report.items[0].name, "Zoom");
        assert_eq!(report.received_files, 6);
        assert_eq!(report.crash_assistant, Some(true));
        assert!(!report.outdated);
    }

    #[test]
    fn faltando_bloqueado_lado_errado_e_a_mais() {
        let dir = temp_dir();
        let (root, game) = (dir.path().join("pack"), dir.path().join("jogo"));
        pack(&root);
        // A versão recebida: Lithium marcado só para o servidor, sem a config, com um mod que
        // saiu do pack; o Sodium não baixou e o Entity Culling foi bloqueado.
        let json = r#"{"packFileHash":{"type":"sha256","value":"abc"},"cachedFiles":{
            "mods/sodium.pw.toml":{"cachedLocation":"mods/sodium.jar"},
            "mods/lithium.pw.toml":{"onlyOtherSide":true},
            "mods/crash.pw.toml":{"cachedLocation":"mods/crash.jar"},
            "mods/zoom.pw.toml":{"cachedLocation":"mods/zoom.jar","isOptional":true},
            "mods/velho.pw.toml":{"cachedLocation":"mods/velho.jar"}}}"#;
        let got = received(
            json,
            &game,
            &["mods/crash.jar", "mods/zoom.jar", "mods/velho.jar"],
        );
        let blocked = [BlockedNotice {
            path: "mods/culling.jar".into(),
            page_url: None,
        }];
        let mut compared = input(&root, &game, &got, &blocked);
        compared.published_sha256 = Some("def".into());
        let report = compare(&compared).unwrap();
        assert_eq!(
            kinds(&report),
            [
                (PlayerItemKind::Blocked, "mods/culling.jar"),
                (PlayerItemKind::NotDownloaded, "mods/sodium.jar"),
                (PlayerItemKind::NotPublished, "config/a.toml"),
                (PlayerItemKind::ServerOnly, "mods/lithium.jar"),
                (PlayerItemKind::Optional, "mods/zoom.jar"),
                (PlayerItemKind::Extra, "mods/velho.jar"),
            ]
        );
        assert_eq!(report.count(PlayerItemKind::Blocked), 1);
        assert!(report.outdated, "hash diferente do publicado");
    }

    #[test]
    fn crash_assistant_ausente_e_pack_sem_marcacao() {
        let dir = temp_dir();
        let (root, game) = (dir.path().join("pack"), dir.path().join("jogo"));
        pack(&root);
        let got = received(RECEIVED, &game, &["mods/sodium.jar"]);
        let report = compare(&input(&root, &game, &got, &[])).unwrap();
        assert_eq!(report.crash_assistant, Some(false));
        fs::remove_file(root.join(".warden/project.toml")).unwrap();
        let report = compare(&input(&root, &game, &got, &[])).unwrap();
        assert_eq!(report.crash_assistant, None);
    }

    #[test]
    fn nada_instalado_vira_nao_baixado() {
        let dir = temp_dir();
        let (root, game) = (dir.path().join("pack"), dir.path().join("jogo"));
        pack(&root);
        fs::create_dir_all(&game).unwrap();
        let got = Received::read(&game).unwrap();
        assert_eq!(got, Received::default());
        let report = compare(&input(&root, &game, &got, &[])).unwrap();
        assert_eq!(report.count(PlayerItemKind::NotDownloaded), 6);
        assert_eq!(report.received_files, 0);
    }

    /// A saída real do packwiz-installer 0.5.14 sem janela com um mod bloqueado
    /// (`CurseForgeSourcer.kt` e `CLIHandler.showExceptions`).
    #[test]
    fn bloqueados_lidos_da_saida_do_installer() {
        let folder = if cfg!(windows) {
            PathBuf::from(r"C:\Dados\cache\tmp\player-test-1\minecraft")
        } else {
            PathBuf::from("/dados/cache/tmp/player-test-1/minecraft")
        };
        let shown = folder.to_string_lossy().into_owned();
        let sep = if cfg!(windows) { "\\" } else { "/" };
        let lines: Vec<String> = [
            "Validating existing files...".to_owned(),
            "Resolving CurseForge metadata...".to_owned(),
            "Failed to download modpack, the following errors were encountered:".to_owned(),
            "Entity Culling: java.lang.Exception: This mod is excluded from the CurseForge API and must be downloaded manually.".to_owned(),
            format!("Please go to https://www.curseforge.com/minecraft/mc-mods/entityculling/files/4712345 and save this file to {shown}{sep}mods{sep}culling.jar"),
            "\tat link.infra.packwiz.installer.metadata.curseforge.CurseForgeSourcerKt.resolveCfMetadata(SourceFile:151)".to_owned(),
            format!("Please go to https://www.curseforge.com/x/files/1 and save this file to {shown}{sep}mods{sep}culling.jar"),
            "Please go to nada and save this file to /fora/outro.jar".to_owned(),
        ]
        .into();
        let notices = blocked_notices(&lines, &folder);
        assert_eq!(
            notices,
            [
                BlockedNotice {
                    path: "mods/culling.jar".into(),
                    page_url: Some(
                        "https://www.curseforge.com/minecraft/mc-mods/entityculling/files/4712345"
                            .into()
                    ),
                },
                BlockedNotice {
                    path: "outro.jar".into(),
                    page_url: None,
                },
            ]
        );
    }

    #[test]
    fn motivo_da_falha_sem_caminho_da_maquina() {
        let folder = PathBuf::from("/dados/tmp/player-test-1/minecraft");
        let lines: Vec<String> = [
            "Loading pack file...",
            "[FATAL] Failed to process index file: ",
            "java.io.FileNotFoundException: /dados/tmp/player-test-1/minecraft/mods/x.jar (sem permissão)",
            "\tat java.base/java.io.FileOutputStream.open0(Native Method)",
        ]
        .iter()
        .map(|line| (*line).to_owned())
        .collect();
        let reason = failure_reason(&lines, &folder);
        assert_eq!(
            reason,
            "java.io.FileNotFoundException: ./mods/x.jar (sem permissão)"
        );
        assert!(!reason.contains("/dados"));
    }

    #[test]
    fn linha_de_comando_do_bootstrap() {
        let command = InstallerCommand {
            java: "java".into(),
            jars: InstallerJars {
                bootstrap: "boot.jar".into(),
                installer: "inst.jar".into(),
            },
            link: Url::parse("https://raw.githubusercontent.com/d/p/main/pack.toml").unwrap(),
            pack_folder: "jogo".into(),
            work_dir: "work".into(),
            extra_jvm_args: vec!["-Dx=1".into()],
            update_url: None,
        };
        let args: Vec<String> = command
            .args()
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            args,
            [
                "-Djava.awt.headless=true",
                "-Dfile.encoding=UTF-8",
                "-Dstdout.encoding=UTF-8",
                "-Dstderr.encoding=UTF-8",
                "-Dx=1",
                "-jar",
                "boot.jar",
                "--bootstrap-no-update",
                "--bootstrap-main-jar",
                "inst.jar",
                "-g",
                "-s",
                "client",
                "--pack-folder",
                "jogo",
                "https://raw.githubusercontent.com/d/p/main/pack.toml",
            ]
        );
    }

    #[test]
    fn bloqueado_no_cache_vai_para_a_instancia_e_o_resto_espera() {
        let dir = temp_dir();
        let (root, game) = (dir.path().join("pack"), dir.path().join("jogo"));
        pack(&root);
        let cache = DownloadCache::open(&dir.path().join("cache")).unwrap();
        let notices = [
            BlockedNotice {
                path: "mods/culling.jar".into(),
                page_url: Some("https://www.curseforge.com/p/files/1".into()),
            },
            BlockedNotice {
                path: "mods/desconhecido.jar".into(),
                page_url: None,
            },
        ];
        let first = supply_blocked(&notices, &root, &cache, &game).unwrap();
        assert!(first.copied.is_empty());
        assert_eq!(first.unknown, ["mods/desconhecido.jar"]);
        assert_eq!(first.waiting.len(), 1);
        let waiting = &first.waiting[0];
        assert_eq!(
            (
                waiting.project_id,
                waiting.file_id,
                waiting.metafile.as_str()
            ),
            (448_233, 4_712_345, "mods/culling.pw.toml")
        );
        assert_eq!(
            waiting.page_url.as_deref(),
            Some("https://www.curseforge.com/p/files/1")
        );
        // O arquivo certo (sha1 de "foo") chega ao cache, como pelo download manual.
        let downloaded = dir.path().join("baixado.jar");
        fs::write(&downloaded, "foo").unwrap();
        crate::accept_manual_file(&cache, waiting, &downloaded).unwrap();
        let second = supply_blocked(&notices[..1], &root, &cache, &game).unwrap();
        assert_eq!(second.copied, ["mods/culling.jar"]);
        assert!(second.waiting.is_empty());
        assert_eq!(
            fs::read_to_string(game.join("mods/culling.jar")).unwrap(),
            "foo"
        );
    }

    #[tokio::test]
    async fn servidor_local_so_serve_a_pasta() {
        let dir = temp_dir();
        write(dir.path(), "servido/pack.toml", "name = \"Vale\"\n");
        write(dir.path(), "servido/mods/a b.pw.toml", "x");
        write(dir.path(), "segredo.txt", "nao");
        let server = LocalPackServer::start(dir.path().join("servido"))
            .await
            .unwrap();
        let link = server.link("pack.toml").unwrap();
        assert_eq!(link.host_str(), Some("127.0.0.1"));
        let http = HttpClient::new(HttpConfig::default()).unwrap();
        let body = http.get(link).bytes().await.unwrap();
        assert_eq!(body, b"name = \"Vale\"\n");
        let spaced = http
            .get(server.link("mods/a%20b.pw.toml").unwrap())
            .bytes()
            .await
            .unwrap();
        assert_eq!(spaced, b"x");
        for outside in ["../segredo.txt", "%2E%2E/segredo.txt", "nada.toml"] {
            let response = http
                .get(Url::parse(&format!("{}{outside}", server.link("").unwrap())).unwrap())
                .send()
                .await;
            assert!(
                response.is_err() || response.is_ok_and(|r| r.status().as_u16() == 404),
                "{outside}"
            );
        }
        assert!(server.requests() >= 3);
    }

    #[test]
    fn caminho_decodificado() {
        assert_eq!(
            percent_decode("a%20b/%C3%A7.toml").as_deref(),
            Some("a b/ç.toml")
        );
        assert_eq!(percent_decode("%zz"), None);
        assert_eq!(percent_decode("%2"), None);
    }
}
