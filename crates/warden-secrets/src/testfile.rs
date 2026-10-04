//! Cofre de teste em arquivo: substitui o cofre do sistema **só em build de debug**
//! (`WARDEN_SECRET_BACKEND=file:<pasta>`; ADR-0048, QUALITY §13.6).
//!
//! Cada segredo é um arquivo `<conta>` na pasta, gravado de forma atômica. Não protege nada:
//! existe para que `cargo xtask dev`, os testes e os E2E nunca toquem no cofre real do dono,
//! no Windows do desenvolvimento e no Linux da CI (que não tem Secret Service).

use std::path::{Path, PathBuf};

use secrecy::{ExposeSecret as _, SecretString};
use warden_core::{CoreError, atomic_write_private};

use crate::error::SecretsError;
use crate::kind::{BackendKind, SecretKind};
use crate::store::SecretStore;

/// Cofre de teste numa pasta.
#[derive(Debug, Clone)]
pub struct TestFileStore {
    dir: PathBuf,
}

impl TestFileStore {
    /// Cofre de teste na pasta `dir` (criada na primeira gravação).
    #[must_use]
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    /// Pasta do cofre.
    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn file(&self, kind: SecretKind) -> PathBuf {
        self.dir.join(kind.account())
    }
}

impl SecretStore for TestFileStore {
    /// Faz o papel do cofre do sistema.
    fn backend(&self) -> BackendKind {
        BackendKind::Keyring
    }

    fn get(&self, kind: SecretKind) -> Result<Option<SecretString>, SecretsError> {
        let path = self.file(kind);
        match std::fs::read(&path) {
            Ok(bytes) => {
                let text = String::from_utf8(bytes).map_err(|_| SecretsError::VaultFailed {
                    kind,
                    message: "o valor guardado não é texto".to_owned(),
                })?;
                Ok((!text.is_empty()).then(|| SecretString::from(text)))
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(CoreError::io("ler", &path, error).into()),
        }
    }

    fn set(&self, kind: SecretKind, value: &SecretString) -> Result<(), SecretsError> {
        std::fs::create_dir_all(&self.dir)
            .map_err(|error| CoreError::io("criar a pasta", &self.dir, error))?;
        atomic_write_private(&self.file(kind), value.expose_secret().as_bytes())?;
        Ok(())
    }

    fn remove(&self, kind: SecretKind) -> Result<(), SecretsError> {
        let path = self.file(kind);
        match std::fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(CoreError::io("apagar", &path, error).into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grava_le_e_apaga() {
        let dir = tempfile::tempdir().unwrap();
        let store = TestFileStore::new(dir.path().join("cofre"));
        assert_eq!(store.backend(), BackendKind::Keyring);
        assert!(store.get(SecretKind::Github).unwrap().is_none());
        store
            .set(SecretKind::Github, &SecretString::from("ghp_x".to_owned()))
            .unwrap();
        assert_eq!(
            store
                .get(SecretKind::Github)
                .unwrap()
                .unwrap()
                .expose_secret(),
            "ghp_x"
        );
        assert!(store.dir().join("github-token").is_file());
        store.purge().unwrap();
        assert!(store.get(SecretKind::Github).unwrap().is_none());
        store.remove(SecretKind::Github).unwrap();
    }

    #[test]
    fn valor_que_nao_e_texto_e_erro() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("gemini-api-key"), [0xff, 0x00]).unwrap();
        let store = TestFileStore::new(dir.path());
        assert!(matches!(
            store.get(SecretKind::Gemini),
            Err(SecretsError::VaultFailed { .. })
        ));
    }
}
