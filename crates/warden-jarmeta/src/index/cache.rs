//! Cache dos índices por hash do jar: `cache/jarindex/<sha256>.json` (ARCHITECTURE §9.6).
//!
//! O índice só depende dos bytes do jar, então o hash basta como chave. Arquivo ilegível, de
//! outro [`INDEX_FORMAT`] ou de outro hash é ignorado (o índice é refeito e regravado); falhar ao
//! gravar não impede o diagnóstico.
//!
//! Por isso as gravações do cache são diretas, sem `fsync` nem arquivo temporário renomeado
//! ([`warden_core::atomic_write`]): um arquivo pela metade (queda no meio, leitura junto da
//! gravação) não é um JSON nem um hash válido, e é ignorado como qualquer outro. No Windows, a
//! gravação atômica de centenas de arquivos novos esperava o antivírus soltar cada temporário
//! para renomear (A-04: 5,6 s para indexar 300 jars sem cache).
//!
//! Calcular o hash obriga a ler o jar inteiro (um pack de 400 mods passa de 1 GB), então o
//! cache também guarda o hash de cada arquivo já visto, em `cache/jarindex/arquivos/<identidade>`:
//! a identidade ([`file_identity`]) junta o tamanho, a data de modificação e o diretório central
//! do zip, que traz o nome, o tamanho e o CRC-32 de cada entrada. Com o arquivo igual, a
//! segunda passagem lê só o fim de cada jar; qualquer mudança no arquivo muda a identidade e o
//! jar é lido de novo.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use sha2::{Digest, Sha256};

use super::build::hex;
use super::{INDEX_FORMAT, JarIndex};

/// Nome da pasta do cache dentro de `cache/` da pasta de dados.
pub const CACHE_DIR_NAME: &str = "jarindex";

/// Subpasta com o hash de cada arquivo já visto, pela identidade dele.
pub const FILES_DIR_NAME: &str = "arquivos";

/// Prefixo da identidade de um arquivo: muda quando o cálculo muda, e as identidades antigas
/// deixam de valer.
const IDENTITY_FORMAT: &[u8] = b"warden-jarindex-arquivo-1\0";

/// Assinatura do fim do diretório central do zip.
const EOCD_SIGNATURE: [u8; 4] = *b"PK\x05\x06";
/// Tamanho do registro de fim do diretório central sem o comentário.
const EOCD_LEN: u64 = 22;
/// Maior diretório central lido para a identidade (um jar com 500 mil entradas tem ~40 MB).
const MAX_CENTRAL_DIRECTORY: u64 = 64 * 1024 * 1024;

/// Pasta do cache dos índices.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JarIndexCache {
    dir: PathBuf,
}

impl JarIndexCache {
    /// Cache na pasta dada (normalmente `<dados>/cache/jarindex`). A pasta é criada na primeira
    /// gravação.
    #[must_use]
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    /// Cache em `<cache>/jarindex`.
    #[must_use]
    pub fn in_cache_dir(cache_dir: &Path) -> Self {
        Self::new(cache_dir.join(CACHE_DIR_NAME))
    }

    /// A pasta.
    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Arquivo do índice de um jar; `None` se o texto não é um SHA-256 em hexadecimal (nunca
    /// vira caminho).
    #[must_use]
    pub fn path_for(&self, sha256: &str) -> Option<PathBuf> {
        is_sha256(sha256).then(|| self.dir.join(format!("{sha256}.json")))
    }

    /// O índice guardado, se existir, for legível, do formato atual e do mesmo hash.
    #[must_use]
    pub fn load(&self, sha256: &str) -> Option<JarIndex> {
        let bytes = std::fs::read(self.path_for(sha256)?).ok()?;
        let index: JarIndex = serde_json::from_slice(&bytes).ok()?;
        (index.format == INDEX_FORMAT && index.sha256 == sha256 && !index.jars.is_empty())
            .then_some(index)
    }

    /// Grava o índice (direto, sem gravação atômica; veja o início do módulo).
    ///
    /// # Errors
    ///
    /// Falha de disco ao criar a pasta ou gravar, ou hash inválido no índice.
    pub fn store(&self, index: &JarIndex) -> Result<(), warden_core::CoreError> {
        let path = self.path_for(&index.sha256).ok_or_else(|| {
            warden_core::CoreError::io(
                "gravar",
                &self.dir,
                std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "o índice não tem um SHA-256 válido",
                ),
            )
        })?;
        std::fs::create_dir_all(&self.dir)
            .map_err(|e| warden_core::CoreError::io("criar a pasta", &self.dir, e))?;
        let json = serde_json::to_vec(index).map_err(|e| {
            warden_core::CoreError::io("gravar", &path, std::io::Error::other(e.to_string()))
        })?;
        write_cache_file(&path, &json)
    }

