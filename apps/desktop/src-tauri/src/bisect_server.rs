//! A busca do culpado com o servidor local (SPEC T25 e T27; ADR-0031 e ADR-0032; decisão
//! D18; ROADMAP D-13; CA-T25-06).
//!
//! Nas versões sem Quick Play para um mundo de um jogador (antes de 1.20) e nos problemas do
//! servidor, a rodada só entra no mundo por um servidor neste computador. O diálogo explica e
//! pede antes (com a memória e a EULA); o aceite chega no pedido da busca
//! ([`crate::bisect_session::BisectRequest::server`]) e a busca guarda o objetivo
//! [`Goal::Server`](warden_bisect::Goal::Server). O laço, o algoritmo e o veredito são os da
//! D-12 (`bisect_session.rs`); aqui fica só o lado do servidor de cada rodada:
//!
//! 1. **Base** ([`prepare_base`]): o pack só com os itens de servidor (os "Só cliente" ficam de
//!    fora) em `instances/<pack>/bisect-base/server/`, com o servidor do loader instalado pela
//!    L-09 ([`crate::server_session::install`], idempotente e com cache). Fica entre as rodadas
//!    e sai com a base da busca.
//! 2. **Rodada** ([`prepare_round`]): recria `instances/<pack>/bisect/server/` com a mesma
//!    [`warden_instance::subset::materialize_subset`] do cliente (os mesmos itens desligados),
//!    liga os arquivos da instalação do servidor (jars por link físico, o resto copiado),
//!    copia o mundo de teste para `world` (cópia descartável; sem mundo de teste, o servidor
//!    gera um com semente fixa), grava o `eula.txt` (só com o aceite) e o `server.properties`
//!    controlado da L-09 (`127.0.0.1`, porta livre) com `level-name=world`.
//! 3. **Abrir** ([`PreparedRound::open`]): o servidor abre pelo registro da L-09
//!    ([`ServerSessions`]: um por vez, `server-state`, "Fechar o Warden" o encontra) com o log
//!    em `<rodada>/server/output.log`; a D-12 lê os eventos ([`OpenServer::next`]) junto com os
//!    do jogo, que entra no servidor por `--server`/`--port` (antes de 1.20) ou Quick Play
//!    (`--quickPlayMultiplayer`), depois do "Done".
//! 4. **Fechar** ([`OpenServer::close`]): no fim de cada rodada o servidor recebe `stop` (à força
//!    depois de 60 s; na hora, ao cancelar) e o Warden espera ele sair antes de apagar o mundo
//!    descartável. A porta fica livre.
//! 5. **Veredito com os dois lados:** o [`warden_bisect::RoundWatcher`] recebe as linhas do jogo
//!    e as do servidor (o `logged in with entity id` do servidor é o "entrou no mundo"); quando a
//!    falha vem do servidor, a assinatura da D-06 é calculada com o log e os crash reports do
//!    servidor ([`server_files`]) e a rodada fica marcada (`server_failed`).
//!
//! O pack, a instância de teste, os mundos dela e a pasta do "Testar como servidor"
//! (`instances/<pack>/server/`) nunca são usados nem mudam.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use warden_core::{PackId, atomic_write};
use warden_diagnostics::postcrash::SessionFiles;
use warden_instance::InstanceDirs;
use warden_instance::subset::{BISECT_BASE_DIR, BISECT_DIR, SubsetOptions, materialize_subset};
use warden_launcher::JavaRuntime;
use warden_launcher::log::LogLine;
use warden_launcher::process::GameExit;
use warden_server::{
    InstalledServer, RunningServer, ServerCommand, ServerEvent, ServerEvents, ServerHandle,
};

use crate::commands::server::ServerPhase;
use crate::error::AppError;
use crate::operations::OperationHandle;
use crate::server_session::{ServerGuard, ServerSessions};
use crate::state::AppState;

