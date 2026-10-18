//! Pacote para servidor (E-03; ADR-0035; ARCHITECTURE §12.1; SPEC T19): um `.zip` para quem
//! vai rodar o pack num servidor, a partir da mesma cópia de staging da exportação nativa.
//!
//! - [`analyze_server_pack`]: o que vai em cada variante, o que fica de fora (os itens "Só
//!   cliente", `options.txt`, resource packs, shaders, `kubejs/client_scripts/`) e, com mods
//!   dentro, os mods da CurseForge que vão pedir o download manual (T20). Só lê.
//! - [`export_server_pack`]: gera o zip, confere e só então grava no destino.
//!
//! As duas variantes levam `start.bat`, `start.sh`, `user_jvm_args.txt` e as configs do
//! servidor; nenhuma leva `eula.txt` (os scripts perguntam a quem roda o servidor):
//! - **pelo link** ([`ServerPackVariant::Link`]): o packwiz-installer-bootstrap e o
//!   packwiz-installer fixados (os mesmos jars, conferidos pelo SHA-256, do "Testar como o
//!   jogador recebe"), que os scripts rodam com `-s server` antes de abrir; o
//!   `server-overrides/` vai como pasta e os scripts o copiam por cima depois de cada
//!   atualização;
//! - **com mods dentro** ([`ServerPackVariant::Embedded`]): o pack é materializado só com o
//!   lado servidor (`warden-instance`, com o cache de downloads e a conferência pelo hash do
//!   índice); mods da CurseForge bloqueados para terceiros passam pelo download manual (T20,
//!   [`ManualDownloads`]); o `server-overrides/` vai já aplicado; e nada de terceiros entra sem
//!   a confirmação de licença.

mod content;
mod model;
mod scripts;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::future::Future;
use std::io::{Read as _, Write as _};
use std::path::Path;
use std::pin::Pin;

use sha2::{Digest as _, Sha256};
use url::Url;
use warden_core::{CancellationToken, Progress, ProgressSink};
use warden_curseforge::{Distribution, FileRef};
use warden_instance::{
    BOOTSTRAP, BlockedFile, INSTALLER, InstallSide, InstallerJars, InstanceDirs, Manifest,
    MaterializeOptions, OnModified, OptionalChoices, OptionalSelection, Outcome, PinnedJar,
    PlayerToolsMode, Sources, game_requirement,
};
use warden_packwiz::hash::{hash_bytes, hash_matches};
use warden_packwiz::{HashFormat, PACK_FILE, PackManifest};
use warden_packwiz_cli::Packwiz;
use zip::write::SimpleFileOptions;

pub use model::{
    JavaRange, LoaderDownload, ServerPackAnalysis, ServerPackContents, ServerPackFolder,
    ServerPackOptions, ServerPackResult, ServerPackVariant,
};

use content::{FilePlace, OVERRIDES_DIR, RefItem};
use scripts::{BOOTSTRAP_JAR, INSTALLER_JAR, JVM_ARGS, LoaderKind, START_BAT, START_SH};

use crate::native::{ExportSource, SourceReader, check_destination, indexed_files, prepare_stage};
use crate::{Error, ExportErrorCode, Result};

/// Etapa "Baixando e conferindo os mods do servidor" (com mods dentro).
pub const STAGE_SERVER_MODS: &str = "serverPackMods";
/// Rótulo da etapa.
pub const STAGE_SERVER_MODS_LABEL: &str = "pacoteServidor:etapa.mods";
/// Etapa "Montando o pacote".
pub const STAGE_SERVER_ZIP: &str = "serverPackZip";
/// Rótulo da etapa.
pub const STAGE_SERVER_ZIP_LABEL: &str = "pacoteServidor:etapa.montar";

/// Os avisos de licença do bootstrap e do installer (MIT), que vão junto dos jars.
const NOTICES_FILE: &str = "LICENCAS-DE-TERCEIROS.txt";

/// Memória aceita em `user_jvm_args.txt`, em MB.
const MEMORY_RANGE: std::ops::RangeInclusive<u32> = 512..=65_536;

/// Nomes mostrados dos itens "Só cliente".
const MAX_CLIENT_NAMES: usize = 50;

/// Futuro devolvido por [`ManualDownloads::wait`].
pub type WaitFuture<'a> = Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>>;

/// Quem resolve os downloads manuais da T20 durante a geração com mods dentro: mostra a lista à
/// pessoa e só termina quando **todos** os arquivos estiverem no cache de downloads (ou com
/// erro, por exemplo o cancelamento).
pub trait ManualDownloads: Send + Sync {
    /// Espera os arquivos de `blocked`.
    fn wait(&self, blocked: Vec<BlockedFile>) -> WaitFuture<'_>;
}

