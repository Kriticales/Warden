//! Auxiliares dos testes de integração da `warden-java`: JREs falsos (zip e tar.gz com o
//! mesmo formato dos pacotes do Temurin), o validador falso, respostas do Adoptium no formato
//! real e o serviço montado contra um servidor simulado.

// Nos auxiliares dos testes, `unwrap`, `expect` e `panic!` são a forma idiomática de falhar
// (o `clippy.toml` só os libera dentro das funções `#[test]`).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    dead_code,
    unreachable_pub
)]

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use warden_http::{HttpClient, HttpConfig, ManualTimer};
use warden_java::adoptium::AdoptiumClient;
use warden_java::mojang::MojangRuntimeClient;
use warden_java::{
    Error, JavaInfo, JavaProbe, JavaRuntimes, JavaRuntimesConfig, JavaVersion, Os, Platform,
};
use warden_packwiz::HashFormat;
use warden_packwiz::hash::hash_bytes;

/// A plataforma em que o teste roda.
pub fn platform() -> Platform {
    Platform::current().unwrap()
}

/// Arquivos de um JRE falso: o `release` (lido pelo [`FakeProbe`]), o `java`/`javaw` e um
/// arquivo grande o bastante para o download ter várias partes.
fn jre_files(version: &str) -> Vec<(String, Vec<u8>)> {
    let release = format!("JAVA_VERSION=\"{version}\"\nIMPLEMENTOR=\"Eclipse Adoptium\"\n");
    let filler: Vec<u8> = (0..200_000_u32).map(|i| (i % 251) as u8).collect();
    vec![
        ("release".into(), release.into_bytes()),
        ("bin/java.exe".into(), b"MZ java".to_vec()),
        ("bin/javaw.exe".into(), b"MZ javaw".to_vec()),
        ("bin/java".into(), b"#!java".to_vec()),
        ("lib/modules".into(), filler),
        ("legal/java.base/LICENSE".into(), b"GPLv2 + CPE".to_vec()),
    ]
}

/// Um `.zip` como o do Temurin: tudo dentro de `<top>/`.
pub fn jre_zip(top: &str, version: &str) -> Vec<u8> {
    let mut buffer = std::io::Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut buffer);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .unix_permissions(0o755);
        zip.add_directory(format!("{top}/"), options).unwrap();
        for (name, bytes) in jre_files(version) {
            zip.start_file(format!("{top}/{name}"), options).unwrap();
            zip.write_all(&bytes).unwrap();
        }
        zip.finish().unwrap();
    }
    buffer.into_inner()
}

/// Um `.tar.gz` como o do Temurin.
pub fn jre_tar_gz(top: &str, version: &str) -> Vec<u8> {
    let encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    let mut tar = tar::Builder::new(encoder);
    for (name, bytes) in jre_files(version) {
        let mut header = tar::Header::new_gnu();
        header.set_size(bytes.len() as u64);
        header.set_mode(0o755);
        header.set_cksum();
        tar.append_data(&mut header, format!("{top}/{name}"), bytes.as_slice())
            .unwrap();
    }
    tar.into_inner().unwrap().finish().unwrap()
}

/// O pacote no formato da plataforma (zip no Windows, tar.gz no Linux) e o nome.
pub fn jre_package(major: u32, version: &str, top: &str) -> (String, Vec<u8>) {
    match platform().os {
        Os::Windows => (
            format!("OpenJDK{major}U-jre_x64_windows_hotspot_{version}.zip"),
            jre_zip(top, version),
        ),
        Os::Linux => (
            format!("OpenJDK{major}U-jre_x64_linux_hotspot_{version}.tar.gz"),
            jre_tar_gz(top, version),
        ),
    }
}

/// SHA-256 em hexadecimal.
pub fn sha256(bytes: &[u8]) -> String {
    hash_bytes(HashFormat::Sha256, bytes)
}

