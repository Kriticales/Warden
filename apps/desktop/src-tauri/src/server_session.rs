//! O servidor local no app (SPEC T27; ARCHITECTURE §7.6; ADR-0032; ROADMAP L-09): as pastas,
//! a EULA, a conta de memória, a instalação e a abertura, um servidor por vez no Warden
//! inteiro, com o evento `server-state`.
//!
//! ## API para as outras tarefas (D-13 busca com servidor, L-12 perfil no servidor, E-03 pacote
//! para servidor)
//!
//! O servidor local nunca abre sozinho: quem usa pede antes à pessoa (ADR-0032) e confere a
//! EULA com [`eula_accepted_at`] (sem aceite, a interface mostra o diálogo da EULA e chama
//! `server_eula_accept`). Depois, na ordem:
//!
//! 1. [`server_dirs`]: a pasta do servidor (`instances/<pack>/server/`) e o estado dele
//!    (`instances/<pack>/state/server/`, com o `manifest.json` da materialização);
//! 2. materializar o pack com `InstallSide::Server` nessas pastas (o Testar usa
//!    `Runner::sync_with`, com os downloads manuais da T20; a E-03 materializa na pasta do
//!    pacote);
//! 3. [`install`]: instala (idempotente, com cache) o servidor do loader do pack;
//! 4. [`configure`]: grava o `eula.txt` (só com o aceite; senão `server.EULA_NOT_ACCEPTED`) e
//!    o `server.properties` controlado, e devolve a porta livre;
//! 5. [`ServerSessions::spawn`]: abre o processo (um por vez, `app.SERVER_ALREADY_RUNNING`
//!    não existe: quem chama espera o anterior), registra a alça para `server_send_command`
//!    e para o "Fechar o Warden", e avisa `server-state`; os eventos
//!    ([`warden_server::ServerEvent`]) vêm com cada linha, o "pronto" e os jogadores que
//!    entraram. [`warden_server::wait_ready`] espera o "pronto" (até [`READY_TIMEOUT`]);
//! 6. comandos pelo stdin com [`ServerSessions::send_command`] (a recarga de scripts da C-07
//!    e o `spark profiler …` da L-12 entram por aqui);
//! 7. parar com [`warden_server::ServerHandle::stop`] (`stop`; à força depois de 60 s) e
//!    esperar a saída; [`diagnose_crash`] dá o "Por que travou" do servidor (o mod apontado,
//!    a classe do cliente e o trecho do log, já redigidos). O registro sai com [`ServerGuard`]
//!    (ao terminar, de qualquer jeito, o `server-state` diz `exited`).
//!
//! Com `WARDEN_E2E_FAKE_SERVER` (só em build de debug, com o jogo simulado do E2E ligado), o
//! "servidor instalado" é o servidor simulado da `warden-server`, com os argumentos do arquivo
//! [`FAKE_ARGS_FILE`] na pasta do pack; todo o resto é o de verdade.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use warden_core::{AppPaths, PackId};
use warden_instance::InstanceDirs;
use warden_launcher::process::GameExit;
use warden_launcher::{GameSpec, JavaRuntime, LaunchCommand};
use warden_server::{
    ControlledProperties, InstalledServer, Installer, RunningServer, ServerEvents, ServerHandle,
    ServerLaunch,
};

use crate::commands::server::{ServerPhase, ServerState};
use crate::error::AppError;
use crate::operations::OperationHandle;
use crate::state::AppState;

/// Etapa "Preparar o servidor" (instalar na primeira vez e copiar o pack só com os itens de
/// servidor).
pub(crate) const STAGE_SERVER: &str = "test.server";
/// Texto da etapa.
pub(crate) const STAGE_SERVER_LABEL: &str = "servidor:etapa.preparar";

/// Quanto o servidor tem para ficar pronto (a primeira abertura gera o mundo).
pub(crate) const READY_TIMEOUT: Duration = Duration::from_mins(15);

/// Pasta do servidor simulado do E2E (com o `WardenFakeServer.class`).
pub(crate) const FAKE_SERVER_ENV: &str = "WARDEN_E2E_FAKE_SERVER";
/// Arquivo na pasta do pack com os argumentos do servidor simulado.
pub(crate) const FAKE_ARGS_FILE: &str = ".warden-fake-server";
/// Arquivo com a memória livre simulada, em MB (E2E do aviso de memória, CA-T27-03).
pub(crate) const FAKE_FREE_MEMORY_ENV: &str = "WARDEN_E2E_FREE_MEMORY_FILE";