/// Tudo o que [`export_server_pack`] precisa.
pub struct ServerPackRequest<'a> {
    /// Pasta do pack.
    pub root: &'a Path,
    /// Estado atual ou versão salva (pelo link, a última versão publicada).
    pub source: &'a ExportSource,
    /// O arquivo `.zip` a criar (não pode existir).
    pub destination: &'a Path,
    /// Pasta das cópias temporárias (`cache/staging`).
    pub staging_parent: &'a Path,
    /// O sidecar.
    pub packwiz: &'a Packwiz,
    /// As escolhas da pessoa.
    pub options: &'a ServerPackOptions,
    /// Como os scripts instalam o loader.
    pub loader: &'a LoaderDownload,
    /// O Java aceito pelos scripts.
    pub java: JavaRange,
    /// O link do `pack.toml` publicado (obrigatório pelo link).
    pub link: Option<&'a Url>,
    /// O bootstrap e o installer do cache (obrigatórios pelo link).
    pub installer: Option<&'a InstallerJars>,
    /// Downloads, CurseForge e cache (obrigatórios com mods dentro).
    pub sources: Option<&'a Sources>,
    /// O download manual da T20 (sem ele, um mod bloqueado impede gerar).
    pub manual: Option<&'a dyn ManualDownloads>,
}

fn cancelled() -> Error {
    Error::internal("exportação cancelada")
}

fn check_cancel(cancel: &CancellationToken) -> Result<()> {
    if cancel.is_cancelled() {
        Err(cancelled())
    } else {
        Ok(())
    }
}

fn err(error: impl std::fmt::Display) -> Error {
    Error::internal(error.to_string())
}

/// O pack lido para o pacote.
struct PackPlan {
    manifest: PackManifest,
    minecraft: String,
    loader: Option<(String, String)>,
    refs: Vec<RefItem>,
    /// Arquivos do índice que não são referência.
    files: BTreeMap<String, Vec<u8>>,
}

impl PackPlan {
    fn read(files: &BTreeMap<String, Vec<u8>>) -> Result<Self> {
        let text = std::str::from_utf8(
            files
                .get(PACK_FILE)
                .ok_or_else(|| Error::internal("pack.toml ausente"))?,
        )
        .map_err(err)?;
        let manifest = PackManifest::parse(text).map_err(err)?.value;
        if manifest
            .options
            .as_ref()
            .is_some_and(|o| o.contains_key("meta-folder") || o.contains_key("meta-folder-base"))
        {
            return Err(Error::new(
                ExportErrorCode::ServerPackUnsupportedPack,
                "o pack usa meta-folder, que o pacote para servidor não reproduz",
            ));
        }
        let game = game_requirement(&manifest)
            .map_err(|e| Error::new(ExportErrorCode::ServerPackUnsupportedPack, e.to_string()))?;
        Ok(Self {
            minecraft: game.minecraft.clone(),
            loader: game.loader.map(|l| (l.loader, l.version)),
            manifest,
            refs: content::refs(files)?,
            files: files
                .iter()
                .filter(|(path, _)| {
                    !path.ends_with(".pw.toml")
                        && !matches!(path.as_str(), PACK_FILE | "index.toml")
                })
                .map(|(path, bytes)| (path.clone(), bytes.clone()))
                .collect(),
        })
    }

    fn loader_kind(&self) -> Option<LoaderKind> {
        LoaderKind::from_pack(self.loader.as_ref().map(|(name, _)| name.as_str()))
    }

    fn loader_text(&self) -> String {
        self.loader
            .as_ref()
            .map(|(name, version)| {
                let label = match name.as_str() {
                    "fabric" => "Fabric",
                    "forge" => "Forge",
                    "neoforge" => "NeoForge",
                    "quilt" => "Quilt",
                    other => other,
                };
                format!("{label} {version}")
            })
            .unwrap_or_default()
    }

    fn client_only(&self) -> Vec<&RefItem> {
        self.refs.iter().filter(|r| !r.on_server()).collect()
    }

    /// O conteúdo pelo link (sem os scripts): as configs e o `server-overrides/` como pasta.
    fn link_files(&self) -> BTreeMap<String, &[u8]> {
        let mut out = BTreeMap::new();
        for (path, bytes) in &self.files {
            match content::place(path) {
                FilePlace::Server if content::in_link_zip(path) => {
                    out.insert(path.clone(), bytes.as_slice());
                }
                FilePlace::Override(_) => {
                    out.insert(path.clone(), bytes.as_slice());
                }
                _ => {}
            }
        }
        out
    }

    /// Os arquivos do pack que vão com mods dentro, com o `server-overrides/` aplicado.
    fn embedded_files(&self) -> BTreeMap<String, &[u8]> {
        let mut out = BTreeMap::new();
        let mut overrides = Vec::new();
        for (path, bytes) in &self.files {
            match content::place(path) {
                FilePlace::Server => {
                    out.insert(path.clone(), bytes.as_slice());
                }
                FilePlace::Override(rest) => overrides.push((rest, bytes.as_slice())),
                FilePlace::Client | FilePlace::Never => {}
            }
        }
        for (rest, bytes) in overrides {
            out.insert(rest, bytes);
        }
        out
    }

