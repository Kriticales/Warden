//! "Testar como o jogador recebe" (SPEC T13; ROADMAP L-07; ARCHITECTURE §8.5).
//!
//! Em vez de copiar o pack para a instância de teste, o Warden instala o pack do zero numa
//! instância limpa ([`CleanInstance`]), com o packwiz-installer-bootstrap real e o **link do
//! pack**, como um jogador:
//!
//! 1. ganchos `before_prepare` (o pré-teste da D-03; a captura da C-03 fica de fora: a instância
//!    é apagada no fim e nada dela volta para o pack);
//! 2. origem: o link do pack publicado (ADR-0028) ou, sem publicação (ou com o repositório
//!    privado, cujo link não abre para os jogadores), o pack exportado agora servido só em
//!    `127.0.0.1`, com um aviso;
//! 3. o `pack.toml` do link diz o Minecraft e o loader: Java (o da L-01) e jogo instalados como
//!    no teste normal;
//! 4. os jars do instalador no cache, e o bootstrap com `--bootstrap-no-update`. Mods da
//!    CurseForge bloqueados: o Warden faz o que o jogador faria (usa o arquivo do cache de
//!    downloads ou espera os downloads manuais da T20) e roda o instalador de novo;
//! 5. o `pack.toml` recebido × o da última publicação: diferente, aviso "O GitHub ainda está
//!    entregando a versão anterior";
//! 6. o jogo abre na instância limpa como num teste normal; no fim, o resultado
//!    ([`PlayerReport`], `player` no `session.json`) e os registros do jogo vão para a sessão, e
//!    a instância é apagada.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::{Map, Value};
use sha2::{Digest as _, Sha256};
use warden_core::{AppPaths, Progress, ProgressSink as _};
use warden_export::{ExportFormat, ExportSource};
use warden_http::Url;
use warden_instance::player_install::{
    BlockedNotice, CompareInput, InstallerCommand, LocalPackServer, PlayerReport, PlayerSource,
    Received, STAGE_EXPORT, STAGE_EXPORT_LABEL, STAGE_INSTALL, STAGE_INSTALL_LABEL,
    blocked_notices, compare, failure_reason, supply_blocked,
};
use warden_instance::{CleanInstance, InstallerCache, InstanceDirs, keep_logs};
use warden_java::RuntimeLease;
use warden_launcher::{GameSpec, InstalledGame, JavaRuntime};
use warden_packwiz::PackManifest;
use warden_versioning::PackRepo;
use warden_versioning::github::GithubEndpoints;

use super::{
    Moment, Prepared, Runner, TestSessionSummary, blocking, game_spec, materialize_sources,
    read_pack_info,
};
use crate::error::{AppError, AppErrorCode};
use crate::test_hooks::{ExitContext, HookContext, TestHooks};

/// Em build de debug, troca a URL do Release de onde vêm os jars do instalador (servidor de
/// fixtures do E2E).
const INSTALLER_BASE_ENV: &str = "WARDEN_INSTALLER_BASE";
/// Em build de debug, argumentos extras da JVM do instalador, separados por espaço (o E2E
/// fecha a saída para a internet com um proxy HTTPS que não existe).
const JVM_ARGS_ENV: &str = "WARDEN_PLAYER_INSTALL_JVM_ARGS";

/// Rodadas do instalador: a primeira e, com mods bloqueados, as que vêm depois de pôr os
/// arquivos no lugar.
const MAX_ROUNDS: usize = 3;

/// Campo do `session.json` com o resultado.
pub(crate) const SESSION_FIELD: &str = "player";

/// Ajustes do "como o jogador recebe".
#[derive(Debug, Clone, Default)]
pub(crate) struct PlayerConfig {
    /// De onde baixar os jars (`None`: os Releases do GitHub).
    installer_base: Option<Url>,
    /// Argumentos extras da JVM do instalador.
    jvm_args: Vec<String>,
    /// Troca o `raw.githubusercontent.com` do link (testes; no app e no E2E, `None`: vale o
    /// `GithubEndpoints::from_env`).
    raw_base: Option<Url>,
    /// O sidecar do packwiz da exportação (testes: o binário de `binaries/`).
    #[cfg(test)]
    packwiz: Option<warden_packwiz_cli::Packwiz>,
}