/// Quem recebe cada mudança do `server-state`.
pub(crate) type ServerNotify = Arc<dyn Fn(&ServerState) + Send + Sync>;

/// As pastas do servidor local de um pack (ARCHITECTURE §8.1): o servidor em
/// `instances/<pack>/server/` e o estado da materialização em `instances/<pack>/state/server/`.
pub(crate) fn server_dirs(paths: &AppPaths, pack_id: PackId) -> InstanceDirs {
    let root = paths.instance_dir(pack_id);
    InstanceDirs {
        game_dir: root.join("server"),
        state_dir: root.join("state").join("server"),
    }
}

/// O aceite da EULA guardado em `settings.json` (`None`: ainda não aceita ou revogada).
pub(crate) fn eula_accepted_at(state: &AppState) -> Option<String> {
    state
        .settings
        .get()
        .eula_accepted_at
        .filter(|date| !date.trim().is_empty())
}

/// Agora, em RFC 3339 (o formato do `eulaAcceptedAt`).
pub(crate) fn now_rfc3339() -> String {
    warden_launcher::session::rfc3339_utc(SystemTime::now())
}

/// Milissegundos desde 1970.
pub(crate) fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| {
            u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
        })
}

/// Memória livre agora, em MB. No E2E (build de debug), o arquivo de
/// [`FAKE_FREE_MEMORY_ENV`] simula o valor.
pub(crate) fn free_memory_mb() -> Option<u64> {
    #[cfg(debug_assertions)]
    if let Some(file) = std::env::var_os(FAKE_FREE_MEMORY_ENV).filter(|value| !value.is_empty())
        && let Ok(text) = std::fs::read_to_string(file)
        && let Ok(mb) = text.trim().parse::<u64>()
    {
        return Some(mb);
    }
    crate::test_session::available_memory_mb()
}

/// O servidor simulado do E2E, se ligado.
#[derive(Debug, Clone)]
pub(crate) struct FakeServer {
    class_dir: PathBuf,
}

impl FakeServer {
    /// O servidor simulado da variável de ambiente, só em build de debug.
    pub(crate) fn from_env() -> Option<Self> {
        if !cfg!(debug_assertions) {
            return None;
        }
        let class_dir = std::env::var_os(FAKE_SERVER_ENV).filter(|value| !value.is_empty())?;
        Some(Self {
            class_dir: class_dir.into(),
        })
    }

    /// O servidor simulado com a classe na pasta dada (testes).
    #[cfg(test)]
    pub(crate) fn new(class_dir: PathBuf) -> Self {
        Self { class_dir }
    }

    /// O "servidor instalado": a classe com os argumentos do pack.
    fn installed(&self, spec: &GameSpec, pack_root: &Path, dir: &Path) -> InstalledServer {
        let args = std::fs::read_to_string(pack_root.join(FAKE_ARGS_FILE))
            .map(|text| text.split_whitespace().map(ToOwned::to_owned).collect())
            .unwrap_or_default();
        InstalledServer {
            dir: dir.to_path_buf(),
            spec: spec.clone(),
            release_time: "2023-06-12T13:25:51+00:00".into(),
            launch: ServerLaunch::Class {
                class_dir: self.class_dir.clone(),
                main_class: "WardenFakeServer".into(),
                args,
            },
        }
    }
}

/// O instalador com a rede, o catálogo e o cache do app (`<cache>/server`).
fn installer(state: &AppState) -> Result<Installer, AppError> {
    let http = warden_http::HttpClient::new(warden_http::HttpConfig::for_version(env!(
        "CARGO_PKG_VERSION"
    )))
    .map_err(|error| AppError::from_domain(&error))?;
    Ok(Installer::new(
        http,
        Arc::new(state.catalog.clone()),
        state.paths.cache_dir().join("server"),
    ))
}