    fn analysis(&self) -> ServerPackAnalysis {
        let client_only = self.client_only();
        let link_files = self.link_files();
        let embedded_files = self.embedded_files();
        let server_items = self.refs.iter().filter(|r| r.on_server()).count();
        let embedded_locals = embedded_files
            .keys()
            .filter(|p| content::is_third_party_local(p))
            .count();
        ServerPackAnalysis {
            version: self.manifest.version.trim().to_owned(),
            minecraft: self.minecraft.clone(),
            loader: self.loader_text(),
            loader_supported: self.loader_kind().is_some(),
            client_only: client_only
                .iter()
                .take(MAX_CLIENT_NAMES)
                .map(|r| r.name.clone())
                .collect(),
            client_only_count: client_only.len(),
            client_files: self
                .files
                .keys()
                .filter(|p| content::place(p) == FilePlace::Client)
                .count(),
            kubejs_client_scripts: self
                .files
                .keys()
                .any(|p| p.starts_with("kubejs/client_scripts/")),
            server_overrides: self
                .files
                .keys()
                .filter(|p| matches!(content::place(p), FilePlace::Override(_)))
                .count(),
            link: ServerPackContents {
                folders: content::folders(link_files.keys().map(String::as_str)),
                server_items,
                third_party: 0,
            },
            embedded: ServerPackContents {
                folders: content::folders(
                    embedded_files
                        .keys()
                        .map(String::as_str)
                        .filter(|p| !content::is_third_party_local(p)),
                ),
                server_items,
                third_party: server_items + embedded_locals,
            },
            manual_downloads: Vec::new(),
            curseforge_key_missing: false,
        }
    }
}

/// Os mods da CurseForge do lado servidor que não estão no cache e que o autor não deixa
/// baixar por apps de terceiros (o download manual da T20 vai abrir antes de gerar). Sem a
/// chave, não dá para conferir: devolve `Err(true)`.
async fn manual_downloads(
    plan: &PackPlan,
    sources: &Sources,
    cancel: &CancellationToken,
) -> Result<std::result::Result<Vec<String>, bool>> {
    let mut pending = Vec::new();
    for item in plan.refs.iter().filter(|r| r.on_server()) {
        let Some((project, file)) = item.curseforge else {
            continue;
        };
        let expected =
            item.meta.hash_format().ok().map(|format| {
                warden_http::ExpectedHash::new(format, item.meta.download.hash.trim())
            });
        let cached = match &expected {
            Some(expected) => sources.cache.find(expected).map_err(err)?.is_some(),
            None => false,
        };
        if !cached {
            pending.push((item, FileRef::new(u64::from(project), u64::from(file))));
        }
    }
    if pending.is_empty() {
        return Ok(Ok(Vec::new()));
    }
    let Some(client) = sources.curseforge.as_ref().filter(|c| c.has_key()) else {
        return Ok(Err(true));
    };
    let refs: Vec<FileRef> = pending.iter().map(|(_, r)| *r).collect();
    let answers = match client.distribution(&refs, Some(cancel)).await {
        Ok(answers) => answers,
        Err(warden_curseforge::Error::Http(warden_http::Error::Cancelled)) => {
            return Err(cancelled());
        }
        Err(warden_curseforge::Error::KeyMissing) => return Ok(Err(true)),
        Err(error) => {
            return Err(Error::new(
                ExportErrorCode::ServerPackLookupFailed,
                error.to_string(),
            ));
        }
    };
    Ok(Ok(pending
        .iter()
        .zip(answers)
        .filter(|(_, answer)| !matches!(answer.distribution, Distribution::Allowed { .. }))
        .map(|((item, _), _)| item.name.clone())
        .collect()))
}

/// O que cada variante vai levar e o que fica de fora. Só lê o pack; com `sources`, confere
/// na CurseForge quais mods vão pedir o download manual com mods dentro.
pub async fn analyze_server_pack(
    root: &Path,
    source: &ExportSource,
    sources: Option<&Sources>,
    cancel: &CancellationToken,
) -> Result<ServerPackAnalysis> {
    let files = indexed_files(&SourceReader::open(root, source)?)?;
    let plan = PackPlan::read(&files)?;
    let mut analysis = plan.analysis();
    if let Some(sources) = sources {
        match manual_downloads(&plan, sources, cancel).await? {
            Ok(names) => analysis.manual_downloads = names,
            Err(key_missing) => analysis.curseforge_key_missing = key_missing,
        }
    }
    Ok(analysis)
}

/// Uma entrada do zip.
struct ZipEntry {
    bytes: Vec<u8>,
    executable: bool,
}