/// Item de `assets/latest` no formato real da API (só os campos que importam e alguns a mais).
pub fn latest_item(
    version: &JavaVersion,
    release_name: &str,
    package_name: &str,
    link: &str,
    bytes: &[u8],
) -> serde_json::Value {
    let platform = platform();
    serde_json::json!({
        "binary": {
            "architecture": platform.arch_key(),
            "download_count": 1,
            "heap_size": "normal",
            "image_type": "jre",
            "jvm_impl": "hotspot",
            "os": platform.adoptium_os(),
            "package": {
                "checksum": sha256(bytes),
                "link": link,
                "name": package_name,
                "size": bytes.len(),
            },
            "project": "jdk",
        },
        "release_name": release_name,
        "vendor": "eclipse",
        "version": {
            "build": version.build,
            "major": version.major,
            "minor": version.minor,
            "patch": version.patch,
            "security": version.security,
            "openjdk_version": format!("{version}+{}", version.build),
            "semver": format!("{}.{}.{}+{}", version.major, version.minor, version.security, version.build),
        },
    })
}

/// Validador falso: lê o `release` da pasta do Java (`bin/..`). Conta as chamadas.
#[derive(Debug, Default)]
pub struct FakeProbe {
    pub calls: AtomicUsize,
}

#[async_trait::async_trait]
impl JavaProbe for FakeProbe {
    async fn probe(&self, java: &Path) -> warden_java::Result<JavaInfo> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let home = java.parent().and_then(Path::parent).unwrap();
        let release =
            std::fs::read_to_string(home.join("release")).map_err(|error| Error::Validation {
                path: java.to_owned(),
                message: format!("sem release: {error}"),
                output: None,
            })?;
        let version = release
            .lines()
            .find_map(|line| line.strip_prefix("JAVA_VERSION=\""))
            .and_then(|rest| rest.strip_suffix('"'))
            .and_then(JavaVersion::parse)
            .ok_or_else(|| Error::Validation {
                path: java.to_owned(),
                message: "release sem JAVA_VERSION".into(),
                output: Some(release.clone()),
            })?;
        Ok(JavaInfo {
            version,
            vendor: "Eclipse Adoptium".into(),
            vm_name: "OpenJDK 64-Bit Server VM".into(),
            arch: "amd64".into(),
            is_64bit: true,
        })
    }
}

/// Cliente HTTP de teste: sem esperas reais entre tentativas.
pub fn http() -> HttpClient {
    let mut config = HttpConfig::for_version("teste");
    config.max_retries = 1;
    HttpClient::with_timer(config, Arc::new(ManualTimer::new())).unwrap()
}

/// Pastas e serviço de teste.
pub struct Env {
    pub dir: tempfile::TempDir,
    pub runtimes: JavaRuntimes,
    pub probe: Arc<FakeProbe>,
}

impl Env {
    pub fn runtimes_dir(&self) -> PathBuf {
        self.dir.path().join("runtimes")
    }

    pub fn downloads_dir(&self) -> PathBuf {
        self.dir.path().join("downloads")
    }

    /// Nomes em `shared/runtimes/` (inclusive os que começam com ponto).
    pub fn runtime_dir_entries(&self) -> Vec<String> {
        match std::fs::read_dir(self.runtimes_dir()) {
            Ok(entries) => {
                let mut names: Vec<String> = entries
                    .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
                    .collect();
                names.sort();
                names
            }
            Err(_) => Vec::new(),
        }
    }
}

/// Serviço com o Adoptium em `adoptium_base` e o índice da Mojang em `mojang_index`.
pub fn env(adoptium_base: &str, mojang_index: &str) -> Env {
    let dir = tempfile::tempdir().unwrap();
    let probe = Arc::new(FakeProbe::default());
    let http = http();
    let config = JavaRuntimesConfig {
        runtimes_dir: dir.path().join("runtimes"),
        downloads_dir: dir.path().join("downloads"),
        platform: platform(),
    };
    let adoptium = AdoptiumClient::with_base_url(http.clone(), adoptium_base).unwrap();
    let mojang = MojangRuntimeClient::with_index_url(http.clone(), mojang_index).unwrap();
    let runtimes =
        JavaRuntimes::with_clients(config, http, adoptium, mojang, probe.clone()).unwrap();
    Env {
        dir,
        runtimes,
        probe,
    }
}

/// Lê uma fixture HTTP gravada.
pub fn fixture(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("http")
        .join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}
