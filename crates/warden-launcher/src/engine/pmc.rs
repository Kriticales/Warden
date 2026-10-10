//! Adaptador do motor `portablemc` 5.0.5 (S1; ADR-0057).
//!
//! - A API do portablemc é síncrona e cria um runtime tokio próprio: a instalação roda numa
//!   thread dedicada, e o progresso volta por um canal.
//! - O único ponto de cancelamento do motor é antes do lote de downloads
//!   (`DownloadResources`). Cancelar responde na hora; a thread termina o lote em segundo
//!   plano, segurando a trava de instalação para a próxima esperar.
//! - Versão exata do loader sempre (`LoaderVersion::Name`, `forge::Version::Name`): sem ela o
//!   motor consulta o meta do loader a cada abertura (S-R5-3 §13).
//! - Correções automáticas do motor que mexem em autenticação ou rede ficam desligadas
//!   (`fix_broken_authlib`, proxy legado): ARCHITECTURE §7.2 proíbe contorno de autenticação.
//! - O jogador e a pasta de jogo usados na instalação são provisórios; a linha de comando
//!   troca pelos de verdade ([`crate::command`]).

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use portablemc::{base, fabric, forge, moj};
use serde_json::Value;
use tokio::sync::{mpsc, oneshot};
use warden_core::{CancellationToken, Progress, ProgressSink, error_chain};

use super::logging;
use super::{EngineDirs, LauncherEngine, stages};
use crate::error::{Error, Result};
use crate::range::is_release_time;
use crate::spec::{GameSpec, InstalledGame, JavaRuntime, LoaderSpec};

/// Jogador provisório da instalação (trocado na linha de comando).
const INSTALL_PLAYER: &str = "WardenPlayer";

/// Nome do launcher passado ao jogo (`${launcher_name}`).
const LAUNCHER_NAME: &str = "warden";

/// Quem descobre a coordenada Maven do Forge (`1.7.10-10.13.4.1614-1.7.10`) a partir da
/// versão do `pack.toml` (`10.13.4.1614`). Só é consultado quando o Forge ainda não está
/// instalado.
#[async_trait::async_trait]
pub trait ForgeArtifactResolver: Send + Sync {
    /// A versão Maven (sem o grupo), ou `None` se o Forge não tem essa versão.
    async fn forge_artifact(
        &self,
        minecraft: &str,
        version: &str,
        cancel: &CancellationToken,
    ) -> Result<Option<String>>;
}

/// O catálogo da P1-05 como fonte das coordenadas do Forge.
#[derive(Debug, Clone)]
pub struct CatalogForgeResolver {
    catalog: Arc<warden_catalog::Catalog>,
}

impl CatalogForgeResolver {
    /// Usa o catálogo dado.
    #[must_use]
    pub fn new(catalog: Arc<warden_catalog::Catalog>) -> Self {
        Self { catalog }
    }
}

#[async_trait::async_trait]
impl ForgeArtifactResolver for CatalogForgeResolver {
    async fn forge_artifact(
        &self,
        minecraft: &str,
        version: &str,
        cancel: &CancellationToken,
    ) -> Result<Option<String>> {
        let versions = self
            .catalog
            .loader_versions(
                warden_catalog::Loader::Forge,
                minecraft,
                warden_catalog::Refresh::IfStale,
                Some(cancel),
            )
            .await
            .map_err(|error| Error::DownloadFailed {
                engine: error_chain(&error),
                missing: Vec::new(),
            })?;
        Ok(versions.get(version).map(|item| {
            item.maven
                .rsplit(':')
                .next()
                .unwrap_or(&item.maven)
                .to_owned()
        }))
    }
}

/// O motor portablemc.
pub struct PortableMcEngine {
    dirs: EngineDirs,
    forge_resolver: Option<Arc<dyn ForgeArtifactResolver>>,
    /// Uma instalação por vez no armazenamento compartilhado (inclusive a que continua em
    /// segundo plano depois de um cancelamento).
    install_lock: Arc<Mutex<()>>,
}