/// Lê um jar fixado do cache e confere o SHA-256.
fn pinned_jar(path: &Path, jar: &PinnedJar) -> Result<Vec<u8>> {
    let bytes = fs::read(path).map_err(err)?;
    if hex::encode(Sha256::digest(&bytes)) != jar.sha256 {
        return Err(Error::new(
            ExportErrorCode::ServerPackInvalidOutput,
            format!("{} não confere com o SHA-256 fixado", jar.file_name),
        ));
    }
    Ok(bytes)
}

/// Os avisos de licença (MIT) do bootstrap e do installer.
fn notices() -> String {
    let bootstrap_version = BOOTSTRAP
        .file_name
        .trim_start_matches("packwiz-installer-bootstrap-")
        .trim_end_matches(".jar");
    let installer_version = INSTALLER
        .file_name
        .trim_start_matches("packwiz-installer-")
        .trim_end_matches(".jar");
    format!(
        "Este pacote leva dois programas de terceiros, sem alterações:\n\
         \n\
         - {BOOTSTRAP_JAR}: packwiz-installer-bootstrap {bootstrap_version}\n  \
           https://github.com/packwiz/packwiz-installer-bootstrap (licença MIT)\n\
         - {INSTALLER_JAR}: packwiz-installer {installer_version}\n  \
           https://github.com/packwiz/packwiz-installer (licença MIT)\n\
         \n\
         Texto da licença MIT, que vale para os dois (no original, o do bootstrap diz\n\
         \"Copyright (c) 2019\" e o do installer, \"Copyright (c) 2021 comp500\"):\n\
         \n\
         MIT License\n\
         \n\
         Permission is hereby granted, free of charge, to any person obtaining a copy of this\n\
         software and associated documentation files (the \"Software\"), to deal in the Software\n\
         without restriction, including without limitation the rights to use, copy, modify,\n\
         merge, publish, distribute, sublicense, and/or sell copies of the Software, and to\n\
         permit persons to whom the Software is furnished to do so, subject to the following\n\
         conditions:\n\
         \n\
         The above copyright notice and this permission notice shall be included in all copies\n\
         or substantial portions of the Software.\n\
         \n\
         THE SOFTWARE IS PROVIDED \"AS IS\", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED,\n\
         INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A\n\
         PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT\n\
         HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF\n\
         CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE\n\
         OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.\n"
    )
}

/// Quantos itens de terceiros iriam dentro do zip (com mods dentro).
fn third_party_count(plan: &PackPlan) -> usize {
    plan.analysis().embedded.third_party
}

/// As conferências que dependem só das escolhas e do pack, antes da cópia e dos downloads:
/// a memória, o destino, a publicação (pelo link), o loader e a confirmação de licença (com
/// mods dentro).
fn check_request<'a>(request: &ServerPackRequest<'a>) -> Result<(Option<&'a Url>, LoaderKind)> {
    let options = request.options;
    if !MEMORY_RANGE.contains(&options.memory_mb) {
        return Err(Error::internal(format!(
            "memória fora da faixa: {} MB",
            options.memory_mb
        )));
    }
    check_destination(request.root, request.destination, false)?;
    let link = match options.variant {
        ServerPackVariant::Link => Some(request.link.ok_or_else(|| {
            Error::new(
                ExportErrorCode::ServerPackNotPublished,
                "o pacote pelo link precisa do pack publicado",
            )
        })?),
        ServerPackVariant::Embedded => None,
    };
    let plan = PackPlan::read(&indexed_files(&SourceReader::open(
        request.root,
        request.source,
    )?)?)?;
    let loader = plan.loader_kind().ok_or_else(|| {
        Error::new(
            ExportErrorCode::ServerPackUnsupportedLoader,
            "o loader do pack não tem scripts de servidor",
        )
        .with_param("loader", plan.loader_text())
    })?;
    if options.variant == ServerPackVariant::Embedded {
        let count = third_party_count(&plan);
        if count > 0 && !options.confirm_third_party {
            return Err(Error::new(
                ExportErrorCode::ServerPackNeedsConfirmation,
                "há arquivos de terceiros sem confirmação de licença",
            )
            .with_param("count", count.to_string()));
        }
    }
    Ok((link, loader))
}

