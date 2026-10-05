//! Coleta dos artefatos de uma sessão de teste (ARCHITECTURE §9.3 item 1).
//!
//! Lê só o que pertence à sessão: o `output.log` da pasta da sessão e, na pasta do jogo, os
//! arquivos gravados depois do início (um `latest.log` de uma sessão anterior não serve quando
//! a JVM nem chegou a abrir o log4j). Arquivos que somem entre a listagem e a leitura (o
//! antivírus cria e apaga temporários) são pulados; outras falhas de leitura viram erro.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use super::{Analysis, AnalysisContext, LogSource, SessionOutcome, SourceKind, analyze};
use crate::error::{DiagnosticsError, Result};

/// Folga para relógios de arquivo de baixa resolução: arquivos gravados até este tempo antes do
/// início ainda contam.
pub const MTIME_TOLERANCE: Duration = Duration::from_secs(2);

/// Onde ficam os arquivos de uma sessão.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionFiles {
    /// `instances/<pack-id>/state/sessions/<data-hora>/`, com o `output.log`. `None` quando
    /// não há captura do processo (análise de uma pasta de jogo qualquer).
    pub session_dir: Option<PathBuf>,
    /// A pasta do jogo (`gameDir`), com `logs/`, `crash-reports/` e os `hs_err_pid*.log`.
    pub game_dir: PathBuf,
    /// Início da sessão (lançamento do processo).
    pub started_at: SystemTime,
}

fn is_recent(path: &Path, since: SystemTime) -> io::Result<bool> {
    let modified = fs::metadata(path)?.modified()?;
    let limit = since.checked_sub(MTIME_TOLERANCE).unwrap_or(since);
    Ok(modified >= limit)
}

/// Lê o arquivo se ele existir e for da sessão; `None` se não existir (ou sumir no meio).
fn read_if_present(
    path: &Path,
    name: String,
    kind: SourceKind,
    since: Option<SystemTime>,
) -> Result<Option<LogSource>> {
    let recent = match since {
        None => path.is_file(),
        Some(since) => match is_recent(path, since) {
            Ok(recent) => recent && path.is_file(),
            Err(error) if error.kind() == io::ErrorKind::NotFound => false,
            Err(error) => return Err(DiagnosticsError::io(path, error)),
        },
    };
    if !recent {
        return Ok(None);
    }
    match LogSource::read(path, name, kind) {
        Ok(source) => Ok(Some(source)),
        Err(DiagnosticsError::Io { source, .. }) if source.kind() == io::ErrorKind::NotFound => {
            Ok(None)
        }
        Err(error) => Err(error),
    }
}

fn has_extension(name: &str, extension: &str) -> bool {
    Path::new(name)
        .extension()
        .is_some_and(|found| found.eq_ignore_ascii_case(extension))
}

/// Arquivos de uma pasta que passam no filtro, em ordem de nome.
fn list(dir: &Path, keep: impl Fn(&str) -> bool) -> Result<Vec<(PathBuf, String)>> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(DiagnosticsError::io(dir, error)),
    };
    let mut found = Vec::new();
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(DiagnosticsError::io(dir, error)),
        };
        let name = entry.file_name().to_string_lossy().into_owned();
        if keep(&name) {
            found.push((entry.path(), name));
        }
    }
    found.sort_by(|a, b| a.1.cmp(&b.1));
    Ok(found)
}

/// Junta os arquivos da sessão, na ordem: `output.log`, `logs/latest.log`, `logs/debug.log` (ou
/// `logs/fml-client-latest.log`), `crash-reports/*.txt` e `hs_err_pid*.log` recentes.
///
/// Os nomes das fontes são relativos (`logs/latest.log`, `crash-reports/crash-….txt`), nunca o
/// caminho completo, que tem o nome do usuário.
pub fn collect_session(files: &SessionFiles) -> Result<Vec<LogSource>> {
    if !files.game_dir.is_dir() {
        return Err(DiagnosticsError::SessionNotFound {
            path: files.game_dir.clone(),
        });
    }
    let since = Some(files.started_at);
    let mut sources = Vec::new();
    if let Some(session_dir) = &files.session_dir {
        if !session_dir.is_dir() {
            return Err(DiagnosticsError::SessionNotFound {
                path: session_dir.clone(),
            });
        }
        sources.extend(read_if_present(
            &session_dir.join("output.log"),
            "output.log".to_owned(),
            SourceKind::Output,
            None,
        )?);
    }
    let logs = files.game_dir.join("logs");
    sources.extend(read_if_present(
        &logs.join("latest.log"),
        "logs/latest.log".to_owned(),
        SourceKind::LatestLog,
        since,
    )?);
    let debug = read_if_present(
        &logs.join("debug.log"),
        "logs/debug.log".to_owned(),
        SourceKind::DebugLog,
        since,
    )?;
    match debug {
        Some(debug) => sources.push(debug),
        None => sources.extend(read_if_present(
            &logs.join("fml-client-latest.log"),
            "logs/fml-client-latest.log".to_owned(),
            SourceKind::DebugLog,
            since,
        )?),
    }
    for (path, name) in list(&files.game_dir.join("crash-reports"), |name| {
        has_extension(name, "txt")
    })? {
        sources.extend(read_if_present(
            &path,
            format!("crash-reports/{name}"),
            SourceKind::CrashReport,
            since,
        )?);
    }
    for (path, name) in list(&files.game_dir, |name| {
        name.to_ascii_lowercase().starts_with("hs_err_pid") && has_extension(name, "log")
    })? {
        sources.extend(read_if_present(&path, name, SourceKind::JvmCrash, since)?);
    }
    Ok(sources)
}