    /// O SHA-256 guardado para o arquivo com esta identidade ([`file_identity`]).
    #[must_use]
    pub fn known_sha256(&self, identity: &str) -> Option<String> {
        let path = self.identity_path(identity)?;
        let text = std::fs::read_to_string(path).ok()?;
        let sha = text.trim();
        is_sha256(sha).then(|| sha.to_owned())
    }

    /// Guarda o SHA-256 do arquivo com esta identidade (direto, como [`Self::store`]).
    ///
    /// # Errors
    ///
    /// Falha de disco ao criar a pasta ou gravar, ou identidade ou hash inválidos.
    pub fn remember_sha256(
        &self,
        identity: &str,
        sha256: &str,
    ) -> Result<(), warden_core::CoreError> {
        let dir = self.dir.join(FILES_DIR_NAME);
        let path = self
            .identity_path(identity)
            .filter(|_| is_sha256(sha256))
            .ok_or_else(|| {
                warden_core::CoreError::io(
                    "gravar",
                    &dir,
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "identidade ou SHA-256 inválidos",
                    ),
                )
            })?;
        std::fs::create_dir_all(&dir)
            .map_err(|e| warden_core::CoreError::io("criar a pasta", &dir, e))?;
        write_cache_file(&path, sha256.as_bytes())
    }

    fn identity_path(&self, identity: &str) -> Option<PathBuf> {
        is_sha256(identity).then(|| self.dir.join(FILES_DIR_NAME).join(identity))
    }
}

/// Grava um arquivo do cache: o que ficar pela metade é recusado na leitura.
fn write_cache_file(path: &Path, bytes: &[u8]) -> Result<(), warden_core::CoreError> {
    std::fs::write(path, bytes).map_err(|e| warden_core::CoreError::io("gravar", path, e))
}

/// Identidade de um jar no disco, sem ler o arquivo inteiro: SHA-256 do tamanho, da data de
/// modificação e do diretório central do zip (com o registro de fim dele). `None` quando não
/// dá para calcular (arquivo ilegível, sem data, que não termina num zip ou zip64): o jar é
/// lido inteiro, como sem esta identidade.
#[must_use]
pub fn file_identity(path: &Path) -> Option<String> {
    let mut file = File::open(path).ok()?;
    let metadata = file.metadata().ok()?;
    let len = metadata.len();
    let modified = metadata
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_nanos();
    // O registro de fim fica nos últimos 22 bytes, mais o comentário (até 64 KiB).
    let tail_len = len.min(EOCD_LEN + u64::from(u16::MAX));
    if tail_len < EOCD_LEN {
        return None;
    }
    let tail_start = len - tail_len;
    file.seek(SeekFrom::Start(tail_start)).ok()?;
    let mut tail = vec![0u8; usize::try_from(tail_len).ok()?];
    file.read_exact(&mut tail).ok()?;
    let eocd = find_eocd(&tail)?;
    let record = tail.get(eocd..)?;
    let entries = u16_at(record, 10)?;
    let cd_size = u32_at(record, 12)?;
    let cd_offset = u32_at(record, 16)?;
    if entries == u16::MAX || cd_size == u32::MAX || cd_offset == u32::MAX {
        return None; // zip64: os campos de 32 bits não dizem onde está o diretório.
    }
    let cd_size = u64::from(cd_size);
    let eocd_at = tail_start + u64::try_from(eocd).ok()?;
    if cd_size > MAX_CENTRAL_DIRECTORY || cd_size > eocd_at {
        return None;
    }
    // O diretório termina onde o registro de fim começa (vale também para zip com dados antes).
    file.seek(SeekFrom::Start(eocd_at - cd_size)).ok()?;
    let mut central = vec![0u8; usize::try_from(cd_size).ok()?];
    file.read_exact(&mut central).ok()?;
    let mut hasher = Sha256::new();
    hasher.update(IDENTITY_FORMAT);
    hasher.update(len.to_le_bytes());
    hasher.update(modified.to_le_bytes());
    hasher.update(&central);
    hasher.update(record);
    Some(hex(&hasher.finalize()))
}