/// Gera o pacote para servidor em `destination` (arquivo novo, fora do pack).
///
/// Etapas: confere o destino e as escolhas; copia o pack e roda o `refresh` (como a exportação
/// nativa); com mods dentro, materializa o lado servidor (com o download manual da T20 para os
/// mods bloqueados) e confere cada jar pelo hash do índice; monta o zip determinístico com os
/// scripts; confere o zip e só então grava. Qualquer falha deixa o destino como estava.
pub async fn export_server_pack(
    request: &ServerPackRequest<'_>,
    progress: &dyn ProgressSink,
    cancel: &CancellationToken,
) -> Result<ServerPackResult> {
    let options = request.options;
    let (link, loader) = check_request(request)?;
    let staged = prepare_stage(
        request.root,
        request.source,
        request.staging_parent,
        request.packwiz,
        cancel,
    )
    .await?;
    let plan = PackPlan::read(&staged.files)?;
    let input = scripts::ScriptInput {
        pack_name: plan.manifest.name.trim(),
        pack_version: plan.manifest.version.trim(),
        minecraft: &plan.minecraft,
        loader,
        loader_version: plan.loader.as_ref().map_or("", |(_, v)| v.as_str()),
        download: request.loader,
        java: request.java,
        link,
    };
    let rendered = scripts::render(&input, options.memory_mb)?;

    let mut forbidden: BTreeSet<String> =
        plan.client_only().iter().map(|r| r.dest.clone()).collect();
    let (mut entries, expected_jars) = match options.variant {
        ServerPackVariant::Link => link_entries(&plan, request.installer)?,
        ServerPackVariant::Embedded => {
            let sources = request
                .sources
                .ok_or_else(|| Error::internal("faltam os downloads para os mods"))?;
            let installed =
                materialize_server(&plan, staged.dir.path(), sources, request, progress, cancel)
                    .await?;
            embedded_entries(&plan, installed)
        }
    };
    check_cancel(cancel)?;
    progress.stage(STAGE_SERVER_ZIP, STAGE_SERVER_ZIP_LABEL);
    for (name, text, executable) in [
        (START_SH, rendered.start_sh, true),
        (START_BAT, rendered.start_bat, false),
        (JVM_ARGS, rendered.jvm_args, false),
    ] {
        entries.insert(
            name.to_owned(),
            ZipEntry {
                bytes: text.into_bytes(),
                executable,
            },
        );
    }
    forbidden.retain(|dest| !expected_jars.contains_key(dest));
    let bytes = build_zip(&entries)?;
    let checked = validate_zip(
        &bytes,
        &Expect {
            variant: options.variant,
            jars: &expected_jars,
            forbidden: &forbidden,
        },
    )?;
    check_cancel(cancel)?;
    crate::prism::write_new(request.destination, &bytes)?;
    Ok(ServerPackResult {
        path: request.destination.to_path_buf(),
        variant: options.variant,
        bytes: bytes.len() as u64,
        sha256: hex::encode(Sha256::digest(&bytes)),
        files: checked.files,
        mods: checked.mods,
        memory_mb: options.memory_mb,
    })
}

/// Jars esperados no zip, pelo caminho, com o hash (`formato:valor`).
type ExpectedJars = BTreeMap<String, String>;

/// O zip pelo link, sem os scripts: as configs, o `server-overrides/` como pasta, o bootstrap e
/// o installer fixados (conferidos pelo SHA-256) e os avisos de licença deles.
fn link_entries(
    plan: &PackPlan,
    jars: Option<&InstallerJars>,
) -> Result<(BTreeMap<String, ZipEntry>, ExpectedJars)> {
    let jars = jars.ok_or_else(|| Error::internal("faltam o bootstrap e o installer"))?;
    let mut entries = BTreeMap::new();
    let mut expected = BTreeMap::new();
    for (path, bytes) in plan.link_files() {
        entries.insert(
            path,
            ZipEntry {
                bytes: bytes.to_vec(),
                executable: false,
            },
        );
    }
    for (name, path, jar) in [
        (BOOTSTRAP_JAR, &jars.bootstrap, &BOOTSTRAP),
        (INSTALLER_JAR, &jars.installer, &INSTALLER),
    ] {
        entries.insert(
            name.to_owned(),
            ZipEntry {
                bytes: pinned_jar(path, jar)?,
                executable: false,
            },
        );
        expected.insert(name.to_owned(), format!("sha256:{}", jar.sha256));
    }
    entries.insert(
        NOTICES_FILE.to_owned(),
        ZipEntry {
            bytes: notices().into_bytes(),
            executable: false,
        },
    );
    Ok((entries, expected))
}

/// O zip com mods dentro, sem os scripts: os arquivos do servidor com o `server-overrides/`
/// aplicado e os jars materializados.
fn embedded_entries(
    plan: &PackPlan,
    installed: Vec<InstalledJar>,
) -> (BTreeMap<String, ZipEntry>, ExpectedJars) {
    let mut entries = BTreeMap::new();
    let mut expected = BTreeMap::new();
    for (path, bytes) in plan.embedded_files() {
        entries.insert(
            path,
            ZipEntry {
                bytes: bytes.to_vec(),
                executable: false,
            },
        );
    }
    for jar in installed {
        expected.insert(jar.dest.clone(), jar.expected);
        entries.insert(
            jar.dest,
            ZipEntry {
                bytes: jar.bytes,
                executable: false,
            },
        );
    }
    (entries, expected)
}

/// Um jar materializado, conferido.
struct InstalledJar {
    dest: String,
    bytes: Vec<u8>,
    /// `formato:valor`, o hash do índice.
    expected: String,
}

/// Repassa o andamento da materialização com a etapa do pacote.
struct ModsProgress<'a>(&'a dyn ProgressSink);

