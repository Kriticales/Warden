//! Montagem de jars sintéticos na memória para os testes.

#![allow(dead_code, unreachable_pub)] // Cada arquivo de teste usa uma parte destes auxiliares.

use std::io::{Cursor, Write};

use zip::CompressionMethod;
use zip::write::{SimpleFileOptions, ZipWriter};

/// Monta um jar (zip) com as entradas dadas.
#[derive(Default)]
pub struct JarBuilder {
    entries: Vec<(String, Vec<u8>, CompressionMethod)>,
}

impl JarBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Entrada comprimida com deflate.
    pub fn file(mut self, name: &str, content: impl AsRef<[u8]>) -> Self {
        self.entries.push((
            name.to_owned(),
            content.as_ref().to_vec(),
            CompressionMethod::Deflated,
        ));
        self
    }

    /// Entrada sem compressão (como os jars embutidos do Fabric).
    pub fn stored(mut self, name: &str, content: impl AsRef<[u8]>) -> Self {
        self.entries.push((
            name.to_owned(),
            content.as_ref().to_vec(),
            CompressionMethod::Stored,
        ));
        self
    }

    /// Classe Java mínima: só o cabeçalho com a versão.
    pub fn class(self, name: &str, major: u16) -> Self {
        let mut header = vec![0xCA, 0xFE, 0xBA, 0xBE, 0, 0];
        header.extend_from_slice(&major.to_be_bytes());
        self.file(name, header)
    }

    pub fn build(self) -> Vec<u8> {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        for (name, content, method) in self.entries {
            let options = SimpleFileOptions::default()
                .compression_method(method)
                .compression_level((method == CompressionMethod::Deflated).then_some(1))
                .large_file(content.len() > u32::MAX as usize / 2);
            writer.start_file(name, options).expect("start_file");
            writer.write_all(&content).expect("write");
        }
        writer.finish().expect("finish").into_inner()
    }
}

/// `fabric.mod.json` mínimo.
pub fn fabric_mod_json(id: &str, version: &str, jars: &[&str]) -> String {
    let jars: Vec<String> = jars
        .iter()
        .map(|j| format!(r#"{{"file": "{j}"}}"#))
        .collect();
    format!(
        r#"{{"schemaVersion": 1, "id": "{id}", "version": "{version}", "jars": [{}]}}"#,
        jars.join(",")
    )
}

/// Agente de usuário dos testes de rede (o Modrinth pede um identificável).
pub const USER_AGENT: &str = "Kriticales/Warden (testes de warden-jarmeta)";

/// Pasta de cache dos downloads dos testes de rede (fora do git, dentro da `target`).
pub fn cache_dir(name: &str) -> std::path::PathBuf {
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    std::fs::create_dir_all(&dir).expect("pasta de cache");
    dir
}

/// sha1 em hexadecimal minúsculo.
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

/// Baixa `url` (com até 3 tentativas), confere o sha1 e guarda no cache pelo hash.
pub fn download_cached(url: &str, sha1: &str, cache: &std::path::Path) -> Result<Vec<u8>, String> {
    let path = cache.join(format!("{sha1}.jar"));
    if let Ok(bytes) = std::fs::read(&path)
        && sha1_hex(&bytes) == sha1
    {
        return Ok(bytes);
    }
    let mut last = String::new();
    for _ in 0..3 {
        match fetch(url) {
            Ok(bytes) if sha1_hex(&bytes) == sha1 => {
                std::fs::write(&path, &bytes).map_err(|e| e.to_string())?;
                return Ok(bytes);
            }
            Ok(bytes) => last = format!("sha1 diferente: {} (esperado {sha1})", sha1_hex(&bytes)),
            Err(e) => last = e,
        }
    }
    Err(format!("{url}: {last}"))
}

/// GET simples, com limite de 512 MB.
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