impl std::fmt::Debug for PortableMcEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PortableMcEngine")
            .field("dirs", &self.dirs)
            .field("forge_resolver", &self.forge_resolver.is_some())
            .finish_non_exhaustive()
    }
}

impl PortableMcEngine {
    /// Motor com as pastas dadas. Sem `forge_resolver`, o Forge ainda não instalado é pedido
    /// como `<minecraft>-<versão>`.
    #[must_use]
    pub fn new(dirs: EngineDirs, forge_resolver: Option<Arc<dyn ForgeArtifactResolver>>) -> Self {
        Self {
            dirs,
            forge_resolver,
            install_lock: Arc::default(),
        }
    }

    /// As pastas.
    #[must_use]
    pub fn dirs(&self) -> &EngineDirs {
        &self.dirs
    }

    /// O nome que o portablemc espera para o Forge/NeoForge, sem rede quando já instalado.
    async fn loader_name(&self, spec: &GameSpec, cancel: &CancellationToken) -> Result<String> {
        let minecraft = spec.minecraft.as_str();
        match &spec.loader {
            LoaderSpec::NeoForge { version } => neoforge_name(minecraft, version),
            LoaderSpec::Forge { version } => {
                let candidates = forge_candidates(minecraft, version);
                if let Some(installed) = candidates.iter().find(|name| {
                    let id = format!("forge-{name}");
                    self.dirs
                        .versions
                        .join(&id)
                        .join(format!("{id}.json"))
                        .is_file()
                }) {
                    return Ok(installed.clone());
                }
                let Some(resolver) = &self.forge_resolver else {
                    return Ok(candidates[0].clone());
                };
                match resolver.forge_artifact(minecraft, version, cancel).await {
                    Ok(Some(name)) => Ok(name),
                    Ok(None) => Err(Error::LoaderVersionNotFound {
                        loader: "Forge".into(),
                        version: version.clone(),
                        minecraft: minecraft.to_owned(),
                        engine: "o Maven do Forge não lista esta versão".into(),
                    }),
                    Err(Error::Cancelled) => Err(Error::Cancelled),
                    Err(error) => {
                        // Sem catálogo (sem rede e sem cache), tenta o formato comum; o
                        // motor diz se não existir.
                        tracing::warn!(
                            error = %error_chain(&error),
                            "catálogo do Forge indisponível; tentando o nome padrão"
                        );
                        Ok(candidates[0].clone())
                    }
                }
            }
            _ => Ok(String::new()),
        }
    }
}

/// Nomes possíveis do Forge no Maven: `<mc>-<versão>` e `<mc>-<versão>-<mc>` (1.7.10 e
/// parte da 1.8 a 1.12.2). Aceita também a versão já completa.
pub(crate) fn forge_candidates(minecraft: &str, version: &str) -> Vec<String> {
    if version.starts_with(&format!("{minecraft}-")) {
        return vec![version.to_owned()];
    }
    vec![
        format!("{minecraft}-{version}"),
        format!("{minecraft}-{version}-{minecraft}"),
    ]
}