impl ProgressSink for ModsProgress<'_> {
    fn stage(&self, _stage: &str, _label_key: &str) {
        self.0.stage(STAGE_SERVER_MODS, STAGE_SERVER_MODS_LABEL);
    }

    fn progress(&self, progress: Progress) {
        self.0.progress(progress);
    }
}

/// Confere `bytes` contra `formato:valor`.
fn matches_expected(expected: &str, bytes: &[u8]) -> bool {
    let Some((format, value)) = expected.split_once(':') else {
        return false;
    };
    let Ok(format) = HashFormat::parse(format) else {
        return false;
    };
    hash_matches(format, value, &hash_bytes(format, bytes))
}

/// Materializa o lado servidor da cópia de staging numa pasta temporária (todos os opcionais
/// ligados, como o installer sem janela faz) e devolve os jars de referência que vão para o
/// servidor, conferidos pelo hash do índice. Mods bloqueados vão para o download manual da T20
/// e a materialização roda de novo, até não sobrar nenhum.
async fn materialize_server(
    plan: &PackPlan,
    stage: &Path,
    sources: &Sources,
    request: &ServerPackRequest<'_>,
    progress: &dyn ProgressSink,
    cancel: &CancellationToken,
) -> Result<Vec<InstalledJar>> {
    let parent = request.staging_parent;
    fs::create_dir_all(parent).map_err(err)?;
    let temp = tempfile::Builder::new()
        .prefix("server-pack-")
        .tempdir_in(parent)
        .map_err(err)?;
    let dirs = InstanceDirs {
        game_dir: temp.path().join("server"),
        state_dir: temp.path().join("state"),
    };
    let choices = OptionalChoices {
        choices: plan
            .refs
            .iter()
            .filter(|r| r.meta.option.as_ref().is_some_and(|o| o.optional))
            .map(|r| (r.meta_path.clone(), true))
            .collect(),
    };
    let options = MaterializeOptions {
        side: InstallSide::Server,
        on_modified: OnModified::Overwrite,
        optionals: OptionalSelection::Explicit(choices),
        player_tools: PlayerToolsMode::AsPlayer,
        ..MaterializeOptions::default()
    };
    let sink = ModsProgress(progress);
    loop {
        sink.stage("", "");
        let outcome = warden_instance::materialize(sources, stage, &dirs, &options, &sink, cancel)
            .await
            .map_err(|error| {
                if cancel.is_cancelled() {
                    cancelled()
                } else {
                    Error::new(ExportErrorCode::ServerPackDownloadFailed, error.to_string())
                }
            })?;
        let Outcome::Done(report) = outcome else {
            return Err(Error::internal(
                "a pasta temporária do pacote mudou no meio",
            ));
        };
        if let Some(error) = report.incomplete_error() {
            use warden_core::DomainError as _;
            let names = report
                .failed
                .iter()
                .map(|item| item.name.clone())
                .collect::<Vec<_>>()
                .join(", ");
            return Err(
                Error::new(ExportErrorCode::ServerPackDownloadFailed, error.to_string())
                    .with_param("mods", names)
                    .with_detail(error.detail().unwrap_or_default()),
            );
        }
        if report.blocked.is_empty() {
            break;
        }
        let Some(manual) = request.manual else {
            let names = report
                .blocked
                .iter()
                .map(|b| b.name.clone())
                .collect::<Vec<_>>();
            return Err(Error::new(
                ExportErrorCode::ServerPackManualDownloads,
                "mods da CurseForge pedem download manual",
            )
            .with_param("count", names.len().to_string())
            .with_param("mods", names.join(", ")));
        };
        manual.wait(report.blocked).await?;
        check_cancel(cancel)?;
    }
    collect_jars(&dirs, cancel)
}

/// Os jars de referência que a materialização gravou (pelo manifesto dela), relidos e
/// conferidos pelo hash do índice. Resource packs e shaders ficam de fora.
fn collect_jars(dirs: &InstanceDirs, cancel: &CancellationToken) -> Result<Vec<InstalledJar>> {
    let manifest = Manifest::load(&dirs.state_dir).map_err(err)?;
    let mut jars = Vec::new();
    for (dest, entry) in &manifest.files {
        if !entry.index_path.ends_with(".pw.toml") || content::is_client_path(dest) {
            continue;
        }
        check_cancel(cancel)?;
        let path = warden_core::resolve_inside(&dirs.game_dir, dest).map_err(err)?;
        let mut bytes = Vec::new();
        fs::File::open(&path)
            .and_then(|mut file| file.read_to_end(&mut bytes))
            .map_err(err)?;
        if !matches_expected(&entry.expected, &bytes) {
            return Err(Error::new(
                ExportErrorCode::ServerPackDownloadFailed,
                format!("{dest} não confere com o hash do índice"),
            )
            .with_param("mods", dest.clone()));
        }
        jars.push(InstalledJar {
            dest: dest.clone(),
            bytes,
            expected: entry.expected.clone(),
        });
    }
    Ok(jars)
}

