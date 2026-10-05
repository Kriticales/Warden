//! Auxiliares dos testes de integração.

// Nos auxiliares dos testes, `unwrap`, `expect` e `panic!` são a forma idiomática de falhar
// (o `clippy.toml` só os libera dentro das funções `#[test]`). Cada arquivo de teste usa uma
// parte destes auxiliares.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr,
    dead_code,
    unreachable_pub
)]

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use warden_packwiz::hygiene::packwizignore_template;
use warden_packwiz::{Loader, PackIndex, PackManifest};
use warden_packwiz_cli::Packwiz;

/// O binário do packwiz: `WARDEN_PACKWIZ_BIN`, senão o sidecar que `cargo xtask build-packwiz`
/// grava em `apps/desktop/src-tauri/binaries/`. Sem binário, avisa e devolve `None`; com
/// `WARDEN_REQUIRE_EXTERNALS=1` (CI), a falta é falha.
pub fn real_binary() -> Option<PathBuf> {
    let candidate = std::env::var_os("WARDEN_PACKWIZ_BIN")
        .filter(|value| !value.is_empty())
        .map_or_else(sidecar_path, PathBuf::from);
    if candidate.is_file() {
        return Some(candidate);
    }
    assert!(
        std::env::var_os("WARDEN_REQUIRE_EXTERNALS").is_none_or(|value| value != "1"),
        "WARDEN_REQUIRE_EXTERNALS=1, mas o packwiz não está em {}",
        candidate.display()
    );
    eprintln!(
        "AVISO: packwiz não encontrado em {} (rode `cargo xtask build-packwiz` ou defina \
         WARDEN_PACKWIZ_BIN); teste com o packwiz real pulado.",
        candidate.display()
    );
    None
}

fn sidecar_path() -> PathBuf {
    let name = if cfg!(windows) {
        "packwiz-x86_64-pc-windows-msvc.exe"
    } else {
        "packwiz-x86_64-unknown-linux-gnu"
    };
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../apps/desktop/src-tauri/binaries")
        .join(name)
}

/// O executável falso (`src/bin/packwiz_fake.rs`).
pub fn fake_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_packwiz-fake"))
}

/// `Packwiz` com cache e config numa pasta temporária.
pub fn packwiz(binary: PathBuf, temp: &Path) -> Packwiz {
    Packwiz::new(
        binary,
        temp.join("dados/cache/packwiz"),
        temp.join("dados/packwiz/packwiz.toml"),
    )
}

/// Grava `text` em `root/path`, criando as pastas.
pub fn write(root: &Path, path: &str, text: &str) {
    let full = root.join(path);
    fs::create_dir_all(full.parent().unwrap()).unwrap();
    fs::write(full, text).unwrap();
}

/// Pack novo escrito só pelo Warden: `pack.toml` (Fabric 1.21.1), `index.toml` vazio e o
/// `.packwizignore` padrão.
pub fn new_pack(root: &Path) {
    let mut pack = PackManifest::new("Teste da P1-02", "1.21.1");
    "Warden".clone_into(&mut pack.author);
    "1.0.0".clone_into(&mut pack.version);
    pack.set_loader_version(Loader::Fabric, "0.16.14");
    fs::create_dir_all(root).unwrap();
    fs::write(root.join("pack.toml"), pack.to_toml_string()).unwrap();
    fs::write(
        root.join("index.toml"),
        PackIndex::default().to_toml_string(),
    )
    .unwrap();
    fs::write(root.join(".packwizignore"), packwizignore_template()).unwrap();
}

/// Pasta que não é um pack de verdade, só com um `pack.toml` (para o executável falso).
pub fn fake_pack(root: &Path, config: &str) {
    write(root, "pack.toml", "name = \"falso\"\n");
    write(root, "fake.txt", config);
}

/// Registros do `tracing` capturados em memória (nível `debug` e acima).
#[derive(Clone, Default)]
pub struct CapturedLogs(Arc<Mutex<Vec<u8>>>);

impl CapturedLogs {
    /// Tudo o que foi registrado.
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.0.lock().unwrap()).into_owned()
    }

    /// Instala a captura como assinante padrão da thread atual, até o guarda cair.
    pub fn install(&self) -> tracing::subscriber::DefaultGuard {
        let writer = self.clone();
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .with_ansi(false)
            .with_writer(move || writer.clone())
            .finish();
        tracing::subscriber::set_default(subscriber)
    }
}

impl std::io::Write for CapturedLogs {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().write_all(buf)?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
