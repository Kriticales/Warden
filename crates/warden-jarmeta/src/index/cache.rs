//! Cache dos índices por hash do jar: `cache/jarindex/<sha256>.json` (ARCHITECTURE §9.6).
//!
//! O índice só depende dos bytes do jar, então o hash basta como chave. Arquivo ilegível, de
//! outro [`INDEX_FORMAT`] ou de outro hash é ignorado (o índice é refeito e regravado); falhar ao
//! gravar não impede o diagnóstico.

use std::path::{Path, PathBuf};

use super::{INDEX_FORMAT, JarIndex};

/// Nome da pasta do cache dentro de `cache/` da pasta de dados.
pub const CACHE_DIR_NAME: &str = "jarindex";

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

    /// Grava o índice (atomicamente).
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
        warden_core::atomic_write(&path, &json)
    }
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