/// Posição do registro de fim do diretório central em `tail` (o fim do arquivo): o último com a
/// assinatura e um comentário que vai exatamente até o fim.
fn find_eocd(tail: &[u8]) -> Option<usize> {
    let last = tail.len().checked_sub(usize::try_from(EOCD_LEN).ok()?)?;
    (0..=last).rev().find(|&at| {
        tail.get(at..at + 4) == Some(EOCD_SIGNATURE.as_slice())
            && u16_at(tail, at + 20).is_some_and(|comment| usize::from(comment) == last - at)
    })
}

fn u16_at(bytes: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(bytes.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

/// 64 dígitos hexadecimais minúsculos.
fn is_sha256(text: &str) -> bool {
    text.len() == 64 && text.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::IndexedJar;

    fn index(sha256: &str) -> JarIndex {
        JarIndex {
            format: INDEX_FORMAT,
            sha256: sha256.to_owned(),
            jars: vec![IndexedJar {
                path: String::new(),
                parent: None,
                mods: vec![],
                artifact: None,
                artifact_version: None,
                packages: vec!["a.b".into()],
            }],
            mixin_configs: vec![],
        }
    }

    #[test]
    fn grava_le_e_recusa_o_que_nao_serve() {
        let dir = tempfile::tempdir().unwrap();
        let cache = JarIndexCache::in_cache_dir(dir.path());
        let sha = "a".repeat(64);
        assert_eq!(cache.load(&sha), None);
        cache.store(&index(&sha)).unwrap();
        assert_eq!(cache.load(&sha), Some(index(&sha)));

        // Outro formato, outro hash dentro do arquivo e JSON quebrado: refaz.
        let path = cache.path_for(&sha).unwrap();
        let mut old = index(&sha);
        old.format = INDEX_FORMAT + 1;
        std::fs::write(&path, serde_json::to_vec(&old).unwrap()).unwrap();
        assert_eq!(cache.load(&sha), None);
        std::fs::write(&path, serde_json::to_vec(&index(&"b".repeat(64))).unwrap()).unwrap();
        assert_eq!(cache.load(&sha), None);
        std::fs::write(&path, b"{").unwrap();
        assert_eq!(cache.load(&sha), None);
    }

    /// As gravações do cache não são atômicas: qualquer começo de um arquivo gravado (queda no
    /// meio da gravação) tem de ser recusado na leitura.
    #[test]
    fn arquivo_pela_metade_e_recusado() {
        let dir = tempfile::tempdir().unwrap();
        let cache = JarIndexCache::in_cache_dir(dir.path());
        let sha = "c".repeat(64);
        let identity = "d".repeat(64);
        cache.store(&index(&sha)).unwrap();
        cache.remember_sha256(&identity, &sha).unwrap();
        assert_eq!(cache.load(&sha), Some(index(&sha)));
        assert_eq!(cache.known_sha256(&identity), Some(sha.clone()));

        let index_path = cache.path_for(&sha).unwrap();
        let full = std::fs::read(&index_path).unwrap();
        for len in 0..full.len() {
            std::fs::write(&index_path, &full[..len]).unwrap();
            assert_eq!(
                cache.load(&sha),
                None,
                "índice com {len} de {} bytes",
                full.len()
            );
        }
        let identity_path = cache.dir().join(FILES_DIR_NAME).join(&identity);
        for len in 0..sha.len() {
            std::fs::write(&identity_path, &sha.as_bytes()[..len]).unwrap();
            assert_eq!(
                cache.known_sha256(&identity),
                None,
                "hash com {len} caracteres"
            );
        }
    }

    #[test]
    fn hash_invalido_nunca_vira_caminho() {
        let cache = JarIndexCache::new(PathBuf::from("x"));
        for bad in [
            "",
            "../../a",
            &"A".repeat(64),
            &"a".repeat(63),
            &"g".repeat(64),
        ] {
            assert_eq!(cache.path_for(bad), None, "{bad}");
            assert_eq!(cache.load(bad), None);
        }
        assert!(cache.store(&index("../x")).is_err());
    }
}