impl PlayerConfig {
    /// O padrão; em build de debug, com [`INSTALLER_BASE_ENV`] e [`JVM_ARGS_ENV`].
    pub(crate) fn from_env() -> Self {
        if !cfg!(debug_assertions) {
            return Self::default();
        }
        Self {
            installer_base: std::env::var(INSTALLER_BASE_ENV)
                .ok()
                .and_then(|text| Url::parse(&text).ok()),
            jvm_args: std::env::var(JVM_ARGS_ENV)
                .unwrap_or_default()
                .split_whitespace()
                .map(ToOwned::to_owned)
                .collect(),
            raw_base: None,
            #[cfg(test)]
            packwiz: None,
        }
    }
}

/// De onde o jogador recebe o pack.
struct Origin {
    source: PlayerSource,
    link: Url,
    /// SHA-256 do `pack.toml` da última publicação.
    published_sha256: Option<String>,
    /// O servidor do pack exportado agora (vive até o instalador terminar).
    server: Option<LocalPackServer>,
    /// A cópia exportada em `cache/staging/` (apagada junto).
    staging: Option<StagingGuard>,
}

/// Apaga a cópia exportada do pack ao sair de escopo (também em erro ou cancelamento).
struct StagingGuard(PathBuf);

impl Drop for StagingGuard {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.0)
            && error.kind() != std::io::ErrorKind::NotFound
        {
            tracing::warn!(%error, "cópia exportada do pack não foi apagada");
        }
    }
}

/// O que a instalação como o jogador preparou para abrir o jogo.
struct Receipt {
    spec: GameSpec,
    java: JavaRuntime,
    java_id: Option<String>,
    lease: Option<RuntimeLease>,
    game: InstalledGame,
    report: PlayerReport,
    received_version: Option<String>,
}

/// O que a instalação deixou.
struct Installed {
    received: Received,
    blocked: Vec<BlockedNotice>,
}

