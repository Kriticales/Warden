//! `cargo xtask installer`: baixa, com versão e SHA-256 fixados, o que o teste de conformidade
//! da materialização (L-03, ADR-0011) precisa para rodar o packwiz-installer real:
//!
//! - `packwiz-installer-bootstrap.jar` e `packwiz-installer.jar` (Releases do GitHub, pela URL
//!   de download, que não passa pela API);
//! - um JRE Temurin 21 (Adoptium) para Windows ou Linux x64.
//!
//! Tudo vai para uma pasta de cache do usuário, fora do git e dividida entre os worktrees
//! ([`cache_dir`]); a variável `WARDEN_INSTALLER_CACHE` troca a pasta. No fim, grava
//! `externals.json` com os caminhos, que os testes da `warden-instance` leem. Um arquivo que
//! já está no cache com o hash certo não é baixado de novo.

use std::fmt::Write as _;
use std::fs;
use std::io::Read as _;
use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result, bail, ensure};
use serde::Serialize;
use sha2::{Digest as _, Sha256};

use crate::e2e::curl_args;
use crate::util::{Cmd, find_program};

/// Variável que troca a pasta do cache.
pub const CACHE_ENV: &str = "WARDEN_INSTALLER_CACHE";

/// Arquivo com os caminhos, lido pelos testes.
pub const EXTERNALS_FILE: &str = "externals.json";

/// Um arquivo fixado: endereço, nome no cache e SHA-256.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pinned {
    /// Endereço de download.
    pub url: &'static str,
    /// Nome do arquivo no cache.
    pub file_name: &'static str,
    /// SHA-256 em hexadecimal minúsculo.
    pub sha256: &'static str,
}

/// packwiz-installer-bootstrap v0.0.3 (12/07/2020, a última).
pub const BOOTSTRAP: Pinned = Pinned {
    url: "https://github.com/packwiz/packwiz-installer-bootstrap/releases/download/v0.0.3/packwiz-installer-bootstrap.jar",
    file_name: "packwiz-installer-bootstrap-0.0.3.jar",
    sha256: "a8fbb24dc604278e97f4688e82d3d91a318b98efc08d5dbfcbcbcab6443d116c",
};

/// packwiz-installer v0.5.14 (21/04/2024, a última).
pub const INSTALLER: Pinned = Pinned {
    url: "https://github.com/packwiz/packwiz-installer/releases/download/v0.5.14/packwiz-installer.jar",
    file_name: "packwiz-installer-0.5.14.jar",
    sha256: "c9f646908d340d84773948a9a7d98bc1dae250d35e1016dc6e2b8459760b5598",
};

/// Versão do JRE Temurin (Adoptium) usado nos testes.
pub const JRE_RELEASE: &str = "21.0.12.1+1";

/// JRE Temurin 21 para Windows x64 (zip).
pub const JRE_WINDOWS: Pinned = Pinned {
    url: "https://github.com/adoptium/temurin21-binaries/releases/download/jdk-21.0.12.1%2B1/OpenJDK21U-jre_x64_windows_hotspot_21.0.12.1_1.zip",
    file_name: "OpenJDK21U-jre_x64_windows_hotspot_21.0.12.1_1.zip",
    sha256: "d35f31e712f0fcf6ac5a093edc90204fbff22f720ba3950bd09d331d5e621636",
};

/// JRE Temurin 21 para Linux x64 (tar.gz).
pub const JRE_LINUX: Pinned = Pinned {
    url: "https://github.com/adoptium/temurin21-binaries/releases/download/jdk-21.0.12.1%2B1/OpenJDK21U-jre_x64_linux_hotspot_21.0.12.1_1.tar.gz",
    file_name: "OpenJDK21U-jre_x64_linux_hotspot_21.0.12.1_1.tar.gz",
    sha256: "2413149700df0f7d440500a84a8f764c535f21e5a5e87d38328b64eec2c5b500",
};