/// Pasta do servidor dentro da base e da rodada da busca.
const SERVER_DIR: &str = "server";
/// Estado da materialização da base do servidor, dentro da base da busca.
const SERVER_STATE_DIR: &str = "server-state";
/// Pasta do mundo do servidor da rodada (o `level-name`).
const WORLD_DIR: &str = "world";
/// Semente do mundo gerado quando a instância de teste não tem mundo (o mesmo em toda rodada).
const GENERATED_SEED: &str = "warden";
/// O que fica na base e não vai para a rodada pela instalação: os mods (só os itens ligados
/// entram, pela materialização do subconjunto), o mundo, os logs, os crash reports e os arquivos
/// que a rodada grava ela mesma.
const NOT_LINKED: &[&str] = &[
    "mods",
    WORLD_DIR,
    "logs",
    "crash-reports",
    "eula.txt",
    warden_server::properties::PROPERTIES_FILE,
];

/// As pastas do servidor da busca de um pack: a base (o pack só com os itens de servidor e o
/// servidor instalado) e a rodada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ServerDirs {
    /// `instances/<pack>/bisect-base/server` (e o estado em `bisect-base/server-state`).
    pub(crate) base: InstanceDirs,
    /// `instances/<pack>/bisect/server`.
    pub(crate) round: PathBuf,
}

impl ServerDirs {
    /// As pastas dentro de `instances/<pack>/`: saem junto com a base e a rodada da busca.
    pub(crate) fn of(instance_root: &Path) -> Self {
        let base = instance_root.join(BISECT_BASE_DIR);
        Self {
            base: InstanceDirs {
                game_dir: base.join(SERVER_DIR),
                state_dir: base.join(SERVER_STATE_DIR),
            },
            round: instance_root.join(BISECT_DIR).join(SERVER_DIR),
        }
    }
}

/// Onde fica o log do servidor de uma rodada (`<rodada>/server/output.log`, como no "Testar
/// como servidor").
pub(crate) fn server_log(round_log_dir: &Path) -> PathBuf {
    crate::test_session::server_log_path(round_log_dir)
}

/// O servidor instalado na base, pronto para as rodadas.
#[derive(Debug, Clone)]
pub(crate) struct ServerBase {
    installed: InstalledServer,
    java: JavaRuntime,
    memory_mb: u32,
}

/// Instala (idempotente) o servidor do loader na base. A materialização do pack com
/// `InstallSide::Server` fica com quem chama (a mesma da base do cliente, com os downloads
/// manuais). Erros: os da `warden-server` (`server.*`) e cancelamento.
pub(crate) async fn prepare_base(
    state: &AppState,
    pack_root: &Path,
    spec: &warden_launcher::GameSpec,
    java: &JavaRuntime,
    dirs: &ServerDirs,
    mod_count: usize,
    handle: &OperationHandle,
) -> Result<ServerBase, AppError> {
    let installed =
        crate::server_session::install(state, pack_root, spec, java, &dirs.base, handle).await?;
    Ok(ServerBase {
        installed,
        java: java.clone(),
        memory_mb: warden_server::server_memory_mb(mod_count),
    })
}

/// O que a rodada do servidor precisa (tudo já resolvido pela D-12).
#[derive(Debug, Clone)]
pub(crate) struct RoundSpec {
    /// Itens do pack que ficam de fora nesta rodada (os mesmos do cliente).
    pub(crate) excluded: BTreeSet<String>,
    /// O mundo de teste da instância de teste (só lido), copiado para `world`.
    pub(crate) world: Option<PathBuf>,
    /// O aceite da EULA guardado nas Configurações (`None`: não aceita; o servidor não abre).
    pub(crate) eula_accepted_at: Option<String>,
    /// Onde a busca da porta livre começa.
    pub(crate) first_port: u16,
    /// A mensagem do servidor (o nome do pack).
    pub(crate) motd: String,
    /// O log do servidor (`<rodada>/server/output.log`).
    pub(crate) log: PathBuf,
}

