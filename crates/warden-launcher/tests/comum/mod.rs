//! Auxiliares dos testes de integração da `warden-launcher`.

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

use std::path::{Path, PathBuf};
use std::sync::Arc;

use warden_http::{HttpClient, HttpConfig};
use warden_java::adoptium::AdoptiumClient;
use warden_java::mojang::MojangRuntimeClient;
use warden_java::{JavaRuntimes, JavaRuntimesConfig, LoaderKind, Platform, ProcessProbe};
use warden_launcher::{GameSpec, InstalledGame, LoaderSpec};

/// Uma combinação da matriz da L-05 (ROADMAP L-05; S-R5-3 §3 e §15).
#[derive(Debug, Clone, Copy)]
pub struct Combo {
    /// Nome dos arquivos de fixture (`forge-1.20.1`).
    pub name: &'static str,
    /// Versão do Minecraft.
    pub minecraft: &'static str,
    /// Loader (`vanilla`, `fabric`, `quilt`, `forge`, `neoforge`).
    pub loader: &'static str,
    /// Versão exata do loader (vazia no vanilla).
    pub version: &'static str,
}

impl Combo {
    /// O spec da combinação.
    pub fn spec(&self) -> GameSpec {
        let version = self.version.to_owned();
        let loader = match self.loader {
            "vanilla" => LoaderSpec::Vanilla,
            "fabric" => LoaderSpec::Fabric { version },
            "quilt" => LoaderSpec::Quilt { version },
            "forge" => LoaderSpec::Forge { version },
            "neoforge" => LoaderSpec::NeoForge { version },
            other => panic!("loader desconhecido: {other}"),
        };
        GameSpec::new(self.minecraft, loader).unwrap()
    }

    /// O loader para a política de Java.
    pub fn loader_kind(&self) -> LoaderKind {
        match self.loader {
            "vanilla" => LoaderKind::Vanilla,
            "fabric" => LoaderKind::Fabric,
            "quilt" => LoaderKind::Quilt,
            "forge" => LoaderKind::Forge,
            _ => LoaderKind::NeoForge,
        }
    }
}

const fn combo(
    name: &'static str,
    minecraft: &'static str,
    loader: &'static str,
    version: &'static str,
) -> Combo {
    Combo {
        name,
        minecraft,
        loader,
        version,
    }
}

/// As 20 combinações da matriz (ROADMAP L-05, D7).
pub const MATRIX: [Combo; 20] = [
    combo("forge-1.7.10", "1.7.10", "forge", "10.13.4.1614"),
    combo("forge-1.12.2", "1.12.2", "forge", "14.23.5.2860"),
    combo("forge-1.16.5", "1.16.5", "forge", "36.2.34"),
    combo("forge-1.18.2", "1.18.2", "forge", "40.3.0"),
    combo("forge-1.19.2", "1.19.2", "forge", "43.5.0"),
    combo("forge-1.20.1", "1.20.1", "forge", "47.4.10"),
    combo("forge-1.21.1", "1.21.1", "forge", "52.1.0"),
    combo("forge-26.2", "26.2", "forge", "65.1.0"),
    combo("neoforge-1.20.1", "1.20.1", "neoforge", "47.1.106"),
    combo("neoforge-1.21.1", "1.21.1", "neoforge", "21.1.252"),
    combo("neoforge-26.2", "26.2", "neoforge", "26.2.0.88"),
    combo("fabric-1.16.5", "1.16.5", "fabric", "0.19.5"),
    combo("fabric-1.18.2", "1.18.2", "fabric", "0.19.5"),
    combo("fabric-1.19.2", "1.19.2", "fabric", "0.19.5"),
    combo("fabric-1.20.1", "1.20.1", "fabric", "0.19.5"),
    combo("fabric-1.21.1", "1.21.1", "fabric", "0.19.5"),
    combo("fabric-26.2", "26.2", "fabric", "0.19.5"),
    combo("fabric-26.3", "26.3", "fabric", "0.19.5"),
    combo("quilt-1.20.1", "1.20.1", "quilt", "0.24.0"),
    combo("vanilla-26.3", "26.3", "vanilla", ""),
];