/// Instala (idempotente) o servidor do pack na pasta dele, com o progresso na operação.
/// Erros: os da `warden-server` (`server.*`) e cancelamento.
pub(crate) async fn install(
    state: &AppState,
    pack_root: &Path,
    spec: &GameSpec,
    java: &JavaRuntime,
    dirs: &InstanceDirs,
    operation: &OperationHandle,
) -> Result<InstalledServer, AppError> {
    if let Some(fake) = state.tests.fake_server() {
        std::fs::create_dir_all(&dirs.game_dir).map_err(|error| {
            AppError::internal(format!("criar a pasta do servidor local: {error}"))
        })?;
        return Ok(fake.installed(spec, pack_root, &dirs.game_dir));
    }
    let installer = installer(state)?;
    operation
        .run_cancellable(async {
            installer
                .install(
                    spec,
                    &dirs.game_dir,
                    &java.java,
                    operation,
                    operation.token(),
                )
                .await
                .map_err(|error| AppError::from_domain(&error))
        })
        .await
}

/// Grava o `eula.txt` (só com o aceite) e o `server.properties` controlado; devolve a porta
/// livre escolhida. Erros: `server.EULA_NOT_ACCEPTED`, `server.NO_FREE_PORT`, disco.
pub(crate) fn configure(state: &AppState, server_dir: &Path, motd: &str) -> Result<u16, AppError> {
    warden_server::write_eula(server_dir, eula_accepted_at(state).as_deref())
        .map_err(|error| AppError::from_domain(&error))?;
    let port = warden_server::properties::free_port_from(state.servers.first_port())
        .map_err(|error| AppError::from_domain(&error))?;
    warden_server::write_properties(
        server_dir,
        &ControlledProperties {
            port,
            motd: motd.to_owned(),
        },
    )
    .map_err(|error| AppError::from_domain(&error))?;
    Ok(port)
}

/// O servidor aberto no Warden.
struct Active {
    state: ServerState,
    handle: Option<ServerHandle>,
}

/// O servidor local do Warden (um por vez) e quem recebe o `server-state`.
pub(crate) struct ServerSessions {
    active: Mutex<Option<Active>>,
    notify: OnceLock<ServerNotify>,
    /// Onde começa a busca da porta livre (25565; nos testes, uma porta dada pelo sistema, para
    /// testes em processos paralelos não disputarem a mesma).
    first_port: AtomicU16,
}

impl Default for ServerSessions {
    fn default() -> Self {
        Self {
            active: Mutex::default(),
            notify: OnceLock::new(),
            first_port: AtomicU16::new(warden_server::properties::FIRST_PORT),
        }
    }
}

impl std::fmt::Debug for ServerSessions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServerSessions")
            .field("active", &self.current().map(|state| state.pack_id))
            .finish_non_exhaustive()
    }
}

impl ServerSessions {
    /// Onde começa a busca da porta livre.
    pub(crate) fn first_port(&self) -> u16 {
        self.first_port.load(Ordering::Relaxed)
    }

    /// Começa a busca da porta em outro lugar (testes).
    #[cfg(test)]
    pub(crate) fn set_first_port(&self, port: u16) {
        self.first_port.store(port, Ordering::Relaxed);
    }

    /// Liga o evento `server-state` (uma vez, na abertura do app).
    pub(crate) fn connect(&self, notify: ServerNotify) {
        let _ = self.notify.set(notify);
    }

    fn lock(&self) -> MutexGuard<'_, Option<Active>> {
        self.active.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn emit(&self, state: &ServerState) {
        if let Some(notify) = self.notify.get() {
            notify(state);
        }
    }

    /// O servidor aberto (ou em preparação), se houver.
    pub(crate) fn current(&self) -> Option<ServerState> {
        self.lock().as_ref().map(|active| active.state.clone())
    }