impl Runner<'_> {
    pub(super) async fn run_as_player(&self) -> Result<TestSessionSummary, AppError> {
        let pack_root = crate::commands::inventory::pack_root(self.state, self.pack_id)?;
        let mut hooks = crate::test_hooks::registered();
        hooks.retain(|hook| hook.name() != "capture");
        let tmp = self.state.paths.tmp_dir();
        let id = self.handle.id().to_string();
        let clean = blocking(move || {
            CleanInstance::create(&tmp, &id).map_err(|error| AppError::from_domain(&error))
        })
        .await?;
        let dirs = clean.dirs().clone();
        self.run_hooks(&hooks, &pack_root, &dirs, Moment::BeforePrepare)
            .await?;

        let receipt = self.receive(&pack_root, &dirs, clean.work_dir()).await?;
        let Receipt {
            spec,
            java,
            java_id,
            lease,
            game,
            report,
            received_version,
        } = receipt;
        let quick_play_world = self.prepare_quick_play(&spec, &dirs).await?;
        let root = pack_root.clone();
        let info = blocking(move || read_pack_info(&root)).await?;
        let prepared = Prepared {
            sessions_dir: InstanceDirs::for_pack(&self.state.paths, self.pack_id)
                .state_dir
                .join("sessions"),
            pack_root,
            dirs: dirs.clone(),
            spec,
            pack_version: received_version,
            // A árvore testada é a que o link entregou, não a da pasta do pack: a sessão não
            // conta como teste da versão local (aviso "versão não testada" da V-03).
            pack_tree: None,
            mod_count: info.mod_count,
            game,
            java,
            java_id,
            quick_play_world,
            _lease: lease,
        };
        self.run_hooks(
            &hooks,
            &prepared.pack_root,
            &prepared.dirs,
            Moment::BeforeLaunch,
        )
        .await?;
        hooks.push(Arc::new(PlayerHook {
            report,
            game_dir: dirs.game_dir.clone(),
        }));
        let summary = self.play(&hooks, prepared).await;
        if let Err(error) = blocking(move || {
            clean
                .remove()
                .map_err(|error| AppError::from_domain(&error))
        })
        .await
        {
            tracing::warn!(code = %error.code, "instância limpa não foi apagada; a próxima tenta de novo");
        }
        summary
    }

    /// Passos 2 a 5: a origem, o Java e o jogo do `pack.toml` do link, a instalação como o
    /// jogador e o que ele recebeu × o pack.
    async fn receive(
        &self,
        pack_root: &Path,
        dirs: &InstanceDirs,
        work_dir: PathBuf,
    ) -> Result<Receipt, AppError> {
        let origin = self.origin(pack_root).await?;
        let pack_toml = self.fetch_pack_toml(&origin.link).await?;
        let manifest = PackManifest::parse(&String::from_utf8_lossy(&pack_toml))
            .map_err(|error| AppError::from_domain(&error))?
            .value;
        let requirement = warden_instance::game_requirement(&manifest)
            .map_err(|error| AppError::from_domain(&error))?;
        let spec = game_spec(&requirement)?;
        let (java, java_id, lease) = self.choose_java(&requirement, &spec).await?;
        let game = self.install_game(&spec, &java, pack_root).await?;
        let installed = self
            .install(pack_root, dirs, &java.java, &origin.link, work_dir)
            .await?;
        drop((origin.server, origin.staging));

        let received_version =
            Some(manifest.version.trim().to_owned()).filter(|version| !version.is_empty());
        let report = {
            let (root, game_dir) = (pack_root.to_path_buf(), dirs.game_dir.clone());
            let (source, published) = (origin.source, origin.published_sha256);
            let version = received_version.clone();
            blocking(move || {
                compare(&CompareInput {
                    pack_root: &root,
                    game_dir: &game_dir,
                    received: &installed.received,
                    blocked: &installed.blocked,
                    source,
                    received_version: version,
                    published_sha256: published,
                })
                .map_err(|error| AppError::from_domain(&error))
            })
            .await?
        };
        if report.outdated {
            self.handle
                .warn(AppError::new(AppErrorCode::TestPlayerOldVersion));
        }
        tracing::info!(
            diferencas = report.items.len(),
            recebidos = report.received_files,
            "pack instalado como o jogador recebe"
        );

        Ok(Receipt {
            spec,
            java,
            java_id,
            lease,
            game,
            report,
            received_version,
        })
    }

    /// O link do pack publicado ou, sem publicação, o pack exportado agora servido aqui.
    async fn origin(&self, pack_root: &Path) -> Result<Origin, AppError> {
        let link = crate::commands::github::stored_link(self.state, self.pack_id)?;
        let root = pack_root.to_path_buf();
        let published = blocking(move || {
            // Sem histórico de versões, não há publicação.
            let Ok(repo) = PackRepo::open(&root) else {
                return Ok(None);
            };
            let version = repo
                .last_published()
                .map_err(|error| AppError::from_domain(&error))?;
            let pack_toml = repo
                .published_file("pack.toml")
                .map_err(|error| AppError::from_domain(&error))?;
            Ok(version.zip(pack_toml))
        })
        .await?;
        if let (Some(link), Some((version, pack_toml))) = (link.filter(|l| !l.private), published) {
            let endpoints = self.endpoints();
            let text = endpoints.pack_link(&link.repo());
            let url = Url::parse(&text)
                .map_err(|error| AppError::internal(format!("link do pack: {error}")))?;
            return Ok(Origin {
                source: PlayerSource::Published {
                    link: text,
                    version: Some(version.to_string()),
                },
                link: url,
                published_sha256: Some(hex::encode(Sha256::digest(&pack_toml))),
                server: None,
                staging: None,
            });
        }
        self.handle
            .warn(AppError::new(AppErrorCode::TestPlayerLocalSource));
        let staging = StagingGuard(self.state.paths.staging_dir(self.handle.id()));
        let exported = self.export_now(pack_root, &staging.0).await?;
        let server = LocalPackServer::start(exported)
            .await
            .map_err(|error| AppError::from_domain(&error))?;
        let link = server
            .link("pack.toml")
            .map_err(|error| AppError::from_domain(&error))?;
        Ok(Origin {
            source: PlayerSource::Local,
            link,
            published_sha256: None,
            server: Some(server),
            staging: Some(staging),
        })
    }

    fn endpoints(&self) -> GithubEndpoints {
        let mut endpoints = GithubEndpoints::from_env();
        if let Some(raw) = &self.state.tests.player.raw_base {
            endpoints.raw = raw.clone();
        }
        endpoints
    }

    /// Exporta o estado atual (E-01) para `<staging>/pack`.
    async fn export_now(&self, pack_root: &Path, staging: &Path) -> Result<PathBuf, AppError> {
        self.handle.stage(STAGE_EXPORT, STAGE_EXPORT_LABEL);
        let paths: &AppPaths = &self.state.paths;
        let destination = staging.join("pack");
        let parent = staging.to_path_buf();
        blocking(move || {
            std::fs::create_dir_all(&parent).map_err(|error| {
                AppError::internal(format!("criar a pasta da exportação: {error}"))
            })
        })
        .await?;
        #[cfg(test)]
        let packwiz = match &self.state.tests.player.packwiz {
            Some(packwiz) => packwiz.clone(),
            None => crate::commands::export::packwiz(self.state)?,
        };
        #[cfg(not(test))]
        let packwiz = crate::commands::export::packwiz(self.state)?;
        let _read = self.state.locks.read_for(self.pack_id, self.handle).await;
        warden_export::export(
            pack_root,
            &ExportSource::Current,
            ExportFormat::Folder,
            &destination,
            &paths.cache_dir().join("staging"),
            &packwiz,
            self.handle.token(),
        )
        .await
        .map_err(|error| {
            if self.handle.is_cancelled() {
                AppError::cancelled()
            } else {
                AppError::from_domain(&error)
            }
        })?;
        Ok(destination)
    }

    /// O `pack.toml` do link, como o jogador receberia. Erros:
    /// `app.TEST_PLAYER_LINK_UNREACHABLE`.
    async fn fetch_pack_toml(&self, link: &Url) -> Result<Vec<u8>, AppError> {
        let http = warden_http::HttpClient::new(warden_http::HttpConfig::for_version(env!(
            "CARGO_PKG_VERSION"
        )))
        .map_err(|error| AppError::from_domain(&error))?;
        let unreachable = |status: String| {
            AppError::new(AppErrorCode::TestPlayerLinkUnreachable).with_param("status", status)
        };
        let response = http
            .get(link.clone())
            .cancel(self.handle.token())
            .max_body(1024 * 1024)
            .send()
            .await
            .map_err(|error| {
                if self.handle.is_cancelled() {
                    AppError::cancelled()
                } else {
                    let status = error.status().map(|status| status.to_string());
                    unreachable(status.unwrap_or_default()).with_detail(error.to_string())
                }
            })?;
        let status = response.status();
        if !status.is_success() {
            return Err(unreachable(status.as_u16().to_string()));
        }
        response
            .bytes_limited(1024 * 1024)
            .await
            .map_err(|error| unreachable(String::new()).with_detail(error.to_string()))
    }

    /// Os jars no cache e o instalador, de novo a cada rodada com mods bloqueados postos no
    /// lugar.
    async fn install(
        &self,
        pack_root: &Path,
        dirs: &InstanceDirs,
        java: &Path,
        link: &Url,
        work_dir: PathBuf,
    ) -> Result<Installed, AppError> {
        let sources = materialize_sources(self.state)?;
        let config = &self.state.tests.player;
        let jars = InstallerCache::new(self.state.paths.cache_dir().join("installer"))
            .with_base(config.installer_base.clone())
            .ensure(&sources.http, self.handle, self.handle.token())
            .await
            .map_err(|error| AppError::from_domain(&error))?;
        let command = InstallerCommand {
            java: java.to_path_buf(),
            jars,
            link: link.clone(),
            pack_folder: dirs.game_dir.clone(),
            work_dir,
            extra_jvm_args: config.jvm_args.clone(),
            update_url: None,
        };
        let mut blocked: Vec<BlockedNotice> = Vec::new();
        for _ in 0..MAX_ROUNDS {
            self.handle.stage(STAGE_INSTALL, STAGE_INSTALL_LABEL);
            let handle = self.handle;
            let run = command
                .run(self.handle.token(), &|line| {
                    if let Some(progress) = installer_progress(line) {
                        handle.progress(progress);
                    }
                })
                .await
                .map_err(|error| AppError::from_domain(&error))?;
            let notices = blocked_notices(&run.lines, &dirs.game_dir);
            for notice in &notices {
                if !blocked.iter().any(|known| known.path == notice.path) {
                    blocked.push(notice.clone());
                }
            }
            if run.success {
                let game_dir = dirs.game_dir.clone();
                let received = blocking(move || {
                    Received::read(&game_dir).map_err(|error| AppError::from_domain(&error))
                })
                .await?;
                return Ok(Installed { received, blocked });
            }
            if notices.is_empty() {
                return Err(AppError::from_domain(
                    &warden_instance::Error::PlayerInstallFailed {
                        reason: failure_reason(&run.lines, &dirs.game_dir),
                    },
                ));
            }
            let supplied = {
                let (root, cache, game_dir) = (
                    pack_root.to_path_buf(),
                    Arc::clone(&sources.cache),
                    dirs.game_dir.clone(),
                );
                let notices = notices.clone();
                blocking(move || {
                    supply_blocked(&notices, &root, &cache, &game_dir)
                        .map_err(|error| AppError::from_domain(&error))
                })
                .await?
            };
            if !supplied.unknown.is_empty() {
                return Err(blocked_error(&notices));
            }
            if !supplied.waiting.is_empty() {
                // Como o jogador: baixar à mão pelo site (T20). O teste continua sozinho.
                self.state
                    .manual_downloads
                    .wait(
                        self.pack_id,
                        supplied.waiting,
                        Arc::clone(&sources.cache),
                        self.handle,
                    )
                    .await?;
            }
        }
        Err(blocked_error(&blocked))
    }
}

