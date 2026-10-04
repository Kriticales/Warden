//! Cofre do sistema: Gerenciador de Credenciais do Windows ou Secret Service do Linux, pelo
//! `keyring-core` (sucessor mantido do `keyring` 3) e os armazenamentos nativos de cada
//! plataforma.
//!
//! No Windows, cada segredo é uma credencial genérica `<conta>.<serviço>` (ex.:
//! `curseforge-api-key.dev.kriticales.warden`), com persistência `Local`: fica só neste
//! computador e não vai junto com perfis móveis.

use std::collections::HashMap;
use std::sync::Arc;

use keyring_core::{CredentialStore, Entry, Error as KeyringError};
use secrecy::{ExposeSecret as _, SecretString};

use crate::error::SecretsError;
use crate::kind::{BackendKind, SecretKind};
use crate::store::SecretStore;

/// Serviço do Warden no cofre (ARCHITECTURE §14).
pub(crate) const SERVICE: &str = "dev.kriticales.warden";

/// O cofre do sistema.
pub struct KeyringStore {
    store: Arc<CredentialStore>,
    service: String,
}

impl std::fmt::Debug for KeyringStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KeyringStore")
            .field("vendor", &self.store.vendor())
            .field("service", &self.service)
            .finish()
    }
}

impl KeyringStore {
    /// Cofre do sistema com o serviço do Warden.
    pub fn system() -> Result<Self, SecretsError> {
        Self::with_service(SERVICE)
    }

    /// Cofre do sistema com outro nome de serviço (só o teste com o cofre real usa, para não
    /// tocar nas chaves do Warden).
    pub fn with_service(service: &str) -> Result<Self, SecretsError> {
        Ok(Self {
            store: platform_store()?,
            service: service.to_owned(),
        })
    }

    fn entry(&self, kind: SecretKind) -> Result<Entry, SecretsError> {
        let modifiers = persistence_modifiers();
        let entry = if modifiers.is_empty() {
            self.store.build(&self.service, kind.account(), None)
        } else {
            self.store
                .build(&self.service, kind.account(), Some(&modifiers))
        };
        entry.map_err(|error| map_error(kind, &error))
    }
}

impl SecretStore for KeyringStore {
    fn backend(&self) -> BackendKind {
        BackendKind::Keyring
    }

    fn get(&self, kind: SecretKind) -> Result<Option<SecretString>, SecretsError> {
        match self.entry(kind)?.get_password() {
            Ok(value) if value.is_empty() => Ok(None),
            Ok(value) => Ok(Some(SecretString::from(value))),
            Err(KeyringError::NoEntry) => Ok(None),
            Err(error) => Err(map_error(kind, &error)),
        }
    }

    fn set(&self, kind: SecretKind, value: &SecretString) -> Result<(), SecretsError> {
        self.entry(kind)?
            .set_password(value.expose_secret())
            .map_err(|error| map_error(kind, &error))
    }

    fn remove(&self, kind: SecretKind) -> Result<(), SecretsError> {
        match self.entry(kind)?.delete_credential() {
            Ok(()) | Err(KeyringError::NoEntry) => Ok(()),
            Err(error) => Err(map_error(kind, &error)),
        }
    }
}

#[cfg(windows)]
fn persistence_modifiers() -> HashMap<&'static str, &'static str> {
    HashMap::from([("persistence", "Local")])
}

#[cfg(not(windows))]
fn persistence_modifiers() -> HashMap<&'static str, &'static str> {
    HashMap::new()
}

#[cfg(windows)]
fn platform_store() -> Result<Arc<CredentialStore>, SecretsError> {
    let store = windows_native_keyring_store::Store::new()
        .map_err(|error| map_error(SecretKind::Curseforge, &error))?;
    Ok(store)
}

#[cfg(target_os = "linux")]
fn platform_store() -> Result<Arc<CredentialStore>, SecretsError> {
    let store = zbus_secret_service_keyring_store::Store::new()
        .map_err(|error| map_error(SecretKind::Curseforge, &error))?;
    Ok(store)
}