/// O servidor da rodada pronto para abrir.
#[derive(Debug)]
pub(crate) struct PreparedRound {
    command: warden_launcher::LaunchCommand,
    port: u16,
    log: PathBuf,
    world: WorldCopy,
}

/// Prepara a pasta do servidor da rodada (disco; rode fora da thread do IPC). Sem o aceite da
/// EULA, para antes de qualquer cópia e não grava `eula=true` (CA-T27-02). Erros:
/// `server.EULA_NOT_ACCEPTED`, `server.NO_FREE_PORT`, os da `warden-instance` (disco).
pub(crate) fn prepare_round(
    base: &ServerBase,
    dirs: &ServerDirs,
    spec: &RoundSpec,
) -> Result<PreparedRound, AppError> {
    if spec.eula_accepted_at.is_none() {
        return Err(AppError::from_domain(
            &warden_server::Error::EulaNotAccepted,
        ));
    }
    let options = SubsetOptions {
        excluded: spec.excluded.clone(),
        ..SubsetOptions::default()
    };
    materialize_subset(&dirs.base, &dirs.round, &options)
        .map_err(|error| AppError::from_domain(&error))?;
    link_installation(&dirs.base, &dirs.round)?;
    let world = WorldCopy(dirs.round.join(WORLD_DIR));
    if let Some(source) = &spec.world {
        warden_instance::subset::copy_world(source, &world.0)
            .map_err(|error| AppError::from_domain(&error))?;
    }
    warden_server::write_eula(&dirs.round, spec.eula_accepted_at.as_deref())
        .map_err(|error| AppError::from_domain(&error))?;
    let port = warden_server::properties::free_port_from(spec.first_port)
        .map_err(|error| AppError::from_domain(&error))?;
    warden_server::write_properties(
        &dirs.round,
        &warden_server::ControlledProperties {
            port,
            motd: spec.motd.clone(),
        },
    )
    .map_err(|error| AppError::from_domain(&error))?;
    force_world(&dirs.round, spec.world.is_none())?;
    let installed = InstalledServer {
        dir: dirs.round.clone(),
        ..base.installed.clone()
    };
    let command = ServerCommand {
        java: base.java.clone(),
        memory_mb: base.memory_mb,
        extra_jvm_args: Vec::new(),
    }
    .build(&installed);
    Ok(PreparedRound {
        command,
        port,
        log: spec.log.clone(),
        world,
    })
}

/// Liga na rodada o que a instalação do servidor deixou na base (tudo o que não é do pack:
/// o jar do servidor, `libraries/`, o arquivo de argumentos do Forge moderno, o marcador da
/// instalação…): jars por link físico (o servidor só os lê), o resto copiado (um link levaria
/// para a base o que o servidor regrava).
fn link_installation(base: &InstanceDirs, round: &Path) -> Result<(), AppError> {
    let manifest = warden_instance::Manifest::load(&base.state_dir)
        .map_err(|error| AppError::from_domain(&error))?;
    let pack_files: BTreeSet<&str> = manifest.files.keys().map(String::as_str).collect();
    let mut stack = vec![PathBuf::new()];
    while let Some(relative) = stack.pop() {
        let dir = base.game_dir.join(&relative);
        for entry in fs::read_dir(&dir).map_err(|error| io_error("ler", &relative, &error))? {
            let entry = entry.map_err(|error| io_error("ler", &relative, &error))?;
            let name = entry.file_name();
            let path = relative.join(&name);
            if relative.as_os_str().is_empty()
                && name.to_str().is_some_and(|name| NOT_LINKED.contains(&name))
            {
                continue;
            }
            let kind = entry
                .file_type()
                .map_err(|error| io_error("ler", &path, &error))?;
            if kind.is_dir() {
                stack.push(path);
                continue;
            }
            let key = path.to_string_lossy().replace('\\', "/");
            if !kind.is_file() || pack_files.contains(key.as_str()) {
                continue;
            }
            let target = round.join(&path);
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).map_err(|error| io_error("criar", &path, &error))?;
            }
            let is_jar = Path::new(&name)
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("jar"));
            if !(is_jar && fs::hard_link(entry.path(), &target).is_ok()) {
                fs::copy(entry.path(), &target)
                    .map_err(|error| io_error("copiar", &path, &error))?;
            }
        }
    }
    Ok(())
}

