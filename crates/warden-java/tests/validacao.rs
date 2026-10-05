//! Validação de um Java: o leitor contra saídas reais de `java -XshowSettings:properties
//! -version` (`tests/fixtures/probe/`, gravadas no Windows com os Temurin 8, 17, 21 e 25 e
//! anonimizadas: caminhos trocados por `C:\exemplo`, usuário por `usuario`, PATH removido) e o
//! [`ProcessProbe`] executando um programa de verdade que imita o Java.

// Nos auxiliares dos testes, `unwrap`, `expect` e `panic!` são a forma idiomática de falhar
// (o `clippy.toml` só os libera dentro das funções `#[test]`).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod comum;

use std::path::{Path, PathBuf};
use std::time::Duration;

use warden_java::probe::{info_from_properties, parse_properties};
use warden_java::{Error, JavaProbe, JavaVersion, ProcessProbe};

fn probe_fixture(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("probe")
        .join(name);
    std::fs::read_to_string(path).unwrap()
}

#[test]
fn saidas_reais_do_temurin() {
    let cases = [
        (
            "temurin-8-windows-x64.txt",
            JavaVersion::new(8, 0, 504, 0, 1),
            "Temurin",
        ),
        (
            "temurin-17-windows-x64.txt",
            JavaVersion::new(17, 0, 20, 1, 1),
            "Eclipse Adoptium",
        ),
        (
            "temurin-21-windows-x64.txt",
            JavaVersion::new(21, 0, 12, 1, 1),
            "Eclipse Adoptium",
        ),
        (
            "temurin-25-windows-x64.txt",
            JavaVersion::new(25, 0, 4, 1, 1),
            "Eclipse Adoptium",
        ),
    ];
    for (name, version, vendor) in cases {
        let text = probe_fixture(name);
        let properties = parse_properties(&text);
        // Nenhuma linha de continuação (`java.library.path`, `sun.boot.class.path`) virou chave.
        assert!(properties.keys().all(|key| !key.contains('\\')), "{name}");
        assert_eq!(properties["os.arch"], "amd64", "{name}");
        let info = info_from_properties(&properties).unwrap();
        assert_eq!(info.version, version, "{name}");
        assert_eq!(info.vendor, vendor, "{name}");
        assert!(info.is_64bit, "{name}");
        assert_eq!(info.vm_name, "OpenJDK 64-Bit Server VM", "{name}");
        // Também com CRLF (a saída crua do Windows).
        let crlf = text.replace('\n', "\r\n");
        assert_eq!(
            info_from_properties(&parse_properties(&crlf)).unwrap(),
            info,
            "{name}"
        );
    }
}

/// Cria um programa que imita o Java: imprime `stderr_text` na saída de erro e sai com `code`.
/// No Windows é um `.cmd` (o Rust o executa pelo `cmd.exe`, com os argumentos em vetor); no
/// Linux, um script `sh`.
fn fake_java(dir: &Path, name: &str, stderr_text: &str, code: i32) -> PathBuf {
    let data = dir.join(format!("{name}.txt"));
    std::fs::write(&data, stderr_text).unwrap();
    if cfg!(windows) {
        let script = dir.join(format!("{name}.cmd"));
        std::fs::write(
            &script,
            format!(
                "@echo off\r\ntype \"{}\" 1>&2\r\nexit /b {code}\r\n",
                data.display()
            ),
        )
        .unwrap();
        script
    } else {
        let script = dir.join(name);
        std::fs::write(
            &script,
            format!("#!/bin/sh\ncat '{}' 1>&2\nexit {code}\n", data.display()),
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        script
    }
}

#[tokio::test]
async fn processo_que_imita_o_java_e_validado() {
    let dir = tempfile::tempdir().unwrap();
    let java = fake_java(
        dir.path(),
        "java21",
        &probe_fixture("temurin-21-windows-x64.txt"),
        0,
    );
    let info = ProcessProbe::default().probe(&java).await.unwrap();
    assert_eq!(info.version, JavaVersion::new(21, 0, 12, 1, 1));
    assert_eq!(info.vendor, "Eclipse Adoptium");
}

#[tokio::test]
async fn java_de_32_bits_e_recusado() {
    let dir = tempfile::tempdir().unwrap();
    let text = probe_fixture("temurin-8-windows-x64.txt")
        .replace("sun.arch.data.model = 64", "sun.arch.data.model = 32")
        .replace("os.arch = amd64", "os.arch = x86");
    let java = fake_java(dir.path(), "java8x86", &text, 0);
    let error = ProcessProbe::default().probe(&java).await.unwrap_err();
    assert!(
        matches!(error, Error::Not64Bit { ref arch, .. } if arch == "x86"),
        "{error:?}"
    );
}

#[tokio::test]
async fn java_que_falha_traz_a_saida_nos_detalhes() {
    let dir = tempfile::tempdir().unwrap();
    let java = fake_java(
        dir.path(),
        "quebrado",
        "Error: Could not create the Java Virtual Machine.\nError: A fatal exception has occurred. Program will exit.\n",
        1,
    );
    let error = ProcessProbe::default().probe(&java).await.unwrap_err();
    let Error::Validation { output, .. } = &error else {
        panic!("{error:?}");
    };
    assert!(output.as_deref().unwrap().contains("Could not create"));
    let detail = warden_core::DomainError::detail(&error).unwrap();
    assert!(detail.contains("saída do Java"), "{detail}");
}

#[tokio::test]
async fn saida_sem_propriedades_e_recusada() {
    let dir = tempfile::tempdir().unwrap();
    let java = fake_java(dir.path(), "estranho", "nada aqui\n", 0);
    let error = ProcessProbe::default().probe(&java).await.unwrap_err();
    assert!(matches!(error, Error::Validation { .. }), "{error:?}");
}

#[tokio::test]
async fn java_que_nao_responde_e_encerrado_no_tempo_limite() {
    let dir = tempfile::tempdir().unwrap();
    let java = if cfg!(windows) {
        let script = dir.path().join("trava.cmd");
        // Laço do próprio `cmd.exe`, sem processo filho: encerrar o `cmd` fecha os pipes (um
        // filho como o `ping` seguraria o pipe e o teste esperaria por ele).
        std::fs::write(&script, "@echo off\r\n:laco\r\ngoto laco\r\n").unwrap();
        script
    } else {
        let script = dir.path().join("trava");
        std::fs::write(&script, "#!/bin/sh\nwhile :; do :; done\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        script
    };
    let started = std::time::Instant::now();
    let error = ProcessProbe::with_timeout(Duration::from_millis(500))
        .probe(&java)
        .await
        .unwrap_err();
    assert!(started.elapsed() < Duration::from_secs(20));
    let Error::Validation { message, .. } = &error else {
        panic!("{error:?}");
    };
    assert!(message.contains("não respondeu"), "{message}");
}
