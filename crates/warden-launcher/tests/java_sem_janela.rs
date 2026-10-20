//! Emenda do `portablemc` (ADR-0062; `third_party/portablemc/WARDEN.md`): o `java.exe` que o
//! motor roda na instalação (`-version`, a cada teste) não abre janela de console no Windows.
//!
//! O teste faz o papel do Warden com o Java falso (`src/bin/java_fake.rs`): o processo que
//! instala roda **sem console** (`DETACHED_PROCESS`), como o Warden, que é um app de janela.
//! Sem a emenda, o Windows dá a cada `java.exe` filho um console novo, com janela, e o Java
//! falso grava um `GetConsoleWindow()` diferente de zero. No Linux o teste só confere que o
//! motor rodou o Java falso (não há janela de console).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;
use std::process::Command;

#[test]
fn java_da_instalacao_roda_sem_janela_de_console() {
    let dir = tempfile::tempdir().unwrap();
    let report = dir.path().join("java-relatorio.txt");
    let mut command = Command::new(PathBuf::from(env!("CARGO_BIN_EXE_java-fake")));
    command
        .arg("instalar")
        .arg(dir.path().join("jogo"))
        .env("WARDEN_JAVA_FAKE_REPORT", &report);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // DETACHED_PROCESS: o instalador sem console, como o Warden.
        command.creation_flags(0x0000_0008);
    }
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "instalação falhou: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = std::fs::read_to_string(&report)
        .unwrap_or_else(|error| panic!("o motor não rodou o Java falso ({error})"));
    assert_eq!(
        text.trim(),
        "console=0",
        "o `java -version` da instalação abriu uma janela de console"
    );
}