/// O nome do NeoForge no Maven e a conferência da versão do Minecraft que ele implica.
pub(crate) fn neoforge_name(minecraft: &str, version: &str) -> Result<String> {
    let mismatch = |implied: &str| Error::LoaderVersionNotFound {
        loader: "NeoForge".into(),
        version: version.to_owned(),
        minecraft: minecraft.to_owned(),
        engine: format!("o NeoForge {version} é do Minecraft {implied}"),
    };
    if minecraft == "1.20.1" {
        // O fork da 1.20.1 usa o artefato `net.neoforged:forge:1.20.1-<versão>`; a
        // 47.1.82 foi publicada sem o prefixo.
        if version.starts_with("1.20.1-") || version == "47.1.82" {
            return Ok(version.to_owned());
        }
        if version.starts_with("47.") {
            return Ok(format!("1.20.1-{version}"));
        }
        return Err(mismatch("posterior à 1.20.1"));
    }
    let numbers: Vec<u32> = version
        .split(['.', '-'])
        .take(3)
        .map_while(|part| part.parse().ok())
        .collect();
    let implied = match numbers.as_slice() {
        [major, minor, 0, ..] if *major >= 26 => format!("{major}.{minor}"),
        [major, minor, patch, ..] if *major >= 26 => format!("{major}.{minor}.{patch}"),
        [major, 0, ..] => format!("1.{major}"),
        [major, minor, ..] => format!("1.{major}.{minor}"),
        _ => return Err(mismatch("desconhecido")),
    };
    if implied == minecraft {
        Ok(version.to_owned())
    } else {
        Err(mismatch(&implied))
    }
}

/// Mensagem do progresso que a thread do motor manda.
enum EngineMessage {
    Stage(&'static str),
    Progress(Progress),
}

/// Ponte entre os eventos do portablemc e o Warden.
struct Bridge {
    sender: mpsc::UnboundedSender<EngineMessage>,
    cancel: CancellationToken,
    cancelled_here: bool,
    hierarchy: Vec<String>,
}

impl Bridge {
    fn send(&self, message: EngineMessage) {
        // A outra ponta pode já ter ido embora (cancelamento); o progresso some.
        let _ = self.sender.send(message);
    }

    fn base(&mut self, event: base::Event<'_>) {
        use base::Event as E;
        match event {
            E::LoadHierarchy { .. } => self.send(EngineMessage::Stage(stages::VERSION)),
            E::LoadedHierarchy { hierarchy } => {
                self.hierarchy = hierarchy.iter().map(|v| v.name().to_owned()).collect();
                self.send(EngineMessage::Stage(stages::VERIFY));
            }
            E::DownloadResources { cancel } => {
                if self.cancel.is_cancelled() {
                    *cancel = true;
                    self.cancelled_here = true;
                }
                self.send(EngineMessage::Stage(stages::DOWNLOAD));
            }
            E::DownloadProgress {
                size, total_size, ..
            } => self.send(EngineMessage::Progress(Progress::bytes(
                u64::from(size),
                Some(u64::from(total_size)),
            ))),
            E::DownloadedResources => self.send(EngineMessage::Stage(stages::FINALIZE)),
            _ => {}
        }
    }

