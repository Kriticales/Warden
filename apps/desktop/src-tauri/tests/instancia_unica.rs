//! Critério 6 da F0-05 com o app de verdade (ROADMAP F0-05; ADR-0048).
//!
//! Abre o executável compilado da `warden-app` (build de debug) com `WARDEN_DATA_ROOT` em
//! pastas temporárias e confere:
//! - uma segunda cópia com a mesma pasta foca a primeira e sai;
//! - com outra pasta, as duas ficam abertas;
//! - nada é criado nas pastas do Warden instalado (só se confere a lista de entradas de
//!   `%APPDATA%` e `%LOCALAPPDATA%` cujo nome tem "kriticales", sem ler o conteúdo delas).
//!
//! Abre janelas de verdade; por isso fica `#[ignore = "app-real"]` e roda sob demanda:
//! `cargo nextest run -p warden-app --run-ignored only -E 'test(instancia_unica)'`.

// Arquivo só de teste: as funções auxiliares (fora de `#[test]`) também podem falhar com
// pânico, como os testes (`clippy.toml` só libera dentro das funções `#[test]`).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant, SystemTime};

const READY_TIMEOUT: Duration = Duration::from_secs(90);
const EXIT_TIMEOUT: Duration = Duration::from_secs(30);

/// Processo do app que é encerrado ao sair de escopo.
struct App(Child);

impl Drop for App {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn launch(data_root: &Path) -> App {
    let child = Command::new(env!("CARGO_BIN_EXE_warden-app"))
        .env("WARDEN_DATA_ROOT", data_root)
        .env_remove("WARDEN_SECRET_BACKEND")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("não abriu o app");
    App(child)
}

fn log_text(data_root: &Path) -> String {
    let Ok(entries) = std::fs::read_dir(data_root.join("data").join("logs")) else {
        return String::new();
    };
    entries
        .filter_map(Result::ok)
        .filter_map(|entry| std::fs::read_to_string(entry.path()).ok())
        .collect()
}

/// Espera (consultando o disco) até o registro do app conter `needle`.
fn wait_for_log(data_root: &Path, needle: &str, app: &mut App) {
    let deadline = Instant::now() + READY_TIMEOUT;
    while Instant::now() < deadline {
        if log_text(data_root).contains(needle) {
            return;
        }
        if let Some(status) = app.0.try_wait().unwrap() {
            panic!("o app saiu antes de {needle:?}: {status}");
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    panic!("o registro não mostrou {needle:?} em {READY_TIMEOUT:?}");
}

fn wait_exit(app: &mut App) -> Option<std::process::ExitStatus> {
    let deadline = Instant::now() + EXIT_TIMEOUT;
    while Instant::now() < deadline {
        if let Some(status) = app.0.try_wait().unwrap() {
            return Some(status);
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    None
}

/// Entradas com "kriticales" no nome dentro das pastas de dados do usuário, com a data de
/// modificação (só os nomes; o conteúdo não é lido).
fn installed_app_entries() -> BTreeMap<PathBuf, Option<SystemTime>> {
    let mut entries = BTreeMap::new();
    for var in [
        "APPDATA",
        "LOCALAPPDATA",
        "XDG_CONFIG_HOME",
        "XDG_DATA_HOME",
    ] {
        let Some(base) = std::env::var_os(var) else {
            continue;
        };
        let Ok(list) = std::fs::read_dir(&base) else {
            continue;
        };
        for entry in list.filter_map(Result::ok) {
            let name = entry.file_name().to_string_lossy().to_lowercase();
            if name.contains("kriticales") {
                let modified = entry.metadata().ok().and_then(|meta| meta.modified().ok());
                entries.insert(entry.path(), modified);
            }
        }
    }
    entries
}

fn files_outside(root: &Path, allowed: &[&str]) -> Vec<PathBuf> {
    std::fs::read_dir(root)
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            !allowed.contains(&name.as_str())
        })
        .collect()
}

#[test]
#[ignore = "app-real"]
fn f0_05_ca6_instancia_unica_por_pasta_de_dados() {
    let before = installed_app_entries();
    let root_a = tempfile::tempdir().unwrap();
    let root_b = tempfile::tempdir().unwrap();

    let mut first = launch(root_a.path());
    wait_for_log(root_a.path(), "configurações lidas", &mut first);

    // Mesma pasta: a segunda cópia avisa a primeira e sai.
    let mut second = launch(root_a.path());
    let status = wait_exit(&mut second).expect("a segunda cópia com a mesma pasta não saiu");
    assert!(status.success(), "a segunda cópia saiu com {status}");
    wait_for_log(root_a.path(), "segunda cópia do Warden aberta", &mut first);
    assert!(
        first.0.try_wait().unwrap().is_none(),
        "a primeira cópia fechou"
    );

    // Outra pasta: abre ao lado da primeira.
    let mut third = launch(root_b.path());
    wait_for_log(root_b.path(), "configurações lidas", &mut third);
    assert!(
        third.0.try_wait().unwrap().is_none(),
        "a cópia com outra pasta fechou"
    );
    assert!(first.0.try_wait().unwrap().is_none());

    drop(third);
    drop(first);

    // Tudo dentro das pastas indicadas: config/, data/ (com o perfil do WebView) e o cofre
    // de teste.
    for root in [root_a.path(), root_b.path()] {
        assert!(
            root.join("data").join("EBWebView").is_dir(),
            "perfil do WebView fora da pasta"
        );
        let outside = files_outside(root, &["config", "data", "cofre-de-teste"]);
        assert!(outside.is_empty(), "criado na raiz: {outside:?}");
    }
    let after = installed_app_entries();
    assert_eq!(
        before, after,
        "pastas do Warden instalado foram criadas ou alteradas"
    );
}
