//! Gravação de cada abertura do jogo (ARCHITECTURE §7.4): `instances/<id>/state/sessions/
//! <data-hora>/` com `output.log` (tudo o que saiu dos pipes, gravado pelo
//! [`crate::process`]) e `session.json` (resultado, duração, versão do pack, artefatos de
//! crash).
//!
//! O `session.json` aceita campos de quem abre o jogo (`extra`): a `warden-app` (L-04)
//! acrescenta o hash da árvore do pack, o modo, o perfil, a assinatura do travamento e os
//! tempos sem mudar esta crate. Retenção: as 30 sessões mais novas que não travaram ficam; as
//! que travaram não são apagadas aqui (a interface avisa antes; [`crashed_over_budget`]).

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use warden_core::atomic_write;

use crate::error::{Error, Result};
use crate::outcome::{CrashArtifacts, Outcome};
use crate::process::GameExit;
use crate::spec::GameSpec;

/// Versão do formato do `session.json`.
pub const SESSION_SCHEMA: u32 = 1;

/// Nome do log cru.
pub const OUTPUT_LOG: &str = "output.log";

/// Nome do registro.
pub const SESSION_FILE: &str = "session.json";

/// Sessões que não travaram mantidas por pack.
pub const KEEP_CLOSED_SESSIONS: usize = 30;

/// Orçamento de disco das sessões que travaram, por pack.
pub const CRASHED_BUDGET_BYTES: u64 = 500 * 1024 * 1024;

/// O `session.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionRecord {
    /// Versão do formato.
    pub schema: u32,
    /// Início (RFC 3339, UTC).
    pub started_at: String,
    /// Fim (RFC 3339, UTC).
    pub ended_at: String,
    /// Duração do processo.
    pub duration_ms: u64,
    /// Resultado.
    pub outcome: Outcome,
    /// Código de saída.
    pub exit_code: Option<i32>,
    /// Versão do pack testada (`pack.toml`).
    pub pack_version: Option<String>,
    /// O jogo.
    pub game: GameSpec,
    /// Major do Java.
    pub java_major: u32,
    /// Crash reports e `hs_err_pid`, relativos ao `gameDir` quando possível.
    pub artifacts: CrashArtifacts,
    /// Campos de quem abre o jogo (hash da árvore do pack, modo, perfil…).
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

impl SessionRecord {
    /// O registro de uma sessão encerrada.
    #[must_use]
    pub fn from_exit(
        started_at: SystemTime,
        exit: &GameExit,
        game: &GameSpec,
        java_major: u32,
        pack_version: Option<String>,
        game_dir: &Path,
    ) -> Self {
        let ended = started_at + std::time::Duration::from_millis(exit.duration_ms);
        let relative = |paths: &[PathBuf]| {
            paths
                .iter()
                .map(|path| path.strip_prefix(game_dir).unwrap_or(path).to_path_buf())
                .collect()
        };
        Self {
            schema: SESSION_SCHEMA,
            started_at: rfc3339_utc(started_at),
            ended_at: rfc3339_utc(ended),
            duration_ms: exit.duration_ms,
            outcome: exit.outcome,
            exit_code: exit.exit_code,
            pack_version,
            game: game.clone(),
            java_major,
            artifacts: CrashArtifacts {
                crash_reports: relative(&exit.artifacts.crash_reports),
                hs_err_files: relative(&exit.artifacts.hs_err_files),
            },
            extra: serde_json::Map::new(),
        }
    }
}

/// A pasta de uma sessão.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionDir {
    path: PathBuf,
}

impl SessionDir {
    /// Cria a pasta `<sessions>/<AAAA-MM-DD_HH-MM-SS>` (UTC), com sufixo `-2`, `-3`… se já
    /// existir outra no mesmo segundo.
    ///
    /// Erros: disco.
    pub fn create(sessions: &Path, started_at: SystemTime) -> Result<Self> {
        fs::create_dir_all(sessions)
            .map_err(|error| Error::io("criar a pasta das sessões", sessions, error))?;
        let base = folder_name(started_at);
        for attempt in 1..1_000 {
            let name = if attempt == 1 {
                base.clone()
            } else {
                format!("{base}-{attempt}")
            };
            let path = sessions.join(name);
            match fs::create_dir(&path) {
                Ok(()) => return Ok(Self { path }),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(Error::io("criar a pasta da sessão", &path, error)),
            }
        }
        Err(Error::internal("mais de mil sessões no mesmo segundo"))
    }

    /// Uma pasta existente.
    #[must_use]
    pub fn open(path: PathBuf) -> Self {
        Self { path }
    }

    /// A pasta.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// O `output.log`.
    #[must_use]
    pub fn output_log(&self) -> PathBuf {
        self.path.join(OUTPUT_LOG)
    }