    fn mojang(&mut self, event: moj::Event<'_>) {
        if let moj::Event::Base(event) = event {
            self.base(event);
        }
    }
}

impl moj::Handler for Bridge {
    fn on_event(&mut self, event: moj::Event<'_>) {
        self.mojang(event);
    }
}

impl fabric::Handler for Bridge {
    fn on_event(&mut self, event: fabric::Event<'_>) {
        if let fabric::Event::Mojang(event) = event {
            self.mojang(event);
        }
    }
}

impl forge::Handler for Bridge {
    fn on_event(&mut self, event: forge::Event<'_>) {
        use forge::Event as E;
        match event {
            E::Mojang(event) => self.mojang(event),
            E::Installing { .. } | E::FetchInstaller { .. } | E::RunInstallerProcessor { .. } => {
                self.send(EngineMessage::Stage(stages::LOADER));
            }
            _ => {}
        }
    }
}

/// Tudo o que a thread do motor precisa (dono, para atravessar a thread).
struct Job {
    dirs: EngineDirs,
    spec: GameSpec,
    loader_name: String,
    java: PathBuf,
}

/// Configura o instalador Mojang que todos os outros usam por baixo.
fn configure(installer: &mut moj::Installer, job: &Job) {
    installer
        .base_mut()
        .set_versions_dir(&job.dirs.versions)
        .set_libraries_dir(&job.dirs.libraries)
        .set_assets_dir(&job.dirs.assets)
        .set_jvm_dir(&job.dirs.jvm)
        .set_bin_dir(&job.dirs.natives)
        .set_mc_dir(&job.dirs.placeholder_game_dir)
        .set_launcher_name(LAUNCHER_NAME)
        .set_launcher_version(env!("CARGO_PKG_VERSION"))
        .set_jvm_policy(base::JvmPolicy::Static(job.java.clone()));
    installer
        .set_auth_offline_username(INSTALL_PLAYER)
        .set_fix_broken_authlib(false)
        .set_fix_legacy_proxy(false)
        .set_fix_legacy_merge_sort(false)
        .set_fix_legacy_quick_play(false)
        .set_fix_legacy_resolution(false);
}

/// O resultado cru da thread.
struct RawInstall {
    game: base::Game,
    hierarchy: Vec<String>,
}

/// Erro da thread, ainda sem tradução (a cadeia do motor e a categoria).
#[derive(Debug)]
enum RawError {
    Cancelled,
    Engine(EngineFailure),
}

/// Categoria do erro do motor.
#[derive(Debug, PartialEq, Eq)]
enum FailureKind {
    VersionNotFound,
    LoaderVersionNotFound,
    LoaderInstall,
    Download,
    Jvm,
    Files,
    Other,
}

#[derive(Debug)]
struct EngineFailure {
    kind: FailureKind,
    chain: String,
    /// Os arquivos do lote de downloads que falharam.
    missing: Vec<PathBuf>,
}

fn failure(kind: FailureKind, error: &(dyn std::error::Error + 'static)) -> RawError {
    let mut chain = error_chain(error);
    let mut missing = Vec::new();
    if let Some(batch) = find_batch(error) {
        for (index, entry) in batch.iter_errors().enumerate() {
            if index < 5 {
                chain.push_str("\n- ");
                chain.push_str(&error_chain(entry));
            }
            missing.push(entry.file().to_path_buf());
        }
    }
    RawError::Engine(EngineFailure {
        kind,
        chain,
        missing,
    })
}

/// O lote de downloads com erro, quando a falha é dele.
fn find_batch<'a>(
    error: &'a (dyn std::error::Error + 'static),
) -> Option<&'a portablemc::download::BatchResult> {
    let mut current = Some(error);
    while let Some(item) = current {
        if let Some(base::Error::Download { batch }) = item.downcast_ref::<base::Error>() {
            return Some(batch);
        }
        current = item.source();
    }
    None
}

fn base_kind(error: &base::Error) -> FailureKind {
    match error {
        base::Error::VersionNotFound { .. } => FailureKind::VersionNotFound,
        base::Error::Download { .. } => FailureKind::Download,
        base::Error::JvmNotFound { .. } => FailureKind::Jvm,
        base::Error::HierarchyLoop { .. }
        | base::Error::AssetsNotFound { .. }
        | base::Error::ClientNotFound { .. }
        | base::Error::LibraryNotFound { .. }
        | base::Error::MainClassNotFound { .. } => FailureKind::Files,
        _ => FailureKind::Other,
    }
}

fn mojang_kind(error: &moj::Error) -> FailureKind {
    match error {
        moj::Error::Base(error) => base_kind(error),
        _ => FailureKind::Other,
    }
}

fn is_cancel(error: &base::Error) -> bool {
    matches!(error, base::Error::DownloadResourcesCancelled { .. })
}

