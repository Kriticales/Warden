//! Hashes que o packwiz aceita (`core/hash.go`): `sha1`, `sha256`, `sha512`, `md5` em
//! hexadecimal minúsculo e `murmur2` (variante da CurseForge) em decimal.
//!
//! O murmur2 da CurseForge é o `MurmurHash2` de 32 bits de Austin Appleby com semente 1, calculado
//! sobre o arquivo **sem** os bytes 9, 10, 13 e 32 (tab, quebra de linha, retorno e espaço).
//! Porte de `curseforge/murmur2` do packwiz (MIT) e de `github.com/aviddiviner/go-murmur`
//! (MIT), no commit registrado em `THIRD_PARTY.md`.

use std::fmt;
use std::io::{self, Read, Seek, SeekFrom};

use md5::Md5;
use sha1::Sha1;
use sha2::{Digest as _, Sha256, Sha512};

use crate::error::{Error, Result};

/// Formato de hash do packwiz.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum HashFormat {
    /// `MurmurHash2` da CurseForge (o mais fraco; só para conferir com a CurseForge).
    Murmur2,
    /// MD5.
    Md5,
    /// SHA-1.
    Sha1,
    /// SHA-256 (o do índice e dos links diretos).
    Sha256,
    /// SHA-512 (o do Modrinth).
    Sha512,
}

impl HashFormat {
    /// Todos, do mais fraco ao mais forte (a ordem de preferência do packwiz).
    pub const ALL: [Self; 5] = [
        Self::Murmur2,
        Self::Md5,
        Self::Sha1,
        Self::Sha256,
        Self::Sha512,
    ];

    /// Nome gravado em `hash-format`.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Murmur2 => "murmur2",
            Self::Md5 => "md5",
            Self::Sha1 => "sha1",
            Self::Sha256 => "sha256",
            Self::Sha512 => "sha512",
        }
    }

    /// Lê o nome, sem diferenciar maiúsculas (como o `GetHashImpl` do packwiz).
    pub fn parse(name: &str) -> Result<Self> {
        Self::ALL
            .into_iter()
            .find(|format| format.as_str().eq_ignore_ascii_case(name))
            .ok_or_else(|| Error::UnknownHashFormat(name.chars().take(64).collect()))
    }

    /// O mais forte entre os formatos dados (o packwiz confere downloads com ele).
    #[must_use]
    pub fn strongest(formats: impl IntoIterator<Item = Self>) -> Option<Self> {
        formats.into_iter().max()
    }
}

impl fmt::Display for HashFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Bytes que a CurseForge descarta antes do murmur2.
fn is_curseforge_whitespace(byte: u8) -> bool {
    matches!(byte, 9 | 10 | 13 | 32)
}

const M: u32 = 0x5bd1_e995;

/// Estado incremental do `MurmurHash2` com o tamanho já conhecido.
struct Murmur2 {
    hash: u32,
    tail: [u8; 4],
    tail_len: usize,
}

impl Murmur2 {
    fn new(seed: u32, length: u32) -> Self {
        Self {
            hash: seed ^ length,
            tail: [0; 4],
            tail_len: 0,
        }
    }

    fn mix(&mut self, block: [u8; 4]) {
        let mut k = u32::from_le_bytes(block);
        k = k.wrapping_mul(M);
        k ^= k >> 24;
        k = k.wrapping_mul(M);
        self.hash = self.hash.wrapping_mul(M) ^ k;
    }

    fn push(&mut self, byte: u8) {
        self.tail[self.tail_len] = byte;
        self.tail_len += 1;
        if self.tail_len == 4 {
            self.mix(self.tail);
            self.tail_len = 0;
        }
    }

    fn finish(mut self) -> u32 {
        let mut h = self.hash;
        if self.tail_len == 3 {
            h ^= u32::from(self.tail[2]) << 16;
        }
        if self.tail_len >= 2 {
            h ^= u32::from(self.tail[1]) << 8;
        }
        if self.tail_len >= 1 {
            h ^= u32::from(self.tail[0]);
            h = h.wrapping_mul(M);
        }
        h ^= h >> 13;
        h = h.wrapping_mul(M);
        h ^= h >> 15;
        self.hash = h;
        self.hash
    }
}

/// `MurmurHash2` de 32 bits (semente dada), sobre todos os bytes.
#[must_use]
pub fn murmur2(data: &[u8], seed: u32) -> u32 {
    // O tamanho entra na semente; dados com mais de 4 GiB repetem o corte do `uint32` do Go.
    #[allow(clippy::cast_possible_truncation)]
    let mut state = Murmur2::new(seed, data.len() as u32);
    for &byte in data {
        state.push(byte);
    }
    state.finish()
}

/// Impressão digital da CurseForge (murmur2 sem espaços em branco, semente 1) de dados em
/// memória.
#[must_use]
pub fn curseforge_fingerprint(data: &[u8]) -> u32 {
    let normalized: Vec<u8> = data
        .iter()
        .copied()
        .filter(|&byte| !is_curseforge_whitespace(byte))
        .collect();
    murmur2(&normalized, 1)
}

