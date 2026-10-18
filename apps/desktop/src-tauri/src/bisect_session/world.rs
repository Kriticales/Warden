//! O mundo de teste da busca do culpado (ROADMAP D-12 item 7; ARCHITECTURE §9.7 item 9;
//! S-R5-3 §14 item 7; decisão D18).
//!
//! As rodadas que entram no mundo não podem abrir o mundo do usuário: ele foi salvo com todos os
//! mods, e uma rodada sem os mods de geração de mundo (biomas, dimensões, registros) abriria um
//! mundo com conteúdo ausente e travaria "diferente". Por isso a busca usa um mundo **gerado sem
//! mods**:
//!
//! 1. **Modelo:** na primeira busca do pack (e quando a versão do Minecraft muda), o servidor
//!    vanilla da mesma versão, instalado e aberto pela `warden-server` (o mesmo caminho do
//!    "Testar como servidor": [`crate::server_session`]), gera o mundo com a semente fixa
//!    [`WORLD_SEED`]; o Warden espera o "pronto" (`Done`), para o servidor com `stop` (o mundo é
//!    salvo) e guarda a pasta `world` como modelo em
//!    `instances/<pack>/state/bisect-world/world/`, com [`MARKER_FILE`] dizendo a versão e a
//!    semente. O servidor só abre com o aceite da EULA (`settings.json`, o mesmo da L-09).
//! 2. **Rodadas:** cada rodada copia o modelo para `saves/`[`WORLD_NAME`] da instância da rodada
//!    (`warden_instance::subset`, sem o `session.lock`) e o abre por Quick Play; a cópia some com
//!    a instância da rodada. O modelo fica para as próximas buscas.
//!
//! O pack, a instância de teste e os mundos do usuário nunca são lidos nem tocados aqui. A pasta
//! em que o servidor roda (`instances/<pack>/bisect-world-server/`) é apagada no começo e no fim
//! da geração, dê certo ou não.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use warden_core::{AppPaths, PackId, atomic_write};
use warden_instance::InstanceDirs;
use warden_launcher::{GameSpec, JavaRuntime, LoaderSpec};
use warden_server::{ReadyFailure, ServerCommand};

use super::blocking;
use crate::commands::server::ServerPhase;
use crate::error::AppError;
use crate::operations::OperationHandle;
use crate::state::AppState;

/// Semente fixa do mundo de teste: todas as buscas, de todos os packs, partem do mesmo terreno,
/// então duas rodadas só diferem pelos mods ligados. O número não tem significado (é a data em
/// que o S-R5-3 terminou); trocar a semente gera o modelo de novo na próxima busca.
pub(crate) const WORLD_SEED: i64 = 20_261_004;

/// Nome da pasta do mundo em `saves/` da rodada (o que o Quick Play abre).
pub(crate) const WORLD_NAME: &str = "warden-mundo-de-teste";

/// Pasta do modelo dentro de `instances/<pack>/state/`.
const TEMPLATE_DIR: &str = "bisect-world";
/// O mundo dentro da pasta do modelo.
const TEMPLATE_WORLD: &str = "world";
/// A versão e a semente do modelo (gravado por último: sem ele, o modelo não vale).
pub(crate) const MARKER_FILE: &str = "mundo.json";
/// O log do servidor da última geração, na pasta do modelo.
const SERVER_LOG: &str = "servidor.log";
/// Pasta em que o servidor vanilla roda, dentro de `instances/<pack>/`.
const SERVER_DIR: &str = "bisect-world-server";
/// `level-name` do servidor (a pasta que ele cria).
const LEVEL_NAME: &str = "world";
/// O arquivo que prova que o servidor salvou o mundo.
const LEVEL_DAT: &str = "level.dat";
/// A trava do mundo que o servidor deixa (não vai para o modelo).
const SESSION_LOCK: &str = "session.lock";
/// Mensagem do servidor (aparece só no log dele).
const MOTD: &str = "Warden: mundo de teste da busca do culpado";

/// Quanto o servidor tem para gerar o mundo e ficar pronto (a primeira abertura baixa o servidor
/// antes, fora deste prazo). Nos testes, o servidor simulado fica pronto em menos de 1 s.
#[cfg(not(test))]
const READY_LIMIT: Duration = crate::server_session::READY_TIMEOUT;
#[cfg(test)]
const READY_LIMIT: Duration = Duration::from_secs(10);

/// Tentativas de mover o mundo gerado para o modelo (no Windows, o servidor que acabou de sair
/// pode segurar um arquivo da pasta por alguns instantes).
const MOVE_ATTEMPTS: u32 = 5;