/// Caminhos gravados em `externals.json`.
#[derive(Debug, Serialize)]
struct Externals {
    java: PathBuf,
    bootstrap: PathBuf,
    installer: PathBuf,
    jre_release: &'static str,
}

/// Pasta do cache: `WARDEN_INSTALLER_CACHE`, senão `%LOCALAPPDATA%\Warden-dev\cache\installer`
/// no Windows e `$XDG_CACHE_HOME/Warden-dev/installer` (ou `~/.cache/...`) no Linux.
pub fn cache_dir() -> Result<PathBuf> {
    if let Some(dir) = std::env::var_os(CACHE_ENV).filter(|value| !value.is_empty()) {
        return Ok(PathBuf::from(dir));
    }
    if cfg!(windows) {
        let local = std::env::var_os("LOCALAPPDATA").context("LOCALAPPDATA não definida")?;
        return Ok(PathBuf::from(local)
            .join("Warden-dev")
            .join("cache")
            .join("installer"));
    }
    if let Some(cache) = std::env::var_os("XDG_CACHE_HOME").filter(|value| !value.is_empty()) {
        return Ok(PathBuf::from(cache).join("Warden-dev").join("installer"));
    }
    let home = std::env::var_os("HOME").context("HOME não definida")?;
    Ok(PathBuf::from(home)
        .join(".cache")
        .join("Warden-dev")
        .join("installer"))
}

