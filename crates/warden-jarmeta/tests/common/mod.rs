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