/// O que [`MARKER_FILE`] guarda.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Marker {
    minecraft: String,
    seed: i64,
}

/// As pastas do mundo de teste de um pack.
#[derive(Debug, Clone)]
pub(crate) struct WorldDirs {
    /// `instances/<pack>/state/bisect-world` (o modelo, o marcador e o log do servidor).
    template: PathBuf,
    /// Onde o servidor vanilla roda (apagada depois de cada geração).
    server: InstanceDirs,
}

impl WorldDirs {
    pub(crate) fn of(paths: &AppPaths, pack_id: PackId) -> Self {
        let test = InstanceDirs::for_pack(paths, pack_id);
        let server_root = paths.instance_dir(pack_id).join(SERVER_DIR);
        Self {
            template: test.state_dir.join(TEMPLATE_DIR),
            server: InstanceDirs {
                game_dir: server_root.join("server"),
                state_dir: server_root.join("state"),
            },
        }
    }

    /// O mundo modelo (a fonte da cópia de cada rodada).
    pub(crate) fn world(&self) -> PathBuf {
        self.template.join(TEMPLATE_WORLD)
    }

    fn marker(&self) -> PathBuf {
        self.template.join(MARKER_FILE)
    }

    /// `instances/<pack>/bisect-world-server`.
    fn server_root(&self) -> PathBuf {
        self.server
            .game_dir
            .parent()
            .map_or_else(|| self.server.game_dir.clone(), Path::to_path_buf)
    }

    /// O modelo já existe para esta versão do Minecraft e esta semente.
    pub(crate) fn ready(&self, minecraft: &str) -> bool {
        let Ok(bytes) = std::fs::read(self.marker()) else {
            return false;
        };
        serde_json::from_slice::<Marker>(&bytes).is_ok_and(|marker| {
            marker.minecraft == minecraft && marker.seed == WORLD_SEED
        }) && self.world().join(LEVEL_DAT).is_file()
    }
}

/// O começo do `server.properties` da geração: a semente, a pasta do mundo e um mundo calmo
/// (pacífico e criativo: o jogador não morre nos 20 s de estabilidade de cada rodada). Os
/// valores são números porque o servidor da 1.7.10 só entende números; os novos aceitam os dois.
/// As chaves de segurança (`127.0.0.1`, porta livre, `online-mode=false`…) vêm depois, da
/// `warden-server`.
fn generation_properties() -> String {
    format!(
        "# Mundo de teste da busca do culpado (Warden): sem mods, semente fixa.\n\
         level-name={LEVEL_NAME}\n\
         level-seed={WORLD_SEED}\n\
         difficulty=0\n\
         gamemode=1\n"
    )
}

/// Garante o mundo modelo do pack para `minecraft`: reaproveita o que existe ou gera um novo
/// com o servidor vanilla. Devolve a pasta do modelo.
///
/// Erros: `bisect.EULA_NEEDED` sem modelo e sem o aceite da EULA; `bisect.TEST_WORLD_FAILED`
/// (com o erro do servidor nos detalhes técnicos) quando a geração falha; cancelamento.
pub(crate) async fn ensure(
    state: &AppState,
    pack_id: PackId,
    pack_root: &Path,
    minecraft: &str,
    java: &JavaRuntime,
    handle: &OperationHandle,
) -> Result<PathBuf, AppError> {
    let dirs = WorldDirs::of(&state.paths, pack_id);
    if dirs.ready(minecraft) {
        return Ok(dirs.world());
    }
    if crate::server_session::eula_accepted_at(state).is_none() {
        return Err(AppError::from_domain(&warden_bisect::BisectError::EulaNeeded));
    }
    let result = generate(state, pack_id, pack_root, minecraft, java, handle, &dirs).await;
    // A pasta do servidor nunca fica, dê certo ou não.
    let server_root = dirs.server_root();
    let cleanup = blocking(move || {
        warden_instance::subset::remove_dir(&server_root)
            .map_err(|error| AppError::from_domain(&error))
    })
    .await;
    if let Err(error) = cleanup {
        tracing::warn!(code = %error.code, "pasta do servidor do mundo de teste ficou");
    }
    match result {
        Ok(()) => Ok(dirs.world()),
        Err(error) if error.is_cancelled() => Err(error),
        Err(error) => Err(failed(minecraft, &error)),
    }
}