/// SHA-256 de um arquivo, em hexadecimal minúsculo.
fn sha256_file(path: &Path) -> Result<String> {
    let mut file =
        fs::File::open(path).with_context(|| format!("falha ao abrir {}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .with_context(|| format!("falha ao ler {}", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    let mut text = String::with_capacity(64);
    for byte in hasher.finalize() {
        // Escrever numa String não falha.
        let _ = write!(text, "{byte:02x}");
    }
    Ok(text)
}

/// Garante `pinned` em `dir` com o hash certo e devolve o caminho.
fn ensure_pinned(dir: &Path, pinned: &Pinned) -> Result<PathBuf> {
    let path = dir.join(pinned.file_name);
    if path.is_file() && sha256_file(&path)? == pinned.sha256 {
        println!("installer: {} já está no cache.", pinned.file_name);
        return Ok(path);
    }
    let partial = dir.join(format!("{}.part", pinned.file_name));
    println!("installer: baixando {}", pinned.url);
    Cmd::new(find_program("curl")?)
        .args(curl_args(pinned.url, &partial))
        .run()
        .with_context(|| format!("installer: falha ao baixar {}", pinned.url))?;
    let actual = sha256_file(&partial)?;
    if actual != pinned.sha256 {
        // Não deixa o arquivo errado no cache.
        let _ = fs::remove_file(&partial);
        bail!(
            "installer: o SHA-256 de {} não confere (esperado {}, recebido {actual})",
            pinned.file_name,
            pinned.sha256
        );
    }
    fs::rename(&partial, &path)
        .with_context(|| format!("falha ao renomear para {}", path.display()))?;
    Ok(path)
}

/// Nome do executável do Java dentro do JRE.
fn java_executable(jre_dir: &Path) -> PathBuf {
    jre_dir
        .join("bin")
        .join(if cfg!(windows) { "java.exe" } else { "java" })
}

/// Extrai o zip do JRE em `destination` (só entradas com caminho seguro).
fn extract_zip(archive_path: &Path, destination: &Path) -> Result<()> {
    let file = fs::File::open(archive_path)
        .with_context(|| format!("falha ao abrir {}", archive_path.display()))?;
    let mut archive = zip::ZipArchive::new(file).context("installer: o JRE não é um zip válido")?;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        let Some(relative) = entry.enclosed_name() else {
            bail!(
                "installer: o zip do JRE tem um caminho inseguro: {}",
                entry.name()
            );
        };
        let target = destination.join(relative);
        if entry.is_dir() {
            fs::create_dir_all(&target)?;
            continue;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut output = fs::File::create(&target)
            .with_context(|| format!("falha ao criar {}", target.display()))?;
        std::io::copy(&mut entry, &mut output)?;
    }
    Ok(())
}

/// Garante o JRE extraído em `<dir>/jre-<versão>/` e devolve o caminho do `java`.
fn ensure_jre(dir: &Path) -> Result<PathBuf> {
    let jre_dir = dir.join(format!("jre-{JRE_RELEASE}"));
    let java = java_executable(&jre_dir);
    if java.is_file() {
        println!("installer: JRE {JRE_RELEASE} já está no cache.");
        return Ok(java);
    }
    let pinned = if cfg!(windows) {
        JRE_WINDOWS
    } else {
        JRE_LINUX
    };
    let archive = ensure_pinned(dir, &pinned)?;
    let staging = dir.join("jre-extraindo");
    if staging.exists() {
        fs::remove_dir_all(&staging)
            .with_context(|| format!("falha ao apagar {}", staging.display()))?;
    }
    fs::create_dir_all(&staging)?;
    if cfg!(windows) {
        extract_zip(&archive, &staging)?;
    } else {
        Cmd::new(find_program("tar")?)
            .args([
                std::ffi::OsStr::new("-xzf"),
                archive.as_os_str(),
                std::ffi::OsStr::new("-C"),
                staging.as_os_str(),
            ])
            .run()?;
    }
    // O pacote tem uma pasta só no topo (`jdk-<versão>-jre`).
    let top: Vec<PathBuf> = fs::read_dir(&staging)?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.is_dir())
        .collect();
    ensure!(
        top.len() == 1,
        "installer: esperava uma pasta no topo do JRE, achei {}",
        top.len()
    );
    fs::rename(&top[0], &jre_dir)
        .with_context(|| format!("falha ao renomear para {}", jre_dir.display()))?;
    fs::remove_dir_all(&staging)?;
    fs::remove_file(&archive)?;
    ensure!(
        java.is_file(),
        "installer: o JRE extraído não tem {}",
        java.display()
    );
    Ok(java)
}

/// `cargo xtask installer`.
pub fn run() -> Result<()> {
    let dir = cache_dir()?;
    fs::create_dir_all(&dir).with_context(|| format!("falha ao criar {}", dir.display()))?;
    println!("installer: cache em {}", dir.display());
    let bootstrap = ensure_pinned(&dir, &BOOTSTRAP)?;
    let installer = ensure_pinned(&dir, &INSTALLER)?;
    let java = ensure_jre(&dir)?;
    let version = Cmd::new(&java).args(["-version"]).read();
    if let Err(error) = version {
        bail!("installer: o Java baixado não roda: {error:#}");
    }
    let externals = Externals {
        java,
        bootstrap,
        installer,
        jre_release: JRE_RELEASE,
    };
    let json = serde_json::to_string_pretty(&externals)?;
    fs::write(dir.join(EXTERNALS_FILE), json)?;
    println!(
        "installer: pronto ({}).",
        dir.join(EXTERNALS_FILE).display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixados_tem_sha256_valido_e_https() {
        for pinned in [BOOTSTRAP, INSTALLER, JRE_WINDOWS, JRE_LINUX] {
            assert_eq!(pinned.sha256.len(), 64, "{}", pinned.file_name);
            assert!(
                pinned
                    .sha256
                    .chars()
                    .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
            );
            assert!(
                pinned.url.starts_with("https://github.com/"),
                "{}",
                pinned.url
            );
            assert!(!pinned.url.contains("api.github.com"), "{}", pinned.url);
        }
    }

    #[test]
    fn jre_tem_a_versao_no_nome() {
        let encoded = JRE_RELEASE.replace('+', "%2B");
        for pinned in [JRE_WINDOWS, JRE_LINUX] {
            assert!(pinned.url.contains(&encoded), "{}", pinned.url);
            assert!(pinned.file_name.contains(&JRE_RELEASE.replace('+', "_")));
        }
    }

    #[test]
    fn java_dentro_de_bin() {
        let java = java_executable(Path::new("jre"));
        assert!(java.starts_with(Path::new("jre").join("bin")));
    }
}