/// Pasta das fixtures desta crate.
pub fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
}

/// Marcador que substitui a pasta `shared/` nas fixtures.
pub const SHARED_MARK: &str = "<SHARED>";

/// Troca a pasta `shared/` real pelo marcador, em todos os argumentos.
pub fn normalize(game: &InstalledGame, shared: &Path) -> InstalledGame {
    let shared = shared.display().to_string();
    let fix = |text: &String| text.replace(&shared, SHARED_MARK);
    InstalledGame {
        jvm_args: game.jvm_args.iter().map(fix).collect(),
        game_args: game.game_args.iter().map(fix).collect(),
        game_dir_placeholder: fix(&game.game_dir_placeholder),
        ..game.clone()
    }
}

/// Cliente HTTP dos testes de rede.
pub fn http() -> HttpClient {
    HttpClient::new(HttpConfig::for_version("teste-launcher")).unwrap()
}

/// O serviço de Java com as fontes reais, numa pasta dada.
pub fn java_service(root: &Path) -> JavaRuntimes {
    let http = http();
    let config = JavaRuntimesConfig {
        runtimes_dir: root.join("shared").join("runtimes"),
        downloads_dir: root.join("cache").join("tmp").join("java"),
        platform: Platform::current().unwrap(),
    };
    let adoptium = AdoptiumClient::new(http.clone()).unwrap();
    let mojang = MojangRuntimeClient::new(http.clone()).unwrap();
    JavaRuntimes::with_clients(
        config,
        http,
        adoptium,
        mojang,
        Arc::new(ProcessProbe::default()),
    )
    .unwrap()
}

/// Pasta de dados dos testes que baixam o jogo: `WARDEN_LAUNCHER_DADOS` (para reaproveitar
/// entre execuções) ou uma temporária dentro de `WARDEN_DATA_ROOT` (ADR-0053).
pub fn data_root() -> (PathBuf, Option<tempfile::TempDir>) {
    if let Some(dir) = std::env::var_os("WARDEN_LAUNCHER_DADOS").filter(|value| !value.is_empty()) {
        let dir = PathBuf::from(dir);
        std::fs::create_dir_all(&dir).unwrap();
        return (dir, None);
    }
    let temp = match std::env::var_os("WARDEN_DATA_ROOT").filter(|value| !value.is_empty()) {
        Some(root) => {
            std::fs::create_dir_all(&root).unwrap();
            tempfile::Builder::new()
                .prefix("launcher-rede-")
                .tempdir_in(root)
                .unwrap()
        }
        None => tempfile::tempdir().unwrap(),
    };
    (temp.path().to_path_buf(), Some(temp))
}

/// Se a variável de ambiente vale `1`.
pub fn env_on(name: &str) -> bool {
    std::env::var_os(name).is_some_and(|value| value == "1")
}

/// Um Java para os testes com processo de verdade: `WARDEN_TEST_JAVA` (caminho do `java`),
/// senão o de `JAVA_HOME`. Sem Java, avisa e devolve `None`; com
/// `WARDEN_REQUIRE_EXTERNALS=1` (CI), a falta é falha.
pub fn test_java() -> Option<PathBuf> {
    let exe = if cfg!(windows) { "java.exe" } else { "java" };
    let candidate = std::env::var_os("WARDEN_TEST_JAVA")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("JAVA_HOME")
                .filter(|value| !value.is_empty())
                .map(|home| PathBuf::from(home).join("bin").join(exe))
        });
    if let Some(java) = candidate.filter(|path| path.is_file()) {
        return Some(java);
    }
    assert!(
        !env_on("WARDEN_REQUIRE_EXTERNALS"),
        "WARDEN_REQUIRE_EXTERNALS=1, mas não há Java (defina WARDEN_TEST_JAVA ou JAVA_HOME)"
    );
    eprintln!("AVISO: sem Java (WARDEN_TEST_JAVA ou JAVA_HOME); teste com processo Java pulado.");
    None
}
