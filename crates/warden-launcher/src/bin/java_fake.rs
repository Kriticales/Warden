//! Executável falso do Java, só para o teste da emenda do `portablemc` (ADR-0062). Não entra
//! no app.
//!
//! Dois modos:
//!
//! - `instalar <pasta>`: faz o papel do Warden. Monta uma versão mínima em `<pasta>` (só o JSON
//!   e o jar do cliente, sem nada a baixar) e roda o instalador do `portablemc` com o Java fixo
//!   (`JvmPolicy::Static`, como o `PortableMcEngine`) apontando para este mesmo executável. O
//!   motor então roda `<este executável> -version`, o mesmo spawn de toda instalação.
//! - `-version` (o modo "Java"): grava em `WARDEN_JAVA_FAKE_REPORT` uma linha `console=<n>`,
//!   com o `GetConsoleWindow()` do processo (0: sem janela de console; no Linux, sempre 0) e
//!   imprime a versão como o Java imprime.

use std::error::Error;
use std::fs;
use std::io::{self, Write as _};
use std::path::Path;
use std::process::ExitCode;

use portablemc::base::{Installer, JvmPolicy};

/// Variável com o arquivo do relatório do modo "Java".
const REPORT_ENV: &str = "WARDEN_JAVA_FAKE_REPORT";

/// Nome da versão mínima.
const VERSION: &str = "warden-java-fake";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let _ = writeln!(io::stderr(), "java-fake: {error}");
            ExitCode::from(99)
        }
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("instalar") => install(Path::new(args.get(1).ok_or("pasta ausente")?)),
        Some("-version") => {
            let report = std::env::var_os(REPORT_ENV).ok_or("relatório ausente")?;
            fs::write(report, format!("console={}\n", console_window()))?;
            writeln!(io::stderr(), "openjdk version \"1.8.0_402\"")?;
            Ok(())
        }
        _ => Err("uso: java-fake instalar <pasta> | -version".into()),
    }
}

fn install(dir: &Path) -> Result<(), Box<dyn Error>> {
    let version_dir = dir.join("versions").join(VERSION);
    fs::create_dir_all(&version_dir)?;
    fs::write(
        version_dir.join(format!("{VERSION}.json")),
        format!(
            r#"{{"id":"{VERSION}","mainClass":"warden.Fake","javaVersion":{{"component":"jre-legacy","majorVersion":8}}}}"#
        ),
    )?;
    fs::write(version_dir.join(format!("{VERSION}.jar")), b"")?;
    // Pastas que o motor espera existir no fim da instalação.
    for sub in ["libraries", "assets"] {
        fs::create_dir_all(dir.join(sub))?;
    }
    let java = std::env::current_exe()?;
    let mut installer = Installer::new(VERSION);
    installer
        .set_main_dir(dir)
        .set_jvm_policy(JvmPolicy::Static(java));
    installer.install(())?;
    Ok(())
}

/// O `GetConsoleWindow()` deste processo, como número.
#[cfg(windows)]
#[allow(unsafe_code)]
fn console_window() -> isize {
    // SAFETY: `GetConsoleWindow` não recebe argumentos nem toca memória do chamador; só devolve
    // a janela do console do processo (ou nula).
    let window = unsafe { windows::Win32::System::Console::GetConsoleWindow() };
    window.0 as isize
}

/// No Linux não há janela de console.
#[cfg(not(windows))]
fn console_window() -> isize {
    0
}
