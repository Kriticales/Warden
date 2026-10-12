//! Crate `warden-github`: o cliente da API do GitHub do Warden (UI-03; ADR-0059).
//!
//! - [`device`]: o login pelo *device flow* de um GitHub App ("Entrar com o GitHub"): pedir o
//!   código, esperar a aprovação respeitando o `interval` e o `slow_down` (com cancelamento) e
//!   renovar o token pelo *refresh token*.
//! - [`GithubApi`]: o que o relato precisa, sempre no repositório do Warden: a conta
//!   (`GET /user`), as labels, a branch órfã dos prints, o envio de arquivos pela Contents API
//!   e a criação da issue.
//! - [`Endpoints`]: os endereços e o *client ID*. São constantes; só em build de debug os
//!   testes trocam por um servidor simulado (`WARDEN_API_BASE_GITHUB`,
//!   `WARDEN_LOGIN_BASE_GITHUB`, `WARDEN_GITHUB_CLIENT_ID`).
//!
//! Limites: esta crate não guarda nada (quem chama grava os tokens no cofre) e não conhece a
//! interface. Tokens, *refresh tokens* e o `device_code` circulam como [`SecretString`]: nunca
//! vão para registros, mensagens de erro nem `Debug`.

mod api;
pub mod device;
mod endpoints;
mod error;

pub use api::{CreatedIssue, GithubApi, Label, User};
pub use device::{DeviceCode, TokenSet, WaitEvent};
pub use endpoints::{
    API_BASE_ENV, API_BASE_URL, ATTACHMENTS_BRANCH, ATTACHMENTS_DIR, CLIENT_ID, CLIENT_ID_ENV,
    Endpoints, LOGIN_BASE_ENV, LOGIN_BASE_URL, REPOSITORY_NAME, REPOSITORY_OWNER,
};
pub use error::{Error, GithubErrorCode, Result};
pub use secrecy::{ExposeSecret, SecretString};