    /// Grava o `session.json` (escrita atômica).
    ///
    /// Erros: disco.
    pub fn write_record(&self, record: &SessionRecord) -> Result<()> {
        let json = serde_json::to_vec_pretty(record)
            .map_err(|error| Error::internal(format!("session.json: {error}")))?;
        atomic_write(&self.path.join(SESSION_FILE), &json)?;
        Ok(())
    }

    /// Lê o `session.json`.
    ///
    /// Erros: disco; JSON inválido vira [`Error::Internal`].
    pub fn read_record(&self) -> Result<SessionRecord> {
        let path = self.path.join(SESSION_FILE);
        let bytes =
            fs::read(&path).map_err(|error| Error::io("ler o session.json", &path, error))?;
        serde_json::from_slice(&bytes)
            .map_err(|error| Error::internal(format!("{}: {error}", path.display())))
    }
}

/// Uma sessão gravada.
#[derive(Debug, Clone, PartialEq)]
pub struct StoredSession {
    /// A pasta.
    pub dir: SessionDir,
    /// O registro (`None` se a sessão não terminou de ser gravada ou o arquivo estragou).
    pub record: Option<SessionRecord>,
}

/// As sessões de um pack, da mais nova para a mais antiga (pelo nome da pasta).
///
/// Erros: disco (pasta ausente devolve lista vazia).
pub fn list_sessions(sessions: &Path) -> Result<Vec<StoredSession>> {
    let entries = match fs::read_dir(sessions) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(Error::io("listar as sessões", sessions, error)),
    };
    let mut dirs: Vec<PathBuf> = entries
        .flatten()
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .map(|entry| entry.path())
        .collect();
    dirs.sort();
    dirs.reverse();
    Ok(dirs
        .into_iter()
        .map(|path| {
            let dir = SessionDir::open(path);
            let record = dir.read_record().ok();
            StoredSession { dir, record }
        })
        .collect())
}

/// Apaga as sessões que não travaram além das `keep` mais novas. Devolve as apagadas.
/// Sessões sem `session.json` (abertas agora ou interrompidas) não são tocadas.
///
/// Erros: disco.
pub fn prune_closed(sessions: &Path, keep: usize) -> Result<Vec<PathBuf>> {
    let mut removed = Vec::new();
    let closed = list_sessions(sessions)?
        .into_iter()
        .filter(|session| {
            session
                .record
                .as_ref()
                .is_some_and(|record| record.outcome != Outcome::Crashed)
        })
        .skip(keep);
    for session in closed {
        let path = session.dir.path().to_path_buf();
        fs::remove_dir_all(&path).map_err(|error| Error::io("apagar a sessão", &path, error))?;
        removed.push(path);
    }
    Ok(removed)
}

/// Tamanho de uma pasta (recursivo; o que não der para ler conta 0).
fn dir_size(path: &Path) -> u64 {
    let Ok(entries) = fs::read_dir(path) else {
        return 0;
    };
    entries
        .flatten()
        .map(|entry| match entry.file_type() {
            Ok(kind) if kind.is_dir() => dir_size(&entry.path()),
            Ok(_) => entry.metadata().map_or(0, |meta| meta.len()),
            Err(_) => 0,
        })
        .sum()
}

/// As sessões que travaram que passam do orçamento, das mais antigas para as mais novas: a
/// interface avisa e apaga essas depois de confirmar.
///
/// Erros: disco.
pub fn crashed_over_budget(sessions: &Path, budget_bytes: u64) -> Result<Vec<PathBuf>> {
    let mut total = 0u64;
    let mut over = Vec::new();
    for session in list_sessions(sessions)? {
        let crashed = session
            .record
            .as_ref()
            .is_some_and(|record| record.outcome == Outcome::Crashed);
        if !crashed {
            continue;
        }
        total = total.saturating_add(dir_size(session.dir.path()));
        if total > budget_bytes {
            over.push(session.dir.path().to_path_buf());
        }
    }
    over.reverse();
    Ok(over)
}

/// Data e hora UTC de um instante.
fn utc_parts(time: SystemTime) -> (i64, i64, i64, i64, i64, i64) {
    let seconds = time.duration_since(UNIX_EPOCH).map_or(0, |elapsed| {
        i64::try_from(elapsed.as_secs()).unwrap_or(i64::MAX)
    });
    let days = seconds.div_euclid(86_400);
    let second_of_day = seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    (
        year,
        month,
        day,
        second_of_day / 3600,
        second_of_day % 3600 / 60,
        second_of_day % 60,
    )
}