/// Erro de disco com o caminho relativo à pasta do servidor da busca (nunca a do usuário).
fn io_error(action: &str, relative: &Path, error: &dyn std::fmt::Display) -> AppError {
    AppError::internal(format!(
        "{action} {} no servidor da busca: {error}",
        Path::new(SERVER_DIR).join(relative).display()
    ))
}

/// O mundo do servidor é sempre a cópia descartável em `world` (um `level-name` vindo do pack
/// abriria ou geraria outro mundo); sem mundo de teste, o gerado tem a mesma semente em toda
/// rodada.
fn force_world(server_dir: &Path, generated: bool) -> Result<(), AppError> {
    let path = server_dir.join(warden_server::properties::PROPERTIES_FILE);
    let text = fs::read(&path)
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
        .map_err(|error| {
            io_error(
                "ler",
                Path::new(warden_server::properties::PROPERTIES_FILE),
                &error,
            )
        })?;
    let text = with_world(&text, generated);
    atomic_write(&path, text.as_bytes()).map_err(|error| {
        io_error(
            "gravar",
            Path::new(warden_server::properties::PROPERTIES_FILE),
            &error,
        )
    })
}

/// O `server.properties` com `level-name=world` (e a semente fixa, quando o mundo é gerado e o
/// pack não escolheu uma), sem as linhas antigas dessas chaves.
fn with_world(text: &str, generated: bool) -> String {
    let key_of = |line: &str| {
        let trimmed = line.trim_start();
        let end = trimmed.find(['=', ':', ' ', '\t']).unwrap_or(trimmed.len());
        trimmed[..end].trim().to_owned()
    };
    let seed_chosen = text.lines().any(|line| {
        key_of(line) == "level-seed"
            && line
                .split_once('=')
                .is_some_and(|(_, v)| !v.trim().is_empty())
    });
    let mut out = String::with_capacity(text.len() + 40);
    for line in text.lines() {
        if key_of(line) == "level-name" {
            continue;
        }
        if generated && !seed_chosen && key_of(line) == "level-seed" {
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out.push_str("level-name=");
    out.push_str(WORLD_DIR);
    out.push('\n');
    if generated && !seed_chosen {
        out.push_str("level-seed=");
        out.push_str(GENERATED_SEED);
        out.push('\n');
    }
    out
}

/// A cópia descartável do mundo do servidor: sai quando a rodada termina (também em erro ou
/// pânico, pelo `Drop`), sempre depois de o servidor fechar.
#[derive(Debug)]
struct WorldCopy(PathBuf);

impl Drop for WorldCopy {
    fn drop(&mut self) {
        if let Err(error) = warden_instance::subset::remove_dir(&self.0) {
            tracing::warn!(%error, "cópia do mundo do servidor da busca não apagada");
        }
    }
}

/// O que o servidor da rodada fez desde a última leitura.
#[derive(Debug)]
pub(crate) enum ServerSignal {
    /// Uma linha do servidor (o tempo é desde a abertura do servidor).
    Line(LogLine),
    /// Ficou pronto (`Done`).
    Ready,
    /// Saiu (sozinho: travou ou fechou).
    Exited(GameExit),
}

/// O servidor aberto durante uma rodada.
pub(crate) struct OpenServer {
    running: RunningServer,
    events: ServerEvents,
    handle: ServerHandle,
    guard: ServerGuard,
    sessions: Arc<ServerSessions>,
    port: u16,
    exit: Option<GameExit>,
    world: WorldCopy,
}

impl std::fmt::Debug for OpenServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenServer")
            .field("port", &self.port)
            .field("exited", &self.exit.is_some())
            .finish_non_exhaustive()
    }
}

