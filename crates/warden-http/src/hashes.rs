//! Hashes dos downloads (gancho 1.1 da ADR-0039, item 1).
//!
//! Todo download calcula de uma vez sha1, sha256, sha512 e a impressão murmur2 da CurseForge
//! ([`FileHashes`]), para o índice do cache de downloads (L-03) e a conferência de segurança
//! da 1.1 não precisarem reler os jars.
//!
//! - sha1, sha256 e sha512 (e o md5, só quando o hash esperado é md5) são calculados durante o
//!   download, pedaço a pedaço, na mesma passada que grava no disco.
//! - O murmur2 da CurseForge não pode ser calculado assim: o algoritmo mistura o **tamanho**
//!   (já sem os espaços em branco) no estado inicial, antes do primeiro byte, e esse tamanho só
//!   se conhece no fim. Por isso ele é calculado logo depois, sobre o `.part` recém-gravado
//!   (ainda no cache do sistema), sem rede, antes da renomeação. O código é o da P1-01
//!   (`warden_packwiz::hash`), reaproveitado sem cópia.

use std::fmt;
use std::io::{self, Read};

use md5::Md5;
use sha1::Sha1;
use sha2::{Digest as _, Sha256, Sha512};
use warden_packwiz::HashFormat;
use warden_packwiz::hash::hash_matches;

/// Os hashes de um arquivo baixado.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FileHashes {
    /// SHA-1 em hexadecimal minúsculo (Modrinth e CurseForge).
    pub sha1: String,
    /// SHA-256 em hexadecimal minúsculo (cache de downloads e links diretos do packwiz).
    pub sha256: String,
    /// SHA-512 em hexadecimal minúsculo (Modrinth).
    pub sha512: String,
    /// Impressão digital da CurseForge (murmur2 sem espaços, semente 1).
    pub murmur2: u32,
    /// MD5, só quando o hash esperado do download era md5.
    pub md5: Option<String>,
}

impl FileHashes {
    /// O hash no formato dado, no texto que o packwiz grava (`None` para md5 não calculado).
    #[must_use]
    pub fn get(&self, format: HashFormat) -> Option<String> {
        match format {
            HashFormat::Sha1 => Some(self.sha1.clone()),
            HashFormat::Sha256 => Some(self.sha256.clone()),
            HashFormat::Sha512 => Some(self.sha512.clone()),
            HashFormat::Murmur2 => Some(self.murmur2.to_string()),
            HashFormat::Md5 => self.md5.clone(),
        }
    }
}

/// Hash esperado de um download (o de um `.pw.toml` ou o da API).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpectedHash {
    /// Formato.
    pub format: HashFormat,
    /// Valor (hexadecimal ou, no murmur2, decimal).
    pub value: String,
}

impl ExpectedHash {
    /// Hash esperado no formato dado.
    #[must_use]
    pub fn new(format: HashFormat, value: impl Into<String>) -> Self {
        Self {
            format,
            value: value.into(),
        }
    }

    /// Confere com os hashes calculados. Devolve o valor calculado quando não confere.
    pub(crate) fn check(&self, hashes: &FileHashes) -> Result<(), String> {
        let actual = hashes.get(self.format).unwrap_or_default();
        if hash_matches(self.format, &self.value, &actual) {
            Ok(())
        } else {
            Err(actual)
        }
    }
}

impl fmt::Display for ExpectedHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.format, self.value)
    }
}

/// sha1, sha256, sha512 (e md5, se pedido) na mesma passada.
#[derive(Clone)]
pub(crate) struct StreamHasher {
    sha1: Sha1,
    sha256: Sha256,
    sha512: Sha512,
    md5: Option<Md5>,
}

impl fmt::Debug for StreamHasher {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StreamHasher")
            .field("md5", &self.md5.is_some())
            .finish_non_exhaustive()
    }
}

impl StreamHasher {
    pub(crate) fn new(with_md5: bool) -> Self {
        Self {
            sha1: Sha1::new(),
            sha256: Sha256::new(),
            sha512: Sha512::new(),
            md5: with_md5.then(Md5::new),
        }
    }

    pub(crate) fn update(&mut self, data: &[u8]) {
        self.sha1.update(data);
        self.sha256.update(data);
        self.sha512.update(data);
        if let Some(md5) = &mut self.md5 {
            md5.update(data);
        }
    }

    /// Acrescenta tudo o que `reader` tiver (o começo de um `.part` retomado). Devolve os
    /// bytes lidos.
    pub(crate) fn update_from<R: Read>(&mut self, reader: &mut R) -> io::Result<u64> {
        let mut buffer = vec![0_u8; 64 * 1024];
        let mut total = 0_u64;
        loop {
            let read = reader.read(&mut buffer)?;
            if read == 0 {
                return Ok(total);
            }
            self.update(&buffer[..read]);
            total += read as u64;
        }
    }

    pub(crate) fn finish(self, murmur2: u32) -> FileHashes {
        FileHashes {
            sha1: hex::encode(self.sha1.finalize()),
            sha256: hex::encode(self.sha256.finalize()),
            sha512: hex::encode(self.sha512.finalize()),
            murmur2,
            md5: self.md5.map(|md5| hex::encode(md5.finalize())),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use warden_packwiz::hash::{curseforge_fingerprint, hash_bytes};

    use super::*;

    fn hashes_of(data: &[u8], with_md5: bool) -> FileHashes {
        let mut hasher = StreamHasher::new(with_md5);
        for chunk in data.chunks(7) {
            hasher.update(chunk);
        }
        hasher.finish(curseforge_fingerprint(data))
    }

    #[test]
    fn pedacos_igual_a_tudo_de_uma_vez() {
        let data: Vec<u8> = (0..100_000_u32).map(|i| (i % 251) as u8).collect();
        let hashes = hashes_of(&data, true);
        for format in HashFormat::ALL {
            assert_eq!(
                hashes.get(format).unwrap(),
                hash_bytes(format, &data),
                "{format}"
            );
        }
        let mut streamed = StreamHasher::new(false);
        assert_eq!(
            streamed.update_from(&mut Cursor::new(&data)).unwrap(),
            data.len() as u64
        );
        let other = streamed.finish(hashes.murmur2);
        assert_eq!(other.sha512, hashes.sha512);
        assert_eq!(other.md5, None);
        assert_eq!(other.get(HashFormat::Md5), None);
    }

    #[test]
    fn conferencia_do_esperado() {
        let hashes = hashes_of(b"abc", false);
        let ok = ExpectedHash::new(HashFormat::Sha1, "A9993E364706816ABA3E25717850C26C9CD0D89D");
        assert_eq!(ok.check(&hashes), Ok(()));
        let wrong = ExpectedHash::new(HashFormat::Sha256, "00");
        assert_eq!(wrong.check(&hashes), Err(hashes.sha256.clone()));
        let murmur = ExpectedHash::new(HashFormat::Murmur2, hashes.murmur2.to_string());
        assert_eq!(murmur.check(&hashes), Ok(()));
        // md5 não calculado nunca confere.
        let md5 = ExpectedHash::new(HashFormat::Md5, "900150983cd24fb0d6963f7d28e17f72");
        assert_eq!(md5.check(&hashes), Err(String::new()));
        assert_eq!(
            ok.to_string(),
            "sha1:A9993E364706816ABA3E25717850C26C9CD0D89D"
        );
        assert!(format!("{:?}", StreamHasher::new(true)).contains("md5: true"));
    }
}