#[cfg(not(any(windows, target_os = "linux")))]
fn platform_store() -> Result<Arc<CredentialStore>, SecretsError> {
    Err(SecretsError::VaultUnavailable {
        kind: SecretKind::Curseforge,
        message: "plataforma sem cofre suportado".to_owned(),
    })
}

/// Converte o erro do cofre sem copiar dados da credencial (os erros `BadEncoding` e
/// `BadDataFormat` carregam os bytes lidos, que podem ser o segredo).
fn map_error(kind: SecretKind, error: &KeyringError) -> SecretsError {
    match error {
        KeyringError::NoStorageAccess(platform) => SecretsError::VaultUnavailable {
            kind,
            message: platform.to_string(),
        },
        KeyringError::NoDefaultStore => SecretsError::VaultUnavailable {
            kind,
            message: "nenhum cofre configurado".to_owned(),
        },
        KeyringError::PlatformFailure(platform) => SecretsError::VaultFailed {
            kind,
            message: platform.to_string(),
        },
        KeyringError::BadEncoding(_) => SecretsError::VaultFailed {
            kind,
            message: "o valor guardado não é texto".to_owned(),
        },
        KeyringError::BadDataFormat(_, _) => SecretsError::VaultFailed {
            kind,
            message: "o valor guardado está num formato inesperado".to_owned(),
        },
        KeyringError::BadStoreFormat(message) => SecretsError::VaultFailed {
            kind,
            message: message.clone(),
        },
        KeyringError::TooLong(name, limit) => SecretsError::VaultFailed {
            kind,
            message: format!("{name} passa do limite de {limit}"),
        },
        KeyringError::Invalid(name, reason) => SecretsError::VaultFailed {
            kind,
            message: format!("{name}: {reason}"),
        },
        KeyringError::Ambiguous(entries) => SecretsError::VaultFailed {
            kind,
            message: format!("{} credenciais com o mesmo nome", entries.len()),
        },
        other => SecretsError::VaultFailed {
            kind,
            message: format!("{:?}", std::mem::discriminant(other)),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn erros_com_dados_nao_copiam_os_bytes() {
        let secret_bytes = b"$2a$10$segredo".to_vec();
        let error = map_error(
            SecretKind::Curseforge,
            &KeyringError::BadEncoding(secret_bytes),
        );
        let text = format!("{error} {error:?}");
        assert!(!text.contains("segredo"), "{text}");
        assert!(matches!(error, SecretsError::VaultFailed { .. }));
    }

    #[test]
    fn sem_acesso_vira_cofre_indisponivel() {
        let error = map_error(SecretKind::Gemini, &KeyringError::NoDefaultStore);
        assert!(matches!(error, SecretsError::VaultUnavailable { .. }));
    }

    /// Critério 3 da F0-05 com o cofre real do Windows: gravar, "reiniciar" (outro objeto,
    /// como o app faz ao abrir de novo) e ler; apagar no fim. Usa um serviço próprio do teste,
    /// nunca o `dev.kriticales.warden`. Só roda na CI Windows (QUALITY §13.6).
    #[test]
    #[ignore = "cofre-real"]
    fn f0_05_ca3_cofre_real_mantem_a_chave_entre_aberturas() {
        let service = format!("{SERVICE}.teste-{}", ulid::Ulid::generate());
        let value = SecretString::from(format!("teste-{}", ulid::Ulid::generate()));
        {
            let store = KeyringStore::with_service(&service).unwrap();
            store.set(SecretKind::Gemini, &value).unwrap();
        }
        let reopened = KeyringStore::with_service(&service).unwrap();
        let read = reopened.get(SecretKind::Gemini).unwrap().unwrap();
        assert_eq!(read.expose_secret(), value.expose_secret());
        reopened.purge().unwrap();
        assert!(reopened.get(SecretKind::Gemini).unwrap().is_none());
    }
}