/// Descompacta um pacote numa pasta (a validação no servidor local), só com caminhos seguros:
/// um caminho que sairia da pasta recusa o pacote inteiro antes de gravar qualquer arquivo.
pub fn unpack_server_pack(archive: &Path, into: &Path) -> Result<()> {
    let file = fs::File::open(archive).map_err(err)?;
    let mut zip = zip::ZipArchive::new(file).map_err(err)?;
    let mut names = Vec::with_capacity(zip.len());
    for index in 0..zip.len() {
        let entry = zip.by_index(index).map_err(err)?;
        let relative = entry.enclosed_name().ok_or_else(|| {
            Error::new(
                ExportErrorCode::ServerPackInvalidOutput,
                "caminho inseguro no pacote",
            )
        })?;
        names.push((relative, entry.is_dir()));
    }
    fs::create_dir_all(into).map_err(err)?;
    for (index, (relative, is_dir)) in names.into_iter().enumerate() {
        let target = into.join(relative);
        if is_dir {
            fs::create_dir_all(&target).map_err(err)?;
            continue;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(err)?;
        }
        let mut entry = zip.by_index(index).map_err(err)?;
        let mut out = fs::File::create(&target).map_err(err)?;
        std::io::copy(&mut entry, &mut out).map_err(err)?;
    }
    Ok(())
}

fn build_zip(entries: &BTreeMap<String, ZipEntry>) -> Result<Vec<u8>> {
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .last_modified_time(zip::DateTime::default());
    for (path, entry) in entries {
        let mode = if entry.executable { 0o755 } else { 0o644 };
        zip.start_file(path, options.unix_permissions(mode))
            .map_err(err)?;
        zip.write_all(&entry.bytes).map_err(err)?;
    }
    Ok(zip.finish().map_err(err)?.into_inner())
}

/// O que a conferência espera do zip.
struct Expect<'a> {
    variant: ServerPackVariant,
    /// Jars que precisam estar lá com este hash (`formato:valor`).
    jars: &'a BTreeMap<String, String>,
    /// Caminhos que não podem estar lá (os itens "Só cliente").
    forbidden: &'a BTreeSet<String>,
}

/// O que a conferência leu.
#[derive(Debug)]
struct Checked {
    files: Vec<String>,
    mods: usize,
}