    /// Começa: o servidor do pack em preparação. Devolve o registro, que sai quando o guarda
    /// é solto.
    pub(crate) fn begin(&self, pack_id: PackId) -> ServerGuard<'_> {
        let state = ServerState {
            pack_id,
            phase: ServerPhase::Preparing,
            port: None,
            started_at_ms: now_ms(),
            ready_ms: None,
        };
        self.emit(&state);
        *self.lock() = Some(Active {
            state,
            handle: None,
        });
        ServerGuard { sessions: self }
    }

    /// Muda o estado e avisa.
    pub(crate) fn update(&self, change: impl FnOnce(&mut ServerState)) {
        let changed = {
            let mut guard = self.lock();
            let Some(active) = guard.as_mut() else {
                return;
            };
            let before = active.state.clone();
            change(&mut active.state);
            (active.state != before).then(|| active.state.clone())
        };
        if let Some(state) = changed {
            self.emit(&state);
        }
    }

    /// Abre o servidor e guarda a alça (comandos e "Fechar o Warden").
    ///
    /// Erros: os da abertura do processo (`server.PROCESS_FAILED`).
    pub(crate) async fn spawn(
        &self,
        command: &LaunchCommand,
        output_log: PathBuf,
        port: u16,
    ) -> Result<(RunningServer, ServerEvents), AppError> {
        let (running, events) = RunningServer::spawn(command, Some(output_log))
            .await
            .map_err(|error| AppError::from_domain(&error))?;
        tracing::info!(pid = running.pid(), port, "servidor local aberto");
        let handle = running.handle();
        if let Some(active) = self.lock().as_mut() {
            active.handle = Some(handle);
        }
        self.update(|state| {
            state.phase = ServerPhase::Starting;
            state.port = Some(port);
        });
        Ok((running, events))
    }

    /// Manda um comando ao servidor aberto do pack. Erros: `server.NOT_RUNNING`,
    /// `server.INVALID_COMMAND`.
    pub(crate) async fn send_command(
        &self,
        pack_id: PackId,
        command: &str,
    ) -> Result<(), AppError> {
        let handle = self
            .lock()
            .as_ref()
            .filter(|active| active.state.pack_id == pack_id)
            .and_then(|active| active.handle.clone())
            .ok_or_else(|| AppError::from_domain(&warden_server::Error::NotRunning))?;
        handle
            .send_command(command)
            .await
            .map_err(|error| AppError::from_domain(&error))
    }

    /// Encerra o servidor aberto na hora ("Fechar o Warden" confirmado; CA-T27-05).
    pub(crate) async fn kill_all(&self) {
        let handle = self
            .lock()
            .as_ref()
            .and_then(|active| active.handle.clone());
        if let Some(handle) = handle {
            tracing::info!("fechando o Warden: servidor local encerrado");
            handle.kill().await;
        }
    }
}

/// Tira o servidor do registro quando o uso termina, de qualquer jeito.
pub(crate) struct ServerGuard<'a> {
    sessions: &'a ServerSessions,
}

impl Drop for ServerGuard<'_> {
    fn drop(&mut self) {
        let finished = self.sessions.lock().take();
        if let Some(mut active) = finished {
            active.state.phase = ServerPhase::Exited;
            self.sessions.emit(&active.state);
        }
    }
}

/// O "Por que travou" do servidor (SPEC T27 passo 4; CA-T27-04).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ServerCrash {
    /// O diagnóstico achou a causa.
    pub(crate) found_cause: bool,
    /// Os mods apontados, do mais provável ao menos provável (nome e caminho no pack).
    pub(crate) mods: Vec<ServerCrashMod>,
    /// A classe do cliente que derrubou o servidor (`net.minecraft.client…`), quando a saída
    /// diz.
    pub(crate) client_class: Option<String>,
    /// O trecho do log do servidor, redigido.
    pub(crate) excerpt: Vec<String>,
    /// O título do achado principal (chave do catálogo `diagnostico`), quando houver.
    pub(crate) title_key: Option<String>,
}

/// Um mod apontado.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ServerCrashMod {
    /// Nome para mostrar.
    pub(crate) name: String,
    /// Caminho no pack (`None`: o mod não está no pack ou o log não diz).
    pub(crate) path: Option<String>,
    /// O lado gravado no pack ("both", "client"…), quando o item está no pack.
    pub(crate) side: Option<String>,
}