/// Mods bloqueados que o Warden não conseguiu pôr no lugar do jogador.
fn blocked_error(notices: &[BlockedNotice]) -> AppError {
    let names: Vec<&str> = notices
        .iter()
        .map(|notice| notice.path.rsplit('/').next().unwrap_or(&notice.path))
        .collect();
    AppError::new(AppErrorCode::TestPlayerBlocked)
        .with_param("count", notices.len().to_string())
        .with_param("names", names.join(", "))
}

/// O andamento de uma linha do instalador (`(3/12) Downloaded sodium`).
fn installer_progress(line: &str) -> Option<Progress> {
    let inner = line.strip_prefix('(')?.split(')').next()?;
    let (current, total) = inner.split_once('/')?;
    Some(Progress::items(
        current.trim().parse().ok()?,
        Some(total.trim().parse().ok()?),
    ))
}

/// Grava o resultado na sessão e guarda os registros do jogo antes de a instância sumir.
struct PlayerHook {
    report: PlayerReport,
    game_dir: PathBuf,
}

#[async_trait::async_trait]
impl TestHooks for PlayerHook {
    fn name(&self) -> &'static str {
        "player"
    }

    async fn after_exit(
        &self,
        _context: &HookContext<'_>,
        exit: &ExitContext<'_>,
    ) -> Result<Map<String, Value>, AppError> {
        let (game_dir, session_dir) = (self.game_dir.clone(), exit.session_dir.to_path_buf());
        let kept = blocking(move || {
            keep_logs(&game_dir, &session_dir).map_err(|error| AppError::from_domain(&error))
        })
        .await;
        match kept {
            Ok(kept) => tracing::info!(arquivos = kept.len(), "registros do jogador guardados"),
            Err(error) => tracing::warn!(code = %error.code, "registros do jogador não guardados"),
        }
        let value = serde_json::to_value(&self.report)
            .map_err(|error| AppError::internal(format!("gravar o resultado: {error}")))?;
        let mut fields = Map::new();
        fields.insert(SESSION_FIELD.to_owned(), value);
        Ok(fields)
    }
}

/// O resultado gravado numa sessão (`None`: sessão de outro modo).
pub(crate) fn report_of(record: &warden_launcher::session::SessionRecord) -> Option<PlayerReport> {
    record
        .extra
        .get(SESSION_FIELD)
        .and_then(|value| serde_json::from_value(value.clone()).ok())
}

#[cfg(test)]
mod tests;