/// O erro da busca para uma geração que falhou, com o erro do servidor nos detalhes técnicos.
fn failed(minecraft: &str, cause: &AppError) -> AppError {
    let mut detail = cause.code.to_string();
    for (name, value) in &cause.params {
        let _ = write!(detail, " {name}={value}");
    }
    if let Some(more) = &cause.detail {
        detail.push_str(": ");
        detail.push_str(more);
    }
    AppError::from_domain(&warden_bisect::BisectError::TestWorldFailed {
        minecraft: minecraft.to_owned(),
    })
    .with_detail(detail)
}

/// Instala o servidor vanilla, gera o mundo e o guarda como modelo.
async fn generate(
    state: &AppState,
    pack_id: PackId,
    pack_root: &Path,
    minecraft: &str,
    java: &JavaRuntime,
    handle: &OperationHandle,
    dirs: &WorldDirs,
) -> Result<(), AppError> {
    handle.ensure_not_cancelled()?;
    let spec = GameSpec::new(minecraft, LoaderSpec::Vanilla)
        .map_err(|error| AppError::from_domain(&error))?;
    {
        // Sobras de uma geração interrompida (o app fechou no meio) e o modelo velho saem.
        let dirs = dirs.clone();
        blocking(move || {
            for dir in [dirs.server_root(), dirs.template.clone()] {
                warden_instance::subset::remove_dir(&dir)
                    .map_err(|error| AppError::from_domain(&error))?;
            }
            std::fs::create_dir_all(&dirs.template).map_err(|error| {
                AppError::internal(format!("criar a pasta do mundo de teste: {error}"))
            })
        })
        .await?;
    }
    let installed =
        crate::server_session::install(state, pack_root, &spec, java, &dirs.server, handle).await?;
    let server_dir = dirs.server.game_dir.clone();
    {
        let server_dir = server_dir.clone();
        blocking(move || {
            atomic_write(
                &server_dir.join(warden_server::properties::PROPERTIES_FILE),
                generation_properties().as_bytes(),
            )
            .map_err(AppError::from)
        })
        .await?;
    }
    let port = crate::server_session::configure(state, &server_dir, MOTD)?;
    let guard = state.servers.begin(pack_id);
    let result = run_server(state, &installed, java, port, handle, dirs).await;
    // Em qualquer saída, o servidor morre antes de o registro sair.
    guard.finish().await;
    result?;
    let dirs = dirs.clone();
    let minecraft = minecraft.to_owned();
    blocking(move || keep_world(&dirs, &server_dir, &minecraft)).await
}

/// Abre o servidor, espera o "pronto" e o para com `stop` (o servidor salva o mundo).
async fn run_server(
    state: &AppState,
    installed: &warden_server::InstalledServer,
    java: &JavaRuntime,
    port: u16,
    handle: &OperationHandle,
    dirs: &WorldDirs,
) -> Result<(), AppError> {
    let command = ServerCommand {
        java: java.clone(),
        memory_mb: warden_server::server_memory_mb(0),
        extra_jvm_args: Vec::new(),
    }
    .build(installed);
    let (running, mut events) = state
        .servers
        .spawn(&command, dirs.template.join(SERVER_LOG), port)
        .await?;
    let server = running.handle();
    tracing::info!(pid = running.pid(), port, "servidor do mundo de teste aberto");
    let ready = tokio::select! {
        ready = warden_server::wait_ready(&mut events, READY_LIMIT, |_| {}) => ready,
        () = handle.token().cancelled() => {
            server.kill().await;
            let _ = running.wait().await;
            return Err(AppError::cancelled());
        }
    };
    if let Err(failure) = ready {
        if matches!(failure, ReadyFailure::Timeout) {
            server.kill().await;
        }
        let _ = running.wait().await;
        return Err(AppError::from_domain(&failure.error(READY_LIMIT)));
    }
    state.servers.update(|current| current.phase = ServerPhase::Stopping);
    let stopped = tokio::select! {
        stopped = server.stop() => stopped,
        () = handle.token().cancelled() => {
            server.kill().await;
            let _ = running.wait().await;
            return Err(AppError::cancelled());
        }
    };
    if stopped.forced {
        tracing::warn!("o servidor do mundo de teste não fechou com stop; o mundo pode ter ficado pela metade");
    }
    running
        .wait()
        .await
        .map_err(|error| AppError::from_domain(&error))?;
    Ok(())
}