/// Analisa o travamento do servidor: o diagnóstico pós-crash da D-03 na pasta do servidor (com
/// o log dele em `log_dir/output.log`), a classe do cliente e o trecho do log.
pub(crate) async fn diagnose_crash(
    state: &AppState,
    pack_id: PackId,
    pack_root: &Path,
    log_dir: &Path,
    server_dir: &Path,
    exit: &GameExit,
    lines: &[String],
) -> ServerCrash {
    let started_at = SystemTime::now()
        .checked_sub(Duration::from_millis(exit.duration_ms) + Duration::from_secs(5))
        .unwrap_or(SystemTime::UNIX_EPOCH);
    let session_id = log_dir
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let request = crate::commands::diagnostics::crash::CrashRequest {
        session_id: &session_id,
        session_dir: log_dir,
        started_at,
        exit_code: exit.exit_code,
    };
    let diagnosis = crate::commands::diagnostics::crash::diagnose_in(
        state, pack_id, &request, server_dir, None,
    )
    .await
    .inspect_err(|error| tracing::warn!(code = %error.code, "diagnóstico do servidor não terminou"))
    .ok();
    let redactor =
        crate::commands::report::redactor(&state.settings.get().player_name, &state.secrets);
    let primary = diagnosis
        .as_ref()
        .filter(|diagnosis| diagnosis.found_cause)
        .and_then(|diagnosis| diagnosis.findings.first());
    let sides = pack_sides(pack_root);
    let mods = primary
        .map(|finding| {
            finding
                .items
                .iter()
                .map(|item| ServerCrashMod {
                    name: item.name.clone(),
                    side: item
                        .path
                        .as_ref()
                        .and_then(|path| sides.iter().find(|(p, _)| p == path))
                        .map(|(_, side)| side.clone()),
                    path: item.path.clone(),
                })
                .collect()
        })
        .unwrap_or_default();
    ServerCrash {
        found_cause: primary.is_some(),
        mods,
        client_class: lines
            .iter()
            .find_map(|line| warden_server::markers::client_class(line)),
        excerpt: warden_server::markers::crash_excerpt(lines)
            .iter()
            .map(|line| redactor.redact(line).text)
            .collect(),
        title_key: primary.map(|finding| finding.finding.title_key().to_owned()),
    }
}

/// O lado de cada metafile do pack (`mods/x.pw.toml` → `both`).
fn pack_sides(pack_root: &Path) -> Vec<(String, String)> {
    let Ok(read) = warden_packwiz::read_pack(pack_root) else {
        return Vec::new();
    };
    read.metafiles
        .iter()
        .filter_map(|metafile| {
            let parsed = metafile.result.as_ref().ok()?;
            Some((metafile.path.clone(), parsed.value.side.as_str().to_owned()))
        })
        .collect()
}

/// Os itens "Só cliente" do pack, que ficam de fora do servidor (nomes, na ordem do índice).
pub(crate) fn client_only_items(pack_root: &Path) -> Vec<String> {
    let Ok(read) = warden_packwiz::read_pack(pack_root) else {
        return Vec::new();
    };
    read.metafiles
        .iter()
        .filter_map(|metafile| {
            let parsed = metafile.result.as_ref().ok()?;
            matches!(parsed.value.side, warden_packwiz::Side::Client).then(|| {
                let name = parsed.value.name.trim();
                if name.is_empty() {
                    metafile.path.clone()
                } else {
                    name.to_owned()
                }
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pastas_do_servidor_ficam_fora_da_instancia_de_teste() {
        let dir = tempfile::tempdir().unwrap();
        let paths = AppPaths::from_dev_root(dir.path()).unwrap();
        let pack = PackId::new();
        let server = server_dirs(&paths, pack);
        let client = InstanceDirs::for_pack(&paths, pack);
        assert!(server.game_dir.ends_with("server"));
        assert_eq!(server.game_dir.parent(), client.game_dir.parent());
        assert!(server.state_dir.starts_with(&client.state_dir));
        assert!(!server.game_dir.starts_with(&client.game_dir));
    }

    #[test]
    fn registro_avisa_cada_mudanca_e_o_fim() {
        let sessions = ServerSessions::default();
        let seen: Arc<Mutex<Vec<ServerPhase>>> = Arc::default();
        let log = Arc::clone(&seen);
        sessions.connect(Arc::new(move |state| {
            log.lock().unwrap().push(state.phase);
        }));
        let pack = PackId::new();
        {
            let _guard = sessions.begin(pack);
            assert_eq!(sessions.current().unwrap().pack_id, pack);
            sessions.update(|state| state.phase = ServerPhase::Ready);
            sessions.update(|state| state.phase = ServerPhase::Ready);
        }
        assert!(sessions.current().is_none());
        assert_eq!(
            *seen.lock().unwrap(),
            [
                ServerPhase::Preparing,
                ServerPhase::Ready,
                ServerPhase::Exited
            ]
        );
    }
}