/// A instalação, dentro da thread do motor.
fn run_install(job: &Job, bridge: &mut Bridge) -> std::result::Result<base::Game, RawError> {
    // O motor canoniza estas pastas no fim e falha se alguma não existir (por exemplo
    // `libraries/` numa instalação sem nenhuma biblioteca a baixar).
    for dir in [
        &job.dirs.placeholder_game_dir,
        &job.dirs.versions,
        &job.dirs.libraries,
        &job.dirs.assets,
        &job.dirs.natives,
        &job.dirs.jvm,
    ] {
        fs::create_dir_all(dir).map_err(|error| failure(FailureKind::Files, &error))?;
    }
    let minecraft = job.spec.minecraft.as_str().to_owned();
    match &job.spec.loader {
        LoaderSpec::Vanilla => {
            let mut installer = moj::Installer::new(minecraft);
            configure(&mut installer, job);
            installer
                .install(&mut *bridge)
                .map_err(|error| match &error {
                    moj::Error::Base(base) if is_cancel(base) => RawError::Cancelled,
                    _ => failure(mojang_kind(&error), &error),
                })
        }
        LoaderSpec::Fabric { version } | LoaderSpec::Quilt { version } => {
            let loader = if matches!(job.spec.loader, LoaderSpec::Quilt { .. }) {
                fabric::Loader::Quilt
            } else {
                fabric::Loader::Fabric
            };
            let mut installer = fabric::Installer::new(
                loader,
                fabric::GameVersion::Name(minecraft),
                fabric::LoaderVersion::Name(version.clone()),
            );
            configure(installer.mojang_mut(), job);
            installer
                .install(&mut *bridge)
                .map_err(|error| match &error {
                    fabric::Error::Mojang(moj::Error::Base(base)) if is_cancel(base) => {
                        RawError::Cancelled
                    }
                    fabric::Error::Mojang(inner) => failure(mojang_kind(inner), &error),
                    fabric::Error::GameVersionNotFound { .. } => {
                        failure(FailureKind::VersionNotFound, &error)
                    }
                    fabric::Error::LoaderVersionNotFound { .. }
                    | fabric::Error::LatestVersionNotFound { .. } => {
                        failure(FailureKind::LoaderVersionNotFound, &error)
                    }
                    _ => failure(FailureKind::Other, &error),
                })
        }
        LoaderSpec::Forge { .. } | LoaderSpec::NeoForge { .. } => {
            let loader = if matches!(job.spec.loader, LoaderSpec::Forge { .. }) {
                forge::Loader::Forge
            } else {
                forge::Loader::NeoForge
            };
            let mut installer =
                forge::Installer::new(loader, forge::Version::Name(job.loader_name.clone()));
            configure(installer.mojang_mut(), job);
            installer
                .install(&mut *bridge)
                .map_err(|error| match &error {
                    forge::Error::Mojang(moj::Error::Base(base)) if is_cancel(base) => {
                        RawError::Cancelled
                    }
                    forge::Error::Mojang(inner) => failure(mojang_kind(inner), &error),
                    forge::Error::InstallerNotFound { .. }
                    | forge::Error::LatestVersionNotFound { .. } => {
                        failure(FailureKind::LoaderVersionNotFound, &error)
                    }
                    _ => failure(FailureKind::LoaderInstall, &error),
                })
        }
    }
}