impl PreparedRound {
    /// A porta em `127.0.0.1` em que o jogo entra.
    pub(crate) fn port(&self) -> u16 {
        self.port
    }

    /// Abre o servidor pelo registro da L-09. Erros: `server.PROCESS_FAILED`.
    pub(crate) async fn open(
        self,
        sessions: &Arc<ServerSessions>,
        pack_id: PackId,
    ) -> Result<OpenServer, AppError> {
        let guard = sessions.begin(pack_id);
        // Em erro, o guarda tira o registro e a cópia do mundo sai (os dois `Drop`).
        let (running, events) = sessions
            .spawn(&self.command, self.log.clone(), self.port)
            .await?;
        let handle = running.handle();
        Ok(OpenServer {
            running,
            events,
            handle,
            guard,
            sessions: Arc::clone(sessions),
            port: self.port,
            exit: None,
            world: self.world,
        })
    }
}

impl OpenServer {
    /// Quando o processo do servidor abriu (a origem do relógio da rodada).
    pub(crate) fn started_at(&self) -> SystemTime {
        self.running.started_at()
    }

    /// A porta em `127.0.0.1`.
    pub(crate) fn port(&self) -> u16 {
        self.port
    }

    /// O próximo sinal do servidor. Depois de ele sair, nunca volta (fica pendente), para quem
    /// lê os dois lados num `select!`.
    pub(crate) async fn next(&mut self) -> ServerSignal {
        if self.exit.is_none() {
            while let Some(event) = self.events.recv().await {
                match event {
                    ServerEvent::Line(line) => return ServerSignal::Line(line),
                    ServerEvent::Ready { elapsed_ms, .. } => {
                        self.sessions.update(|state| {
                            state.phase = ServerPhase::Ready;
                            state.ready_ms = Some(elapsed_ms);
                        });
                        return ServerSignal::Ready;
                    }
                    ServerEvent::Joined { .. } => {}
                    ServerEvent::Exited(exit) => {
                        self.exit = Some(exit.clone());
                        return ServerSignal::Exited(exit);
                    }
                }
            }
        }
        std::future::pending().await
    }

    /// Fecha o servidor no fim da rodada: `stop` (à força depois de 60 s), ou encerrado na
    /// hora com `now` (cancelar), espera a saída, tira o registro e apaga o mundo descartável.
    /// Devolve como ele saiu (`None` se a saída não pôde ser lida).
    pub(crate) async fn close(self, now: bool) -> Option<GameExit> {
        let Self {
            running,
            mut events,
            handle,
            guard,
            sessions,
            exit,
            world,
            ..
        } = self;
        if !handle.has_exited() {
            sessions.update(|state| state.phase = ServerPhase::Stopping);
            if now {
                handle.kill().await;
            } else {
                handle.stop().await;
            }
        }
        // O canal fecha com a saída; ler até o fim deixa o log inteiro gravado.
        while events.recv().await.is_some() {}
        let waited = running
            .wait()
            .await
            .inspect_err(|error| tracing::warn!(%error, "saída do servidor da busca não lida"))
            .ok();
        guard.finish().await;
        drop(world);
        exit.or(waited)
    }
}

/// Os arquivos do lado do servidor para a assinatura da D-06 (o log da rodada e os crash
/// reports da pasta do servidor).
pub(crate) fn server_files(
    round_log_dir: &Path,
    dirs: &ServerDirs,
    started_at: SystemTime,
) -> SessionFiles {
    SessionFiles {
        session_dir: server_log(round_log_dir).parent().map(Path::to_path_buf),
        game_dir: dirs.round.clone(),
        started_at,
    }
}

#[cfg(test)]
mod tests;
