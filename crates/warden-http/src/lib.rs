//! Crate `warden-http`: o cliente HTTP único do Warden (ARCHITECTURE §3 e §17).
//!
//! - [`HttpClient`]: `reqwest` com `rustls`, User-Agent no formato pedido pelo Modrinth
//!   ([`user_agent`]), limitador por servidor ([`HostPolicy`]: taxa, simultâneas e bloqueio
//!   quando o servidor manda esperar), novas tentativas com espera exponencial só em
//!   requisições idempotentes, respeitando `Retry-After` e `X-Ratelimit-Reset`.
//! - Redirecionamentos seguidos pelo próprio cliente: cabeçalhos sensíveis só vão para o mesmo
//!   site e [`HttpClient::resolve_redirects`] acha o endereço final sem `Range` (o CDN da
//!   CurseForge recusa `Range` antes do redirecionamento).
//! - Downloads ([`HttpClient::download`]): streaming para `.part`, sha1, sha256, sha512 e
//!   murmur2 da CurseForge de uma vez ([`FileHashes`]), retomada com `Range`, conferência de
//!   hash e tamanho e renomeação no fim.
//! - Tempo injetável ([`Timer`], [`ManualTimer`]): os testes conferem as esperas sem dormir.
//!
//! Limites: esta crate não conhece APIs (Modrinth, CurseForge…), não guarda nada em disco além
//! do arquivo pedido e não lê segredos: quem chama passa a chave como `SecretString`. O índice
//! do cache de downloads (`cache/downloads/index.sqlite`) é da L-03, que usa [`FileHashes`].

mod client;
mod config;
mod download;
mod error;
mod hashes;
pub mod headers;
mod limiter;
mod timer;

pub use client::{HttpClient, Request, Response, parse_url, same_site};
pub use config::{
    CONTACT_URL, CURSEFORGE_API_HOST, HostPolicy, HttpConfig, MODRINTH_API_HOST, nonzero,
    user_agent,
};
pub use download::{DownloadRequest, Downloaded, PART_SUFFIX, part_path};
pub use error::{Error, HttpErrorCode, Result};
pub use hashes::{ExpectedHash, FileHashes};
pub use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
pub use reqwest::{Method, StatusCode};
pub use timer::{ManualTimer, Sleep, SystemTimer, Timer};
pub use url::Url;
