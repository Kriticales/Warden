//! Como o jogo terminou (ARCHITECTURE §7.4): "Encerrado por você" se foi pedido; "Travou" se
//! o código de saída não é 0 ou apareceu um crash report ou um `hs_err_pid` novo; senão
//! "Fechado normalmente".
//!
//! O crash report de mentira do FML 1.7.10 (`SplashProgress`, "THIS IS NOT A ERROR") não
//! conta (S1 §2; S-R5-3 §3.1).

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};

/// Margem para relógios de sistema de arquivos com resolução de 1–2 s.
const MTIME_SLACK: Duration = Duration::from_secs(2);

/// Resultado do teste.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum Outcome {
    /// O usuário pediu para parar (ou o Warden fechou).
    StoppedByUser,
    /// Código diferente de 0, crash report ou `hs_err_pid` novo.
    Crashed,
    /// Saiu com código 0 sem sinais de travamento.
    ClosedNormally,
}

/// Arquivos de travamento criados durante a sessão.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CrashArtifacts {
    /// `crash-reports/crash-*.txt` novos.
    pub crash_reports: Vec<PathBuf>,
    /// `hs_err_pid*.log` novos (queda da JVM).
    pub hs_err_files: Vec<PathBuf>,
}

impl CrashArtifacts {
    /// Se há algum.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.crash_reports.is_empty() && self.hs_err_files.is_empty()
    }
}

/// Classifica a saída.
#[must_use]
pub fn classify(
    stop_requested: bool,
    exit_code: Option<i32>,
    artifacts: &CrashArtifacts,
) -> Outcome {
    if stop_requested {
        Outcome::StoppedByUser
    } else if exit_code != Some(0) || !artifacts.is_empty() {
        Outcome::Crashed
    } else {
        Outcome::ClosedNormally
    }
}

/// Se o arquivo foi modificado depois de `since` (com folga).
fn modified_since(path: &Path, since: SystemTime) -> bool {
    let threshold = since.checked_sub(MTIME_SLACK).unwrap_or(since);
    fs::metadata(path)
        .and_then(|meta| meta.modified())
        .is_ok_and(|modified| modified >= threshold)
}

/// O crash report de mentira do FML 1.7.10.
fn is_splash_debug_report(path: &Path) -> bool {
    fs::read(path).is_ok_and(|bytes| {
        let text = String::from_utf8_lossy(&bytes);
        text.contains("THIS IS NOT A ERROR") || text.contains("Loading screen debug info")
    })
}

/// Arquivos de uma pasta cujo nome satisfaz `wanted`, modificados desde `since`.
fn new_files(dir: &Path, since: SystemTime, wanted: impl Fn(&str) -> bool) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut found: Vec<PathBuf> = entries
        .flatten()
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
        .filter(|entry| entry.file_name().to_str().is_some_and(&wanted))
        .map(|entry| entry.path())
        .filter(|path| modified_since(path, since))
        .collect();
    found.sort();
    found
}

/// Se o nome termina com a extensão (sem diferenciar maiúsculas).
fn has_extension(name: &str, extension: &str) -> bool {
    Path::new(name)
        .extension()
        .is_some_and(|found| found.eq_ignore_ascii_case(extension))
}

/// Procura os arquivos de travamento criados desde `since` no `gameDir`.
#[must_use]
pub fn find_crash_artifacts(game_dir: &Path, since: SystemTime) -> CrashArtifacts {
    let crash_reports = new_files(&game_dir.join("crash-reports"), since, |name| {
        name.starts_with("crash-") && has_extension(name, "txt")
    })
    .into_iter()
    .filter(|path| !is_splash_debug_report(path))
    .collect();
    let hs_err_files = new_files(game_dir, since, |name| {
        name.starts_with("hs_err_pid") && has_extension(name, "log")
    });
    CrashArtifacts {
        crash_reports,
        hs_err_files,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classificacao() {
        let none = CrashArtifacts::default();
        let crash = CrashArtifacts {
            crash_reports: vec![PathBuf::from("crash-reports/crash-1.txt")],
            hs_err_files: Vec::new(),
        };
        assert_eq!(classify(true, Some(1), &crash), Outcome::StoppedByUser);
        assert_eq!(classify(false, Some(0), &none), Outcome::ClosedNormally);
        assert_eq!(classify(false, Some(1), &none), Outcome::Crashed);
        assert_eq!(classify(false, None, &none), Outcome::Crashed);
        assert_eq!(classify(false, Some(0), &crash), Outcome::Crashed);
    }

    #[test]
    fn so_arquivos_novos_e_sem_o_falso_crash_do_1_7_10() {
        let dir = tempfile::tempdir().unwrap();
        let reports = dir.path().join("crash-reports");
        fs::create_dir_all(&reports).unwrap();
        let old = reports.join("crash-2020-01-01_00.00.00-client.txt");
        fs::write(&old, "---- Minecraft Crash Report ----").unwrap();
        let old_file = fs::File::options().write(true).open(&old).unwrap();
        old_file
            .set_modified(SystemTime::now() - Duration::from_secs(3_600))
            .unwrap();
        drop(old_file);

        let start = SystemTime::now();
        let real = reports.join("crash-2026-10-05_10.00.00-client.txt");
        fs::write(
            &real,
            "---- Minecraft Crash Report ----\nDescription: Initializing game",
        )
        .unwrap();
        let splash = reports.join("crash-2026-10-05_10.00.01-client.txt");
        fs::write(
            &splash,
            "---- Minecraft Crash Report ----\n// THIS IS NOT A ERROR\nDescription: Loading screen debug info",
        )
        .unwrap();
        fs::write(reports.join("notas.txt"), "x").unwrap();
        let hs_err = dir.path().join("hs_err_pid4242.log");
        fs::write(&hs_err, "# A fatal error has been detected").unwrap();

        let found = find_crash_artifacts(dir.path(), start);
        assert_eq!(found.crash_reports, vec![real]);
        assert_eq!(found.hs_err_files, vec![hs_err]);
        assert_eq!(classify(false, Some(0), &found), Outcome::Crashed);
    }

    #[test]
    fn pasta_inexistente_nao_e_erro() {
        let found = find_crash_artifacts(Path::new("nao-existe"), SystemTime::now());
        assert!(found.is_empty());
    }
}
