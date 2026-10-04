//! Crate `warden-secrets`: onde ficam as chaves e tokens do usuário (ARCHITECTURE §14;
//! ADR-0025, que substitui a ADR-0017).
//!
//! - [`SecretStore`] é o contrato de um armazenamento. Há duas implementações de produção,
//!   escolhidas pelo usuário em Configurações → Chaves e contas: o cofre do sistema
//!   ([`KeyringStore`], padrão: Gerenciador de Credenciais no Windows, Secret Service no Linux)
//!   e o arquivo `.env` na pasta de configuração ([`EnvFileStore`]).
//! - [`TestFileStore`] é o cofre de teste em arquivo, que existe **só em build de debug**:
//!   `cargo xtask dev`, os testes e os E2E o usam no lugar do cofre do sistema, para nunca
//!   tocar no cofre real do dono (`WARDEN_SECRET_BACKEND=file:<pasta>`, ADR-0048).
//! - [`Secrets`] junta tudo: estado das chaves, gravar, remover, ler na hora do uso (com o
//!   recuo de desenvolvimento por variável de ambiente, só em debug) e a troca de modo, que
//!   **move** as chaves sem perder nenhuma se algo falhar no meio.
//!
//! Os valores circulam como [`SecretString`] (o `Debug` mostra `[REDACTED]`) e nunca voltam
//! para a interface: ela só conhece [`SecretsStatus`]. Nenhuma mensagem de erro desta crate
//! contém o valor de um segredo.

mod envfile;
mod error;
mod keyring;
mod kind;
mod manager;
mod store;
#[cfg(debug_assertions)]
mod testfile;

pub use envfile::EnvFileStore;
pub use error::{SecretsError, SecretsErrorCode};
pub use keyring::KeyringStore;
pub use kind::{BackendKind, SecretKind, SecretsStatus};
pub use manager::{
    BACKEND_OVERRIDE_ENV, BackendOverride, EnvLookup, Secrets, SecretsConfig, SwitchReport,
};
pub use secrecy::{ExposeSecret, SecretString};
pub use store::{SecretStore, normalize_secret};
#[cfg(debug_assertions)]
pub use testfile::TestFileStore;