/// Coleta e analisa uma sessão.
pub fn analyze_session(
    files: &SessionFiles,
    context: &AnalysisContext,
    outcome: Option<&SessionOutcome>,
) -> Result<Analysis> {
    let sources = collect_session(files)?;
    analyze(&sources, context, outcome)
}

#[cfg(test)]
mod tests {
    use std::fs::File;

    use super::*;

    fn write(path: &Path, text: &str, modified: SystemTime) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
        File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(modified)
            .unwrap();
    }

    #[test]
    fn coleta_so_os_arquivos_da_sessao() {
        let dir = tempfile::tempdir().unwrap();
        let game = dir.path().join("minecraft");
        let session = dir.path().join("sessions").join("2026-10-04_19-00-00");
        let start = SystemTime::now() - Duration::from_secs(60);
        let old = start - Duration::from_secs(3600);
        let new = start + Duration::from_secs(10);

        write(&session.join("output.log"), "saída\n", old);
        write(&game.join("logs/latest.log"), "latest\n", new);
        write(&game.join("logs/debug.log"), "debug velho\n", old);
        write(&game.join("logs/fml-client-latest.log"), "fml\n", new);
        write(
            &game.join("crash-reports/crash-2026-10-04_19.01.00-client.txt"),
            "novo\n",
            new,
        );
        write(
            &game.join("crash-reports/crash-2026-10-03_10.00.00-client.txt"),
            "velho\n",
            old,
        );
        write(&game.join("crash-reports/nota.md"), "não é crash\n", new);
        write(
            &game.join("hs_err_pid1234.log"),
            "# C  [ig9icd64.dll+0x1]\n",
            new,
        );
        write(&game.join("hs_err_pid1.log"), "velho\n", old);

        let files = SessionFiles {
            session_dir: Some(session),
            game_dir: game,
            started_at: start,
        };
        let sources = collect_session(&files).unwrap();
        let names: Vec<_> = sources.iter().map(LogSource::name).collect();
        assert_eq!(
            names,
            [
                "output.log",
                "logs/latest.log",
                "logs/fml-client-latest.log",
                "crash-reports/crash-2026-10-04_19.01.00-client.txt",
                "hs_err_pid1234.log",
            ]
        );
        let analysis = analyze_session(&files, &AnalysisContext::default(), None).unwrap();
        assert_eq!(analysis.sources.len(), 5);
        assert_eq!(
            analysis.findings[0].finding.rule().as_str(),
            "E_GRAPHICS_DRIVER"
        );
    }

    #[test]
    fn pasta_inexistente_e_erro_claro() {
        let dir = tempfile::tempdir().unwrap();
        let files = SessionFiles {
            session_dir: None,
            game_dir: dir.path().join("nao-existe"),
            started_at: SystemTime::now(),
        };
        assert!(matches!(
            collect_session(&files),
            Err(DiagnosticsError::SessionNotFound { .. })
        ));
        let files = SessionFiles {
            session_dir: Some(dir.path().join("sessao-podada")),
            game_dir: dir.path().to_path_buf(),
            started_at: SystemTime::now(),
        };
        assert!(matches!(
            collect_session(&files),
            Err(DiagnosticsError::SessionNotFound { .. })
        ));
        // Pasta do jogo vazia: nada a analisar, sem erro.
        let files = SessionFiles {
            session_dir: None,
            game_dir: dir.path().to_path_buf(),
            started_at: SystemTime::now(),
        };
        assert!(collect_session(&files).unwrap().is_empty());
    }

    #[test]
    fn arquivo_ausente_e_pulado() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("x.log");
        assert!(
            read_if_present(&missing, "x".into(), SourceKind::Text, None)
                .unwrap()
                .is_none()
        );
        assert!(
            read_if_present(
                &missing,
                "x".into(),
                SourceKind::Text,
                Some(SystemTime::now())
            )
            .unwrap()
            .is_none()
        );
        assert!(list(&missing, |_| true).unwrap().is_empty());
        // Uma pasta no lugar do arquivo não é lida.
        assert!(
            read_if_present(dir.path(), "d".into(), SourceKind::Text, None)
                .unwrap()
                .is_none()
        );
    }
}
