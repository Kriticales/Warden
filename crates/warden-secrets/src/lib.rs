//! Crate `warden-secrets`: onde ficam as chaves e tokens do usuário (ARCHITECTURE §14;
//! ADR-0069, que substitui a ADR-0025).
//!
//! - [`SecretStore`] é o contrato de um armazenamento. Em produção há um só: o cofre do sistema
//!   ([`KeyringStore`]: Gerenciador de Credenciais no Windows, Secret Service no Linux). O
//!   antigo modo "arquivo `.env`" saiu; quem o usava tem as chaves levadas para o cofre ao
//!   abrir o app ([`Secrets::migrate_legacy_env`]).
//! - [`TestFileStore`] é o cofre de teste em arquivo, que existe **só em build de debug**:
//!   `cargo xtask dev`, os testes e os E2E o usam no lugar do cofre do sistema, para nunca
//!   tocar no cofre real do dono (`WARDEN_SECRET_BACKEND=file:<pasta>`, ADR-0048).
//! - [`Secrets`] junta tudo: estado das chaves, gravar, remover, ler na hora do uso (com o
//!   recuo de desenvolvimento por variável de ambiente, só em debug), o final da chave para a
//!   tela (`••••xxxx`) e a migração do `.env` antigo, que só apaga o arquivo depois de conferir
//!   que o cofre gravou cada chave.
//!
//! Os valores circulam como [`SecretString`] (o `Debug` mostra `[REDACTED]`) e nunca voltam
//! para a interface: ela só conhece [`SecretsStatus`]. Nenhuma mensagem de erro desta crate
//! contém o valor de um segredo.

mod error;
mod keyring;
mod kind;
mod legacy_env;
mod manager;
mod store;
#[cfg(debug_assertions)]
mod testfile;

pub use error::{SecretsError, SecretsErrorCode};
pub use keyring::KeyringStore;
pub use kind::{SecretKind, SecretsStatus};
pub use manager::{
    BACKEND_OVERRIDE_ENV, BackendOverride, EnvLookup, EnvMigration, Secrets, SecretsConfig,
};
pub use secrecy::{ExposeSecret, SecretString};
pub use store::{SecretStore, normalize_secret};
#[cfg(debug_assertions)]
pub use testfile::TestFileStore;