/// Move o mundo salvo pelo servidor para o modelo e grava o marcador (por último).
fn keep_world(dirs: &WorldDirs, server_dir: &Path, minecraft: &str) -> Result<(), AppError> {
    let generated = server_dir.join(LEVEL_NAME);
    if !generated.join(LEVEL_DAT).is_file() {
        return Err(AppError::internal(
            "o servidor fechou sem salvar o mundo (bisect-world-server/server/world/level.dat)",
        ));
    }
    move_dir(&generated, &dirs.world())?;
    // O `session.lock` é do servidor que gerou o mundo; cada rodada cria o seu.
    match std::fs::remove_file(dirs.world().join(SESSION_LOCK)) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(AppError::internal(format!(
                "apagar o {SESSION_LOCK} do mundo de teste: {error}"
            )));
        }
    }
    let marker = serde_json::to_vec_pretty(&Marker {
        minecraft: minecraft.to_owned(),
        seed: WORLD_SEED,
    })
    .map_err(|error| AppError::internal(format!("gravar o {MARKER_FILE}: {error}")))?;
    atomic_write(&dirs.marker(), &marker)?;
    Ok(())
}

/// Renomeia a pasta (mesmo disco: `instances/<pack>/`), tentando de novo enquanto o Windows a
/// segura.
fn move_dir(from: &Path, to: &Path) -> Result<(), AppError> {
    let mut attempt = 0;
    loop {
        attempt += 1;
        match std::fs::rename(from, to) {
            Ok(()) => return Ok(()),
            Err(error) if attempt < MOVE_ATTEMPTS => {
                tracing::debug!(%error, attempt, "mundo de teste ainda preso; tentando de novo");
                std::thread::sleep(Duration::from_millis(100 * u64::from(attempt)));
            }
            Err(error) => {
                return Err(AppError::internal(format!(
                    "guardar o mundo de teste (bisect-world-server/server/world → \
                     state/bisect-world/world): {error}"
                )));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn propriedades_da_geracao_tem_a_semente_fixa() {
        let text = generation_properties();
        assert!(text.lines().any(|line| line == "level-seed=20261004"));
        assert!(text.lines().any(|line| line == "level-name=world"));
        // As chaves de segurança da warden-server entram por cima sem perder a semente.
        let rendered = warden_server::properties::render(
            Some(&text),
            &warden_server::ControlledProperties {
                port: 25_600,
                motd: MOTD.into(),
            },
        );
        assert!(rendered.contains("level-seed=20261004"));
        assert!(rendered.contains("server-ip=127.0.0.1"));
    }

    #[test]
    fn modelo_so_vale_com_a_mesma_versao_e_semente() {
        let dir = tempfile::tempdir().unwrap();
        let paths = AppPaths::from_dev_root(dir.path()).unwrap();
        let dirs = WorldDirs::of(&paths, PackId::new());
        assert!(!dirs.ready("1.20.1"));
        std::fs::create_dir_all(dirs.world()).unwrap();
        std::fs::write(dirs.world().join(LEVEL_DAT), "nbt").unwrap();
        // Sem marcador (geração interrompida antes do fim), não vale.
        assert!(!dirs.ready("1.20.1"));
        let marker = |minecraft: &str, seed: i64| {
            serde_json::to_vec(&Marker {
                minecraft: minecraft.into(),
                seed,
            })
            .unwrap()
        };
        std::fs::write(dirs.marker(), marker("1.20.1", WORLD_SEED)).unwrap();
        assert!(dirs.ready("1.20.1"));
        assert!(!dirs.ready("1.21.1"), "outra versão do Minecraft");
        std::fs::write(dirs.marker(), marker("1.20.1", 1)).unwrap();
        assert!(!dirs.ready("1.20.1"), "outra semente");
        std::fs::write(dirs.marker(), "{").unwrap();
        assert!(!dirs.ready("1.20.1"));
        // O modelo fica no estado da instância de teste, longe do pack e dos mundos.
        let test = InstanceDirs::for_pack(&paths, PackId::new());
        assert!(!dirs.world().starts_with(test.game_dir.join("saves")));
    }

    #[test]
    fn falha_leva_o_erro_do_servidor_nos_detalhes() {
        let cause = AppError::from_domain(&warden_server::Error::ReadyTimeout { seconds: 900 });
        let error = failed("1.20.1", &cause);
        assert_eq!(error.code.to_string(), "bisect.TEST_WORLD_FAILED");
        assert_eq!(error.params["minecraft"], "1.20.1");
        let detail = error.detail.unwrap();
        assert!(detail.starts_with("server.READY_TIMEOUT"), "{detail}");
        assert!(detail.contains("seconds=900"), "{detail}");
        assert!(error.retryable);
    }
}
