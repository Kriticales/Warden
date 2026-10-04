//! Copiado do spike S1 (única mudança: variante `Quilt`) (branch `spike/launcher-engine`,
//! `spikes/launcher-engine/pmc-proto/src/engine.rs`).
//!
//! Esboço da interface `LauncherEngine` proposta para o Warden e sua implementação
//! sobre a crate `portablemc` (Apache-2.0).
//!
//! Princípio validado neste spike: o motor só precisa **instalar** e devolver um
//! [`PreparedLaunch`] (Java + argumentos + diretório de trabalho). Iniciar, capturar
//! saída, detectar marcadores e encerrar o processo é responsabilidade do Warden
//! (ver `supervisor.rs`), o que deixa a troca de motor barata.

use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use portablemc::{base, fabric, forge, moj};

/// Qual loader instalar sobre a versão do Minecraft.
#[derive(Debug, Clone)]
pub enum LoaderSpec {
    Vanilla,
    /// `None` = versão estável mais recente do Fabric Loader.
    Fabric { loader_version: Option<String> },
    /// Quilt (fork do Fabric), mesma regra de versão.
    Quilt { loader_version: Option<String> },
    /// Versão completa no formato do Maven do Forge, ex.: `1.12.2-14.23.5.2860`.
    Forge { version: String },
    /// Versão do NeoForge, ex.: `21.1.228` (ou `1.20.1-47.1.106` para o fork de 1.20.1).
    NeoForge { version: String },
}

/// Diretórios controlados pelo Warden. Os caches são compartilhados entre instâncias;
/// só `instance_dir` (o `.minecraft` do pack) é por instância.
#[derive(Debug, Clone)]
pub struct EngineDirs {
    pub versions: PathBuf,
    pub libraries: PathBuf,
    pub assets: PathBuf,
    pub runtimes: PathBuf,
    pub natives: PathBuf,
}

impl EngineDirs {
    pub fn under(root: &Path) -> Self {
        Self {
            versions: root.join("versions"),
            libraries: root.join("libraries"),
            assets: root.join("assets"),
            runtimes: root.join("runtimes"),
            natives: root.join("natives"),
        }
    }
}

/// Como escolher o Java. O Warden quer gerenciar o próprio Java (Temurin), então
/// `Path` é o modo previsto em produção; `Mojang` é o download automático do motor.
#[derive(Debug, Clone)]
pub enum JavaSpec {
    Mojang,
    Path(PathBuf),
}

#[derive(Debug, Clone)]
pub struct InstallRequest {
    pub mc_version: String,
    pub loader: LoaderSpec,
    pub instance_dir: PathBuf,
    pub java: JavaSpec,
    /// Nome do jogador offline. O UUID é derivado como no jogo (`OfflinePlayer:<nome>`).
    pub offline_player: String,
}

/// Eventos de progresso, já desacoplados dos tipos da crate do motor.
#[derive(Debug, Clone)]
pub enum EngineEvent {
    Stage(String),
    Progress { count: u32, total_count: u32, size: u32, total_size: u32 },
    JavaSelected { path: PathBuf, version: Option<String>, compatible: bool },
    ProcessorStarted { name: String, task: Option<String> },
    Warning(String),
}

/// Resultado da instalação: tudo o que é preciso para iniciar o jogo.
#[derive(Debug, Clone)]
pub struct PreparedLaunch {
    pub java: PathBuf,
    pub working_dir: PathBuf,
    pub jvm_args: Vec<String>,
    pub main_class: String,
    pub game_args: Vec<String>,
}

impl PreparedLaunch {
    pub fn command(&self) -> Command {
        let mut cmd = Command::new(&self.java);
        cmd.current_dir(&self.working_dir)
            .args(&self.jvm_args)
            .arg(&self.main_class)
            .args(&self.game_args);
        cmd
    }
}

/// Sinal de cancelamento compartilhável entre a UI e a tarefa de instalação.
#[derive(Debug, Clone, Default)]
pub struct CancelFlag(Arc<AtomicBool>);