/// Texto de erro que indica falta de conexão (o motor não expõe o tipo do `reqwest`).
fn looks_like_network(chain: &str) -> bool {
    let lower = chain.to_ascii_lowercase();
    [
        "error sending request",
        "tcp connect error",
        "dns error",
        "connection refused",
        "connection reset",
        "timed out",
        "os error 10060",
        "os error 10061",
        "os error 11001",
        "os error 111",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

/// Traduz o erro da thread.
fn translate(error: RawError, spec: &GameSpec, java: &Path) -> Error {
    let RawError::Engine(failure) = error else {
        return Error::Cancelled;
    };
    let EngineFailure {
        kind,
        chain,
        missing,
    } = failure;
    let loader = spec.loader.label().to_owned();
    let minecraft = spec.minecraft.as_str().to_owned();
    match kind {
        FailureKind::VersionNotFound => Error::MinecraftVersionNotFound {
            minecraft,
            engine: chain,
        },
        FailureKind::LoaderVersionNotFound => Error::LoaderVersionNotFound {
            loader,
            version: spec.loader.version().unwrap_or_default().to_owned(),
            minecraft,
            engine: chain,
        },
        FailureKind::Jvm => Error::JavaUnusable {
            path: java.to_path_buf(),
            message: chain,
        },
        _ if looks_like_network(&chain) => Error::NetworkUnavailable {
            engine: chain,
            missing,
        },
        FailureKind::Download => Error::DownloadFailed {
            engine: chain,
            missing,
        },
        FailureKind::LoaderInstall => Error::LoaderInstallFailed {
            loader,
            engine: chain,
        },
        FailureKind::Files => Error::GameFilesInvalid { message: chain },
        FailureKind::Other => Error::Internal { message: chain },
    }
}

/// Lê o JSON de uma versão instalada.
fn read_version_json(versions: &Path, id: &str) -> Result<Value> {
    let path = versions.join(id).join(format!("{id}.json"));
    let bytes = fs::read(&path).map_err(|error| Error::io("ler o JSON da versão", &path, error))?;
    serde_json::from_slice(&bytes).map_err(|error| Error::GameFilesInvalid {
        message: format!("{}: {error}", path.display()),
    })
}

/// Monta o [`InstalledGame`] a partir do que o motor devolveu.
pub(crate) fn finish(
    spec: &GameSpec,
    dirs: &EngineDirs,
    game: base::Game,
    hierarchy: Vec<String>,
) -> Result<InstalledGame> {
    let base::Game {
        main_class,
        mut jvm_args,
        game_args,
        ..
    } = game;
    let jsons = hierarchy
        .iter()
        .map(|id| read_version_json(&dirs.versions, id))
        .collect::<Result<Vec<_>>>()?;
    let release_time = jsons
        .iter()
        .rev()
        .find_map(|json| json.get("releaseTime").and_then(Value::as_str))
        .filter(|time| is_release_time(time))
        .ok_or_else(|| Error::GameFilesInvalid {
            message: format!("{spec}: nenhuma versão da hierarquia tem releaseTime"),
        })?
        .to_owned();
    let decision = logging::decide(
        &spec.loader,
        spec.minecraft.as_str(),
        &release_time,
        &jvm_args,
    );
    if decision.strip_engine_arg {
        logging::strip_config_args(&mut jvm_args);
    }
    let game_dir_placeholder = game_args
        .iter()
        .position(|arg| arg == "--gameDir")
        .and_then(|index| game_args.get(index + 1))
        .cloned()
        .unwrap_or_else(|| dirs.placeholder_game_dir.display().to_string());
    Ok(InstalledGame {
        spec: spec.clone(),
        version_id: hierarchy.first().cloned().unwrap_or_default(),
        hierarchy,
        release_time,
        main_class,
        jvm_args,
        game_args,
        game_dir_placeholder,
        logging: decision.config,
    })
}

#[async_trait::async_trait]
impl LauncherEngine for PortableMcEngine {
    async fn install(
        &self,
        spec: &GameSpec,
        java: &JavaRuntime,
        progress: &dyn ProgressSink,
        cancel: &CancellationToken,
    ) -> Result<InstalledGame> {
        spec.loader.validate()?;
        if cancel.is_cancelled() {
            return Err(Error::Cancelled);
        }
        progress.stage(stages::VERSION, stages::VERSION);
        let loader_name = self.loader_name(spec, cancel).await?;
        let job = Job {
            dirs: self.dirs.clone(),
            spec: spec.clone(),
            loader_name,
            java: java.java.clone(),
        };
        let (sender, mut messages) = mpsc::unbounded_channel();
        let (done, result) = oneshot::channel();
        let lock = Arc::clone(&self.install_lock);
        let thread_cancel = cancel.clone();
        std::thread::Builder::new()
            .name("warden-launcher-install".into())
            .spawn(move || {
                let _guard = lock.lock().unwrap_or_else(PoisonError::into_inner);
                let mut bridge = Bridge {
                    sender,
                    cancel: thread_cancel,
                    cancelled_here: false,
                    hierarchy: Vec::new(),
                };
                let outcome = run_install(&job, &mut bridge).map(|game| RawInstall {
                    game,
                    hierarchy: std::mem::take(&mut bridge.hierarchy),
                });
                let outcome = match outcome {
                    Err(_) if bridge.cancelled_here => Err(RawError::Cancelled),
                    other => other,
                };
                let _ = done.send((outcome, job));
            })
            .map_err(|error| Error::internal(format!("thread do motor: {error}")))?;

        let mut result = result;
        let (outcome, job) = loop {
            tokio::select! {
                message = messages.recv() => match message {
                    Some(EngineMessage::Stage(stage)) => progress.stage(stage, stage),
                    Some(EngineMessage::Progress(value)) => progress.progress(value),
                    None => {}
                },
                finished = &mut result => {
                    break finished
                        .map_err(|_| Error::internal("a thread do motor terminou sem resposta"))?;
                }
                () = cancel.cancelled() => return Err(Error::Cancelled),
            }
        };
        while let Ok(message) = messages.try_recv() {
            match message {
                EngineMessage::Stage(stage) => progress.stage(stage, stage),
                EngineMessage::Progress(value) => progress.progress(value),
            }
        }
        let raw = outcome.map_err(|error| translate(error, spec, &job.java))?;
        finish(spec, &self.dirs, raw.game, raw.hierarchy)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nomes_do_forge() {
        assert_eq!(
            forge_candidates("1.7.10", "10.13.4.1614"),
            vec![
                "1.7.10-10.13.4.1614".to_owned(),
                "1.7.10-10.13.4.1614-1.7.10".to_owned()
            ]
        );
        assert_eq!(
            forge_candidates("1.20.1", "1.20.1-47.4.10"),
            vec!["1.20.1-47.4.10".to_owned()]
        );
    }

    #[test]
    fn nomes_do_neoforge_e_conferencia_da_versao_do_jogo() {
        assert_eq!(
            neoforge_name("1.20.1", "47.1.106").unwrap(),
            "1.20.1-47.1.106"
        );
        assert_eq!(
            neoforge_name("1.20.1", "1.20.1-47.1.106").unwrap(),
            "1.20.1-47.1.106"
        );
        assert_eq!(neoforge_name("1.21.1", "21.1.252").unwrap(), "21.1.252");
        assert_eq!(neoforge_name("1.21", "21.0.167").unwrap(), "21.0.167");
        assert_eq!(neoforge_name("26.2", "26.2.0.88").unwrap(), "26.2.0.88");
        assert_eq!(
            neoforge_name("26.1.2", "26.1.2.15-beta").unwrap(),
            "26.1.2.15-beta"
        );
        for (minecraft, version) in [
            ("1.21.1", "21.4.10"),
            ("1.20.1", "21.1.252"),
            ("26.3", "26.2.0.88"),
            ("1.21.1", "abc"),
        ] {
            let error = neoforge_name(minecraft, version).unwrap_err();
            assert!(
                matches!(error, Error::LoaderVersionNotFound { .. }),
                "{minecraft} {version}"
            );
        }
    }

    #[test]
    fn erros_de_rede_reconhecidos_pelo_texto() {
        assert!(looks_like_network(
            "error sending request for url (https://meta.fabricmc.net/v2/x)\ncausa: tcp connect error"
        ));
        assert!(looks_like_network(
            "A connection attempt failed (os error 10060)"
        ));
        assert!(!looks_like_network("installer processor execution failed"));
    }

    #[test]
    fn traducao_dos_erros_do_motor() {
        let spec = GameSpec::new(
            "1.20.1",
            LoaderSpec::Forge {
                version: "47.9.9".into(),
            },
        )
        .unwrap();
        let java = Path::new("java");
        let engine = |kind, chain: &str| {
            RawError::Engine(EngineFailure {
                kind,
                chain: chain.into(),
                missing: Vec::new(),
            })
        };
        assert!(matches!(
            translate(
                engine(FailureKind::LoaderVersionNotFound, "installer not found"),
                &spec,
                java
            ),
            Error::LoaderVersionNotFound { .. }
        ));
        assert!(matches!(
            translate(
                engine(FailureKind::Download, "download: 2 errors"),
                &spec,
                java
            ),
            Error::DownloadFailed { .. }
        ));
        assert!(matches!(
            translate(
                engine(FailureKind::Download, "tcp connect error"),
                &spec,
                java
            ),
            Error::NetworkUnavailable { .. }
        ));
        // Sem conexão, os arquivos que faltaram seguem até a mensagem.
        let offline = translate(
            RawError::Engine(EngineFailure {
                kind: FailureKind::Download,
                chain: "download: 1 errors over 1 entries\ncausa: tcp connect error".into(),
                missing: vec![PathBuf::from("libraries/net/sf/jopt-simple-5.0.4.jar")],
            }),
            &spec,
            java,
        );
        assert_eq!(
            offline.missing_files(),
            [PathBuf::from("libraries/net/sf/jopt-simple-5.0.4.jar")]
        );
        assert!(matches!(offline, Error::NetworkUnavailable { .. }));
        assert!(matches!(
            translate(
                engine(FailureKind::LoaderInstall, "processor failed"),
                &spec,
                java
            ),
            Error::LoaderInstallFailed { .. }
        ));
        assert!(matches!(
            translate(engine(FailureKind::Jvm, "jvm not found"), &spec, java),
            Error::JavaUnusable { .. }
        ));
        assert!(matches!(
            translate(RawError::Cancelled, &spec, java),
            Error::Cancelled
        ));
    }

    #[derive(Default)]
    struct CountingResolver {
        calls: std::sync::atomic::AtomicUsize,
    }

    #[async_trait::async_trait]
    impl ForgeArtifactResolver for CountingResolver {
        async fn forge_artifact(
            &self,
            minecraft: &str,
            version: &str,
            _cancel: &CancellationToken,
        ) -> Result<Option<String>> {
            self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(Some(format!("{minecraft}-{version}-{minecraft}")))
        }
    }

    #[tokio::test]
    async fn forge_instalado_nao_consulta_o_catalogo() {
        let dir = tempfile::tempdir().unwrap();
        let resolver = Arc::new(CountingResolver::default());
        let engine = PortableMcEngine::new(
            EngineDirs::under(dir.path()),
            Some(Arc::clone(&resolver) as Arc<dyn ForgeArtifactResolver>),
        );
        let spec = GameSpec::new(
            "1.7.10",
            LoaderSpec::Forge {
                version: "10.13.4.1614".into(),
            },
        )
        .unwrap();
        let cancel = CancellationToken::new();
        // Não instalado: pergunta ao catálogo.
        assert_eq!(
            engine.loader_name(&spec, &cancel).await.unwrap(),
            "1.7.10-10.13.4.1614-1.7.10"
        );
        assert_eq!(resolver.calls.load(std::sync::atomic::Ordering::SeqCst), 1);
        // Instalado: acha a pasta e não pergunta de novo.
        let id = "forge-1.7.10-10.13.4.1614-1.7.10";
        let version_dir = dir.path().join("versions").join(id);
        fs::create_dir_all(&version_dir).unwrap();
        fs::write(version_dir.join(format!("{id}.json")), "{}").unwrap();
        assert_eq!(
            engine.loader_name(&spec, &cancel).await.unwrap(),
            "1.7.10-10.13.4.1614-1.7.10"
        );
        assert_eq!(resolver.calls.load(std::sync::atomic::Ordering::SeqCst), 1);
    }
}