/// Confere o zip gerado: scripts com os finais de linha certos, nenhum `eula.txt`, nenhum
/// arquivo de cliente, nenhum item "Só cliente", cada jar esperado com o hash certo e, pelo
/// link, o bootstrap e o installer fixados.
fn validate_zip(bytes: &[u8], expect: &Expect<'_>) -> Result<Checked> {
    let invalid = |what: String| Error::new(ExportErrorCode::ServerPackInvalidOutput, what);
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(err)?;
    let mut files = Vec::with_capacity(archive.len());
    let mut problems = Vec::new();
    let mut found_jars = BTreeSet::new();
    for index in 0..archive.len() {
        let mut file = archive.by_index(index).map_err(err)?;
        let name = file
            .enclosed_name()
            .map(|p| p.to_string_lossy().replace('\\', "/"));
        let Some(name) = name.filter(|n| !n.is_empty() && *n == file.name().unwrap_or_default())
        else {
            problems.push(format!(
                "caminho inseguro: {}",
                file.name().unwrap_or_default()
            ));
            continue;
        };
        let mut data = Vec::new();
        file.read_to_end(&mut data).map_err(err)?;
        if name == content::EULA_FILE {
            problems.push("o pacote não pode levar eula.txt".to_owned());
        }
        if content::is_client_path(&name) || expect.forbidden.contains(&name) {
            problems.push(format!("arquivo de cliente no pacote: {name}"));
        }
        if name.starts_with(OVERRIDES_DIR) && expect.variant == ServerPackVariant::Embedded {
            problems.push(format!("server-overrides não aplicado: {name}"));
        }
        if name == START_SH && data.contains(&b'\r') {
            problems.push("start.sh com CR".to_owned());
        }
        if name == START_BAT
            && data
                .iter()
                .enumerate()
                .any(|(i, b)| *b == b'\n' && (i == 0 || data[i - 1] != b'\r'))
        {
            problems.push("start.bat sem CRLF".to_owned());
        }
        if let Some(expected) = expect.jars.get(&name) {
            if !matches_expected(expected, &data) {
                problems.push(format!("{name} não confere com o hash esperado"));
            }
            found_jars.insert(name.clone());
        }
        files.push(name);
    }
    for required in [START_SH, START_BAT, JVM_ARGS] {
        if !files.iter().any(|f| f == required) {
            problems.push(format!("falta {required}"));
        }
    }
    for jar in expect.jars.keys() {
        if !found_jars.contains(jar) {
            problems.push(format!("falta {jar}"));
        }
    }
    if !problems.is_empty() {
        return Err(invalid(format!(
            "o pacote gerado reprovou em {} conferência(s)",
            problems.len()
        ))
        .with_param("count", problems.len().to_string())
        .with_detail(problems.join("\n")));
    }
    let mods = match expect.variant {
        ServerPackVariant::Link => 0,
        ServerPackVariant::Embedded => expect.jars.len(),
    };
    Ok(Checked { files, mods })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(text: &str) -> ZipEntry {
        ZipEntry {
            bytes: text.as_bytes().to_vec(),
            executable: false,
        }
    }

    fn base() -> BTreeMap<String, ZipEntry> {
        BTreeMap::from([
            (START_SH.to_owned(), entry("#!/bin/sh\necho\n")),
            (START_BAT.to_owned(), entry("@echo off\r\necho.\r\n")),
            (JVM_ARGS.to_owned(), entry("-Xmx4G\n")),
        ])
    }

    #[test]
    fn zip_e_deterministico() {
        let a = build_zip(&base()).unwrap();
        let b = build_zip(&base()).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn conferencia_reprova_eula_cliente_finais_de_linha_e_hash() {
        let jar = b"JAR-DO-SERVIDOR";
        let expected = format!("sha256:{}", hex::encode(Sha256::digest(jar)));
        let jars = BTreeMap::from([("mods/a.jar".to_owned(), expected)]);
        let forbidden = BTreeSet::from(["mods/cliente.jar".to_owned()]);
        let expect = Expect {
            variant: ServerPackVariant::Embedded,
            jars: &jars,
            forbidden: &forbidden,
        };
        let mut ok = base();
        ok.insert(
            "mods/a.jar".into(),
            ZipEntry {
                bytes: jar.to_vec(),
                executable: false,
            },
        );
        let checked = validate_zip(&build_zip(&ok).unwrap(), &expect).unwrap();
        assert_eq!(checked.mods, 1);

        let mut bad = base();
        bad.insert("mods/a.jar".into(), entry("OUTRO"));
        bad.insert("eula.txt".into(), entry("eula=true\n"));
        bad.insert("options.txt".into(), entry("a"));
        bad.insert("mods/cliente.jar".into(), entry("a"));
        bad.insert("server-overrides/x".into(), entry("a"));
        bad.insert(START_SH.into(), entry("#!/bin/sh\r\n"));
        bad.insert(START_BAT.into(), entry("@echo off\n"));
        let error = validate_zip(&build_zip(&bad).unwrap(), &expect).unwrap_err();
        let detail = warden_core::DomainError::detail(&error).unwrap();
        for text in [
            "eula.txt",
            "options.txt",
            "mods/cliente.jar",
            "server-overrides",
            "start.sh com CR",
            "start.bat sem CRLF",
            "mods/a.jar não confere",
        ] {
            assert!(detail.contains(text), "{text}: {detail}");
        }
        let mut missing = base();
        missing.remove(JVM_ARGS);
        let error = validate_zip(&build_zip(&missing).unwrap(), &expect).unwrap_err();
        let detail = warden_core::DomainError::detail(&error).unwrap();
        assert!(detail.contains("falta user_jvm_args.txt"), "{detail}");
        assert!(detail.contains("falta mods/a.jar"), "{detail}");
    }

    #[test]
    fn descompactar_recusa_caminho_para_fora_sem_gravar_nada() {
        let temp = tempfile::tempdir().unwrap();
        let good = temp.path().join("bom.zip");
        fs::write(&good, build_zip(&base()).unwrap()).unwrap();
        unpack_server_pack(&good, &temp.path().join("bom")).unwrap();
        assert!(temp.path().join("bom/start.sh").is_file());
        let bad = temp.path().join("mau.zip");
        let mut zip = zip::ZipWriter::new(fs::File::create(&bad).unwrap());
        zip.start_file("a.txt", SimpleFileOptions::default())
            .unwrap();
        zip.write_all(b"x").unwrap();
        zip.start_file("../fora.txt", SimpleFileOptions::default())
            .unwrap();
        zip.write_all(b"x").unwrap();
        zip.finish().unwrap();
        assert!(unpack_server_pack(&bad, &temp.path().join("dentro")).is_err());
        assert!(!temp.path().join("fora.txt").exists());
        assert!(!temp.path().join("dentro/a.txt").exists());
    }

    #[test]
    fn hash_do_indice_em_qualquer_formato() {
        let data = b"abc";
        assert!(matches_expected(
            &format!("sha1:{}", hash_bytes(HashFormat::Sha1, data)),
            data
        ));
        assert!(!matches_expected("sha1:00", data));
        assert!(!matches_expected("sem-formato", data));
    }

    #[test]
    fn avisos_citam_as_versoes_fixadas() {
        let text = notices();
        assert!(text.contains("packwiz-installer-bootstrap 0.0.3"), "{text}");
        assert!(text.contains("packwiz-installer 0.5.14"), "{text}");
        assert!(text.contains("MIT"));
    }
}