impl CancelFlag {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

#[derive(Debug)]
pub enum EngineError {
    Cancelled,
    /// Erro do motor, já com a cadeia de causas formatada.
    Engine(String),
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EngineError::Cancelled => write!(f, "instalação cancelada"),
            EngineError::Engine(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for EngineError {}

/// Contrato mínimo de um motor de launcher para o Warden.
///
/// É síncrono de propósito: o app Tauri chama em `spawn_blocking`. Uma versão
/// assíncrona pode envolver esta sem perda.
pub trait LauncherEngine {
    fn install(
        &self,
        dirs: &EngineDirs,
        req: &InstallRequest,
        events: &mut dyn FnMut(EngineEvent),
        cancel: &CancelFlag,
    ) -> Result<PreparedLaunch, EngineError>;
}

// ---------------------------------------------------------------------------------
// Implementação com portablemc
// ---------------------------------------------------------------------------------

pub struct PortableMcEngine;

/// Adaptador dos eventos do portablemc (que emprestam dados) para `EngineEvent`.
struct Bridge<'a> {
    sink: &'a mut dyn FnMut(EngineEvent),
    cancel: &'a CancelFlag,
    cancelled_by_us: bool,
}

impl Bridge<'_> {
    fn base(&mut self, event: base::Event) {
        use base::Event as E;
        let out = match event {
            E::LoadHierarchy { root_version } => EngineEvent::Stage(format!("carregando hierarquia de {root_version}")),
            E::LoadedHierarchy { hierarchy } => EngineEvent::Stage(format!(
                "hierarquia: {}",
                hierarchy.iter().map(|v| v.name()).collect::<Vec<_>>().join(" -> ")
            )),
            E::LoadClient => EngineEvent::Stage("verificando client.jar".into()),
            E::LoadLibraries => EngineEvent::Stage("verificando bibliotecas".into()),
            E::LoadedLibraries { libraries } => EngineEvent::Stage(format!("{} bibliotecas", libraries.len())),
            E::LoadLogger { id } => EngineEvent::Stage(format!("configuração de log {id}")),
            E::LoadAssets { id } => EngineEvent::Stage(format!("índice de assets {id}")),
            E::VerifiedAssets { id, count } => EngineEvent::Stage(format!("assets {id}: {count} objetos verificados")),
            E::LoadJvm { major_version } => EngineEvent::Stage(format!("Java {major_version} necessário")),
            E::WarnJvmUnsupportedPlatform => EngineEvent::Warning("plataforma sem Java da Mojang".into()),
            E::WarnJvmMissingDistribution => EngineEvent::Warning("distribuição de Java da Mojang ausente".into()),
            E::LoadedJvm { file, version, compatible } => EngineEvent::JavaSelected {
                path: file.to_path_buf(),
                version: version.map(str::to_string),
                compatible,
            },
            E::DownloadResources { cancel } => {
                // Único ponto de cancelamento cooperativo exposto pelo portablemc.
                if self.cancel.is_cancelled() {
                    *cancel = true;
                    self.cancelled_by_us = true;
                }
                EngineEvent::Stage("baixando recursos".into())
            }
            E::DownloadProgress { count, total_count, size, total_size } => {
                EngineEvent::Progress { count, total_count, size, total_size }
            }
            E::DownloadedResources => EngineEvent::Stage("downloads concluídos".into()),
            E::ExtractedBinaries { dir } => EngineEvent::Stage(format!("natives em {}", dir.display())),
            _ => return,
        };
        (self.sink)(out);
    }

    fn mojang(&mut self, event: moj::Event) {
        use moj::Event as E;
        match event {
            E::Base(e) => self.base(e),
            E::FetchVersion { version } => (self.sink)(EngineEvent::Stage(format!("baixando metadados de {version}"))),
            E::InvalidatedVersion { version } => (self.sink)(EngineEvent::Warning(format!("metadados de {version} inválidos, rebaixando"))),
            E::FixedLegacyProxy { host, port } => (self.sink)(EngineEvent::Stage(format!("correção legada: proxy {host}:{port}"))),
            E::FixedBrokenAuthlib => (self.sink)(EngineEvent::Stage("correção: authlib quebrada".into())),
            E::FixedLegacyMergeSort => (self.sink)(EngineEvent::Stage("correção legada: merge sort".into())),
            _ => {}
        }
    }
}

impl moj::Handler for Bridge<'_> {
    fn on_event(&mut self, event: moj::Event) {
        self.mojang(event);
    }
}

impl fabric::Handler for Bridge<'_> {
    fn on_event(&mut self, event: fabric::Event) {
        use fabric::Event as E;
        match event {
            E::Mojang(e) => self.mojang(e),
            E::FetchVersion { game_version, loader_version } => (self.sink)(EngineEvent::Stage(format!(
                "baixando perfil Fabric {loader_version} para {game_version}"
            ))),
            _ => {}
        }
    }
}

