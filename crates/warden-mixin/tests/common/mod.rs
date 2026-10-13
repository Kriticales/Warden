//! Auxiliares dos testes: os jars sintéticos ([`classgen`]), o corpus e, nos testes de rede,
//! [`fetch`] e [`download_cached`].

#![allow(dead_code, unreachable_pub, clippy::expect_used, clippy::panic)]

/// Jars e classes de mixin sintéticos.
mod classgen;

#[allow(unused_imports)] // o teste de rede não usa os jars sintéticos
pub use classgen::*;

/// Pasta do corpus.
pub fn corpus(path: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/corpus")
        .join(path)
}

/// Agente de usuário dos testes de rede.
pub const USER_AGENT: &str = "Kriticales/Warden (testes de warden-mixin)";

/// Pasta de cache dos downloads (dentro da `target`).
pub fn cache_dir(name: &str) -> std::path::PathBuf {
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    std::fs::create_dir_all(&dir).expect("pasta de cache");
    dir
}

pub fn sha1_hex(bytes: &[u8]) -> String {
    use sha1::Digest;
    use std::fmt::Write as _;
    sha1::Sha1::digest(bytes)
        .iter()
        .fold(String::new(), |mut out, b| {
            let _ = write!(out, "{b:02x}");
            out
        })
}

/// Baixa `url` (até 3 tentativas), confere o sha1 e guarda no cache.
pub fn download_cached(
    url: &str,
    sha1: &str,
    cache: &std::path::Path,
    name: &str,
) -> Result<std::path::PathBuf, String> {
    let dir = cache.join(sha1);
    let path = dir.join(name);
    if let Ok(bytes) = std::fs::read(&path)
        && sha1_hex(&bytes) == sha1
    {
        return Ok(path);
    }
    let mut last = String::new();
    for _ in 0..3 {
        match fetch(url) {
            Ok(bytes) if sha1_hex(&bytes) == sha1 => {
                std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
                std::fs::write(&path, &bytes).map_err(|e| e.to_string())?;
                return Ok(path);
            }
            Ok(bytes) => last = format!("sha1 diferente: {} (esperado {sha1})", sha1_hex(&bytes)),
            Err(e) => last = e,
        }
    }
    Err(format!("{url}: {last}"))
}

pub fn fetch(url: &str) -> Result<Vec<u8>, String> {
    let mut response = ureq::get(url)
        .header("User-Agent", USER_AGENT)
        .call()
        .map_err(|e| e.to_string())?;
    response
        .body_mut()
        .with_config()
        .limit(512 * 1024 * 1024)
        .read_to_vec()
        .map_err(|e| e.to_string())
}