/// Impressão digital da CurseForge de um arquivo, sem carregá-lo inteiro na memória: uma
/// passada conta os bytes que entram, a segunda calcula.
pub fn curseforge_fingerprint_reader<R: Read + Seek>(reader: &mut R) -> io::Result<u32> {
    let start = reader.stream_position()?;
    let mut buffer = vec![0_u8; 64 * 1024];
    let mut kept: u64 = 0;
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        kept += buffer[..read]
            .iter()
            .filter(|&&byte| !is_curseforge_whitespace(byte))
            .count() as u64;
    }
    reader.seek(SeekFrom::Start(start))?;
    // O Go converte o tamanho para `uint32` (corta acima de 4 GiB); aqui é igual.
    #[allow(clippy::cast_possible_truncation)]
    let mut state = Murmur2::new(1, kept as u32);
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        for &byte in &buffer[..read] {
            if !is_curseforge_whitespace(byte) {
                state.push(byte);
            }
        }
    }
    Ok(state.finish())
}

/// Calcula o hash de dados em memória, no texto que o packwiz grava.
#[must_use]
pub fn hash_bytes(format: HashFormat, data: &[u8]) -> String {
    match format {
        HashFormat::Murmur2 => curseforge_fingerprint(data).to_string(),
        HashFormat::Md5 => hex::encode(Md5::digest(data)),
        HashFormat::Sha1 => hex::encode(Sha1::digest(data)),
        HashFormat::Sha256 => hex::encode(Sha256::digest(data)),
        HashFormat::Sha512 => hex::encode(Sha512::digest(data)),
    }
}

/// Calcula o hash lendo aos pedaços (o murmur2 precisa voltar ao início: por isso `Seek`).
pub fn hash_reader<R: Read + Seek>(format: HashFormat, reader: &mut R) -> io::Result<String> {
    fn digest<D: sha2::Digest, R: Read>(reader: &mut R) -> io::Result<String> {
        let mut hasher = D::new();
        let mut buffer = vec![0_u8; 64 * 1024];
        loop {
            let read = reader.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
        }
        Ok(hex::encode(hasher.finalize()))
    }
    match format {
        HashFormat::Murmur2 => Ok(curseforge_fingerprint_reader(reader)?.to_string()),
        HashFormat::Md5 => digest::<Md5, _>(reader),
        HashFormat::Sha1 => digest::<Sha1, _>(reader),
        HashFormat::Sha256 => digest::<Sha256, _>(reader),
        HashFormat::Sha512 => digest::<Sha512, _>(reader),
    }
}

/// Compara um hash calculado com o gravado, como o packwiz: hexadecimal sem diferenciar
/// maiúsculas; murmur2 como número.
#[must_use]
pub fn hash_matches(format: HashFormat, expected: &str, actual: &str) -> bool {
    match format {
        HashFormat::Murmur2 => {
            matches!((expected.trim().parse::<u64>(), actual.trim().parse::<u64>()), (Ok(a), Ok(b)) if a == b)
        }
        _ => expected.trim().eq_ignore_ascii_case(actual.trim()),
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn nomes_e_ordem_de_forca() {
        for format in HashFormat::ALL {
            assert_eq!(HashFormat::parse(format.as_str()).unwrap(), format);
            assert_eq!(format.to_string(), format.as_str());
        }
        assert_eq!(HashFormat::parse("SHA512").unwrap(), HashFormat::Sha512);
        assert!(matches!(
            HashFormat::parse("crc32"),
            Err(Error::UnknownHashFormat(name)) if name == "crc32"
        ));
        assert_eq!(
            HashFormat::strongest([HashFormat::Sha1, HashFormat::Murmur2, HashFormat::Md5]),
            Some(HashFormat::Sha1)
        );
        assert_eq!(HashFormat::strongest([]), None);
    }

    #[test]
    fn vetores_conhecidos() {
        // Vetores dos algoritmos padrão (FIPS 180, RFC 1321).
        assert_eq!(
            hash_bytes(HashFormat::Sha256, b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            hash_bytes(HashFormat::Sha1, b"abc"),
            "a9993e364706816aba3e25717850c26c9cd0d89d"
        );
        assert_eq!(
            hash_bytes(HashFormat::Md5, b"abc"),
            "900150983cd24fb0d6963f7d28e17f72"
        );
        assert!(hash_bytes(HashFormat::Sha512, b"abc").starts_with("ddaf35a193617aba"));
    }

    #[test]
    fn leitura_aos_pedacos_igual_a_memoria() {
        let data: Vec<u8> = (0..200_000_u32)
            .map(|i| u8::try_from(i.wrapping_mul(2_654_435_761) >> 24).unwrap_or(0))
            .collect();
        for format in HashFormat::ALL {
            let mut cursor = Cursor::new(&data);
            assert_eq!(
                hash_reader(format, &mut cursor).unwrap(),
                hash_bytes(format, &data),
                "{format}"
            );
        }
    }

    #[test]
    fn comparacao_de_hashes() {
        assert!(hash_matches(HashFormat::Sha1, "ABC", "abc"));
        assert!(!hash_matches(HashFormat::Sha1, "abc", "abd"));
        assert!(hash_matches(HashFormat::Murmur2, "0012", "12"));
        assert!(!hash_matches(HashFormat::Murmur2, "x", "x"));
    }

    #[test]
    fn murmur2_ignora_espacos_da_curseforge() {
        assert_eq!(
            curseforge_fingerprint(b"a b\tc\r\nd"),
            curseforge_fingerprint(b"abcd")
        );
        assert_ne!(curseforge_fingerprint(b"abcd"), murmur2(b"abcd", 0));
    }
}