impl forge::Handler for Bridge<'_> {
    fn on_event(&mut self, event: forge::Event) {
        use forge::Event as E;
        match event {
            E::Mojang(e) => self.mojang(e),
            E::Installing { reason, .. } => (self.sink)(EngineEvent::Stage(format!("instalando loader ({reason:?})"))),
            E::FetchInstaller { version } => (self.sink)(EngineEvent::Stage(format!("baixando instalador {version}"))),
            E::FetchInstallerLibraries => (self.sink)(EngineEvent::Stage("baixando bibliotecas do instalador".into())),
            E::RunInstallerProcessor { name, task } => (self.sink)(EngineEvent::ProcessorStarted {
                name: name.to_string(),
                task: task.map(str::to_string),
            }),
            E::Installed => (self.sink)(EngineEvent::Stage("loader instalado".into())),
            _ => {}
        }
    }
}

/// Formata um erro com toda a cadeia `source()`.
fn chain(err: &dyn std::error::Error) -> String {
    let mut out = err.to_string();
    let mut cur = err.source();
    while let Some(e) = cur {
        out.push_str(&format!(" <- {e}"));
        cur = e.source();
    }
    out
}

fn configure(mojang: &mut moj::Installer, dirs: &EngineDirs, req: &InstallRequest) {
    let base = mojang.base_mut();
    base.set_versions_dir(&dirs.versions)
        .set_libraries_dir(&dirs.libraries)
        .set_assets_dir(&dirs.assets)
        .set_jvm_dir(&dirs.runtimes)
        .set_bin_dir(&dirs.natives)
        .set_mc_dir(&req.instance_dir)
        .set_launcher_name("warden-spike")
        .set_launcher_version("0.0.0")
        .set_jvm_policy(match &req.java {
            JavaSpec::Mojang => base::JvmPolicy::Mojang,
            JavaSpec::Path(p) => base::JvmPolicy::Static(p.clone()),
        });
    mojang.set_auth_offline_username(req.offline_player.clone());
}

impl LauncherEngine for PortableMcEngine {
    fn install(
        &self,
        dirs: &EngineDirs,
        req: &InstallRequest,
        events: &mut dyn FnMut(EngineEvent),
        cancel: &CancelFlag,
    ) -> Result<PreparedLaunch, EngineError> {
        if cancel.is_cancelled() {
            return Err(EngineError::Cancelled);
        }
        let mut bridge = Bridge { sink: events, cancel, cancelled_by_us: false };

        let result = match &req.loader {
            LoaderSpec::Vanilla => {
                let mut inst = moj::Installer::new(req.mc_version.clone());
                configure(&mut inst, dirs, req);
                inst.install(&mut bridge).map_err(|e| chain(&e))
            }
            LoaderSpec::Fabric { loader_version } | LoaderSpec::Quilt { loader_version } => {
                let lv = match loader_version {
                    Some(v) => fabric::LoaderVersion::Name(v.clone()),
                    None => fabric::LoaderVersion::Stable,
                };
                let kind = if matches!(req.loader, LoaderSpec::Quilt { .. }) { fabric::Loader::Quilt } else { fabric::Loader::Fabric };
                let mut inst = fabric::Installer::new(
                    kind,
                    fabric::GameVersion::Name(req.mc_version.clone()),
                    lv,
                );
                configure(inst.mojang_mut(), dirs, req);
                inst.install(&mut bridge).map_err(|e| chain(&e))
            }
            LoaderSpec::Forge { version } | LoaderSpec::NeoForge { version } => {
                let loader = if matches!(req.loader, LoaderSpec::Forge { .. }) {
                    forge::Loader::Forge
                } else {
                    forge::Loader::NeoForge
                };
                let mut inst = forge::Installer::new(loader, forge::Version::Name(version.clone()));
                configure(inst.mojang_mut(), dirs, req);
                inst.install(&mut bridge).map_err(|e| chain(&e))
            }
        };

        match result {
            Ok(game) => Ok(PreparedLaunch {
                java: game.jvm_file,
                working_dir: game.mc_dir,
                jvm_args: game.jvm_args,
                main_class: game.main_class,
                game_args: game.game_args,
            }),
            Err(_) if bridge.cancelled_by_us => Err(EngineError::Cancelled),
            Err(msg) => Err(EngineError::Engine(msg)),
        }
    }
}