/// `2026-10-05T14:03:22Z`.
#[must_use]
pub fn rfc3339_utc(time: SystemTime) -> String {
    let (year, month, day, hour, minute, second) = utc_parts(time);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

/// `2026-10-05_14-03-22` (UTC; ordena pelo nome).
fn folder_name(time: SystemTime) -> String {
    let (year, month, day, hour, minute, second) = utc_parts(time);
    format!("{year:04}-{month:02}-{day:02}_{hour:02}-{minute:02}-{second:02}")
}

/// Dias desde 1970-01-01 → (ano, mês, dia), algoritmo `civil_from_days` de Howard Hinnant
/// (domínio público), o mesmo da `warden-versioning`.
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::spec::LoaderSpec;

    fn at(seconds: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(seconds)
    }

    fn spec() -> GameSpec {
        GameSpec::new(
            "1.20.1",
            LoaderSpec::Fabric {
                version: "0.19.5".into(),
            },
        )
        .unwrap()
    }

    fn exit(outcome: Outcome) -> GameExit {
        GameExit {
            outcome,
            exit_code: Some(i32::from(outcome != Outcome::ClosedNormally)),
            stop_requested: outcome == Outcome::StoppedByUser,
            duration_ms: 61_500,
            artifacts: CrashArtifacts::default(),
        }
    }

    #[test]
    fn datas_utc() {
        assert_eq!(rfc3339_utc(at(0)), "1970-01-01T00:00:00Z");
        assert_eq!(rfc3339_utc(at(1_790_874_309)), "2026-10-01T17:05:09Z");
        assert_eq!(folder_name(at(1_790_874_309)), "2026-10-01_17-05-09");
        // 2024-02-29T12:00:00Z
        assert_eq!(rfc3339_utc(at(1_709_208_000)), "2024-02-29T12:00:00Z");
    }

    #[test]
    fn grava_e_le_o_registro_com_campos_extras() {
        let root = tempfile::tempdir().unwrap();
        let started = at(1_790_874_309);
        let dir = SessionDir::create(root.path(), started).unwrap();
        let second = SessionDir::create(root.path(), started).unwrap();
        assert!(second.path().ends_with("2026-10-01_17-05-09-2"));
        let game_dir = PathBuf::from("instancia");
        let mut crashed = exit(Outcome::Crashed);
        crashed.artifacts.crash_reports = vec![game_dir.join("crash-reports").join("crash-1.txt")];
        let mut record = SessionRecord::from_exit(
            started,
            &crashed,
            &spec(),
            17,
            Some("1.3.0".into()),
            &game_dir,
        );
        record
            .extra
            .insert("mode".into(), serde_json::Value::String("normal".into()));
        dir.write_record(&record).unwrap();
        let read = dir.read_record().unwrap();
        assert_eq!(read, record);
        assert_eq!(read.ended_at, "2026-10-01T17:06:10Z");
        assert_eq!(
            read.artifacts.crash_reports,
            vec![PathBuf::from("crash-reports").join("crash-1.txt")]
        );
        let json: serde_json::Value =
            serde_json::from_slice(&fs::read(dir.path().join(SESSION_FILE)).unwrap()).unwrap();
        assert_eq!(json["mode"], "normal");
        assert_eq!(json["outcome"], "crashed");
        assert_eq!(json["game"]["loader"]["kind"], "fabric");
    }

    #[test]
    fn retencao_mantem_as_mais_novas_e_nao_toca_nas_que_travaram() {
        let root = tempfile::tempdir().unwrap();
        for index in 0..5u64 {
            let started = at(1_790_000_000 + index * 60);
            let dir = SessionDir::create(root.path(), started).unwrap();
            let outcome = if index == 1 {
                Outcome::Crashed
            } else {
                Outcome::ClosedNormally
            };
            let record = SessionRecord::from_exit(
                started,
                &exit(outcome),
                &spec(),
                17,
                None,
                Path::new("x"),
            );
            dir.write_record(&record).unwrap();
            fs::write(dir.output_log(), vec![b'x'; 1_000]).unwrap();
        }
        // Uma sessão em andamento (sem session.json) nunca é apagada.
        SessionDir::create(root.path(), at(1_700_000_000)).unwrap();
        let removed = prune_closed(root.path(), 2).unwrap();
        assert_eq!(removed.len(), 2);
        let left = list_sessions(root.path()).unwrap();
        assert_eq!(left.len(), 4);
        assert!(left.iter().any(|session| {
            session
                .record
                .as_ref()
                .is_some_and(|r| r.outcome == Outcome::Crashed)
        }));
        assert!(left.iter().any(|session| session.record.is_none()));

        assert!(crashed_over_budget(root.path(), 10_000).unwrap().is_empty());
        assert_eq!(crashed_over_budget(root.path(), 10).unwrap().len(), 1);
    }

    #[test]
    fn pasta_de_sessoes_ausente() {
        assert!(list_sessions(Path::new("nao-existe")).unwrap().is_empty());
    }
}
