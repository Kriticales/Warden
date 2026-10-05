//! Erros do domínio `curseforge` (ARCHITECTURE §5).
//!
//! Criado pela F0-05 só com `INTERNAL`; a P1-04 acrescentou os demais. As falhas de rede
//! chegam da `warden-http` e viram códigos da CurseForge, para a frase dizer "a CurseForge" em
//! vez de um servidor genérico; sem conexão, tempo esgotado e cancelamento continuam com os
//! códigos comuns do domínio `core`. As frases ficam em
//! `apps/desktop/src/i18n/errors/curseforge.ts`.
//!
//! Nenhuma mensagem leva a chave nem o corpo de uma resposta da CurseForge (termos, ARCHITECTURE
//! §16): a `warden-http` já não guarda o corpo dos erros de `*.curseforge.com` e `*.forgecdn.net`,
//! e os erros desta crate só levam IDs, nomes de arquivo e o código HTTP.

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};
use warden_core::{DomainCode, DomainError, error_chain};
use warden_http::HttpErrorCode;

/// Códigos do domínio `curseforge`. O código é contrato: renomear é mudança de contrato;
/// acrescentar é permitido (só acréscimo, ROADMAP §1).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CurseforgeErrorCode {
    /// Bug: invariante quebrada sem código específico.
    Internal,
    /// Não há chave da CurseForge configurada; nenhuma requisição foi feita.
    #[serde(rename = "CURSEFORGE_KEY_MISSING")]
    KeyMissing,
    /// A CurseForge recusou a chave (401 ou 403 da API).
    #[serde(rename = "CURSEFORGE_KEY_INVALID")]
    KeyInvalid,
    /// O projeto (mod, resource pack, shader…) não existe ou não está disponível.
    ModNotFound,
    /// O arquivo não existe ou não está disponível.
    FileNotFound,
    /// O autor não deixa apps de terceiros baixarem o arquivo (distribuição bloqueada).
    DistributionBlocked,
    /// A busca passou dos limites da API (página, 10 mil resultados, filtros demais).
    InvalidQuery,
    /// A CurseForge não encontrou o endereço pedido.
    NotFound,
    /// A CurseForge limitou as requisições (429) por mais tempo do que o Warden espera sozinho.
    RateLimited,
    /// A CurseForge está fora do ar ou com erro (5xx).
    Unavailable,
    /// A CurseForge recusou o pedido (4xx que não é de chave).
    RequestRejected,
    /// A CurseForge respondeu algo que o Warden não entende.
    InvalidResponse,
}

/// Erro das funções desta crate. O `Debug` é escrito à mão para a página do download manual
/// (montada com dados da API) não ir para os registros por um `?error`.
#[derive(thiserror::Error)]
pub enum Error {
    /// Sem chave configurada (nenhuma requisição saiu).
    #[error("sem chave da CurseForge")]
    KeyMissing,
    /// A API recusou a chave (ou a chave tem caracteres que não podem ir num cabeçalho).
    #[error("a CurseForge recusou a chave{}", status_suffix(*.status))]
    KeyInvalid {
        /// Código HTTP (401 ou 403); `None` quando a chave nem pôde ser mandada.
        status: Option<u16>,
    },
    /// Projeto inexistente.
    #[error("projeto {id} não encontrado na CurseForge")]
    ModNotFound {
        /// ID do projeto.
        id: u64,
    },
    /// Arquivo inexistente.
    #[error("arquivo {file_id} não encontrado na CurseForge")]
    FileNotFound {
        /// ID do projeto, quando conhecido.
        mod_id: Option<u64>,
        /// ID do arquivo.
        file_id: u64,
    },
    /// Distribuição por terceiros desligada pelo autor. A página do download manual fica fora
    /// da mensagem (que pode ir para os registros) e entra só nos "Detalhes técnicos".
    #[error(
        "a CurseForge não deixa apps de terceiros baixarem o arquivo {file_id} do projeto {mod_id}"
    )]
    DistributionBlocked {
        /// ID do projeto.
        mod_id: u64,
        /// ID do arquivo.
        file_id: u64,
        /// Nome do arquivo, quando conhecido.
        file_name: Option<String>,
        /// Página do arquivo no site, para o download manual.
        page_url: Option<String>,
    },
    /// Parâmetros de busca fora dos limites da API.
    #[error("busca inválida na CurseForge: {0}")]
    InvalidQuery(String),
    /// Falha da requisição.
    #[error(transparent)]
    Http(#[from] warden_http::Error),
    /// Resposta que não pôde ser usada (só o motivo, nunca o corpo).
    #[error("resposta inválida da CurseForge: {0}")]
    InvalidResponse(String),
    /// Invariante quebrada.
    #[error("erro interno: {0}")]
    Internal(String),
}

fn status_suffix(status: Option<u16>) -> String {
    status.map_or_else(
        || " (caracteres inválidos para um cabeçalho HTTP)".to_owned(),
        |status| format!(" (HTTP {status})"),
    )
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::KeyMissing => f.write_str("KeyMissing"),
            Self::KeyInvalid { status } => f
                .debug_struct("KeyInvalid")
                .field("status", status)
                .finish(),
            Self::ModNotFound { id } => f.debug_struct("ModNotFound").field("id", id).finish(),
            Self::FileNotFound { mod_id, file_id } => f
                .debug_struct("FileNotFound")
                .field("mod_id", mod_id)
                .field("file_id", file_id)
                .finish(),
            Self::DistributionBlocked {
                mod_id,
                file_id,
                file_name,
                page_url,
            } => f
                .debug_struct("DistributionBlocked")
                .field("mod_id", mod_id)
                .field("file_id", file_id)
                .field("file_name", file_name)
                .field("page_url", &page_url.as_ref().map(|_| "[só em memória]"))
                .finish(),
            Self::InvalidQuery(reason) => f.debug_tuple("InvalidQuery").field(reason).finish(),
            Self::Http(error) => f.debug_tuple("Http").field(error).finish(),
            Self::InvalidResponse(reason) => {
                f.debug_tuple("InvalidResponse").field(reason).finish()
            }
            Self::Internal(reason) => f.debug_tuple("Internal").field(reason).finish(),
        }
    }
}

/// Atalho para os resultados da crate.
pub type Result<T, E = Error> = std::result::Result<T, E>;

impl Error {
    /// Se a falha é de chave (ausente ou recusada): quem combina fontes trata a CurseForge como
    /// indisponível e mostra o aviso de Configurações (SPEC T08).
    #[must_use]
    pub fn is_key_problem(&self) -> bool {
        matches!(self, Self::KeyMissing | Self::KeyInvalid { .. })
    }

    /// Se a falha é de conexão (sem rede, tempo esgotado, limite ou servidor fora do ar).
    #[must_use]
    pub fn is_offline(&self) -> bool {
        matches!(
            self,
            Self::Http(
                warden_http::Error::Network { .. }
                    | warden_http::Error::Timeout { .. }
                    | warden_http::Error::RateLimited { .. }
                    | warden_http::Error::ServerError { .. }
            )
        )
    }
}

impl DomainError for Error {
    type Code = CurseforgeErrorCode;

    fn code(&self) -> DomainCode<CurseforgeErrorCode> {
        DomainCode::Domain(match self {
            Self::KeyMissing => CurseforgeErrorCode::KeyMissing,
            Self::KeyInvalid { .. } => CurseforgeErrorCode::KeyInvalid,
            Self::ModNotFound { .. } => CurseforgeErrorCode::ModNotFound,
            Self::FileNotFound { .. } => CurseforgeErrorCode::FileNotFound,
            Self::DistributionBlocked { .. } => CurseforgeErrorCode::DistributionBlocked,
            Self::InvalidQuery(_) => CurseforgeErrorCode::InvalidQuery,
            Self::InvalidResponse(_) => CurseforgeErrorCode::InvalidResponse,
            Self::Internal(_) => CurseforgeErrorCode::Internal,
            Self::Http(error) => match error.code() {
                DomainCode::Core(code) => return DomainCode::Core(code),
                DomainCode::Domain(code) => match code {
                    HttpErrorCode::RateLimited => CurseforgeErrorCode::RateLimited,
                    HttpErrorCode::ServerError => CurseforgeErrorCode::Unavailable,
                    HttpErrorCode::NotFound => CurseforgeErrorCode::NotFound,
                    HttpErrorCode::UnexpectedStatus => CurseforgeErrorCode::RequestRejected,
                    HttpErrorCode::InvalidResponse | HttpErrorCode::ResponseTooLarge => {
                        CurseforgeErrorCode::InvalidResponse
                    }
                    HttpErrorCode::Internal
                    | HttpErrorCode::HashMismatch
                    | HttpErrorCode::SizeMismatch
                    | HttpErrorCode::InvalidUrl
                    | HttpErrorCode::TooManyRedirects
                    | HttpErrorCode::InsecureRedirect => CurseforgeErrorCode::Internal,
                },
            },
        })
    }

    /// Parâmetros: `modId` e `fileId` nos "não encontrado" e no bloqueio, `file` (nome do
    /// arquivo ou, sem ele, o ID) no bloqueio, `status` na chave recusada; os da `warden-http`
    /// (`status`, `seconds`…) nos de rede.
    fn params(&self) -> BTreeMap<String, String> {
        let mut params = BTreeMap::new();
        match self {
            Self::KeyInvalid {
                status: Some(status),
            } => {
                params.insert("status".to_owned(), status.to_string());
            }
            Self::ModNotFound { id } => {
                params.insert("modId".to_owned(), id.to_string());
            }
            Self::FileNotFound { mod_id, file_id } => {
                if let Some(mod_id) = mod_id {
                    params.insert("modId".to_owned(), mod_id.to_string());
                }
                params.insert("fileId".to_owned(), file_id.to_string());
            }
            Self::DistributionBlocked {
                mod_id,
                file_id,
                file_name,
                ..
            } => {
                params.insert("modId".to_owned(), mod_id.to_string());
                params.insert("fileId".to_owned(), file_id.to_string());
                params.insert(
                    "file".to_owned(),
                    file_name.clone().unwrap_or_else(|| file_id.to_string()),
                );
            }
            Self::Http(error) => return error.params(),
            _ => {}
        }
        params
    }

    fn detail(&self) -> Option<String> {
        match self {
            Self::KeyMissing => None,
            Self::DistributionBlocked {
                page_url: Some(page),
                ..
            } => Some(format!(
                "{self}
página para baixar à mão: {page}"
            )),
            Self::Http(error) => error.detail(),
            _ => Some(error_chain(self)),
        }
    }

    fn retryable(&self) -> bool {
        match self {
            Self::Http(error) => error.retryable(),
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use warden_core::CoreErrorCode;

    use super::*;

    #[test]
    fn codigos_em_maiusculas_com_sublinhado() {
        let json = serde_json::to_value([
            CurseforgeErrorCode::Internal,
            CurseforgeErrorCode::KeyMissing,
            CurseforgeErrorCode::KeyInvalid,
            CurseforgeErrorCode::ModNotFound,
            CurseforgeErrorCode::FileNotFound,
            CurseforgeErrorCode::DistributionBlocked,
            CurseforgeErrorCode::InvalidQuery,
            CurseforgeErrorCode::NotFound,
            CurseforgeErrorCode::RateLimited,
            CurseforgeErrorCode::Unavailable,
            CurseforgeErrorCode::RequestRejected,
            CurseforgeErrorCode::InvalidResponse,
        ])
        .unwrap();
        assert_eq!(
            json,
            serde_json::json!([
                "INTERNAL",
                "CURSEFORGE_KEY_MISSING",
                "CURSEFORGE_KEY_INVALID",
                "MOD_NOT_FOUND",
                "FILE_NOT_FOUND",
                "DISTRIBUTION_BLOCKED",
                "INVALID_QUERY",
                "NOT_FOUND",
                "RATE_LIMITED",
                "UNAVAILABLE",
                "REQUEST_REJECTED",
                "INVALID_RESPONSE"
            ])
        );
    }

    fn http(error: warden_http::Error) -> Error {
        Error::Http(error)
    }

    #[test]
    fn erros_de_rede_viram_codigos_da_curseforge() {
        let host = || "api.curseforge.com".to_owned();
        let url = || "https://api.curseforge.com/v1/mods/search".to_owned();
        let cases = [
            (
                http(warden_http::Error::RateLimited {
                    host: host(),
                    url: url(),
                    wait: Some(Duration::from_secs(400)),
                }),
                DomainCode::Domain(CurseforgeErrorCode::RateLimited),
                true,
                true,
            ),
            (
                http(warden_http::Error::ServerError {
                    host: host(),
                    url: url(),
                    status: 503,
                }),
                DomainCode::Domain(CurseforgeErrorCode::Unavailable),
                true,
                true,
            ),
            (
                http(warden_http::Error::NotFound {
                    host: host(),
                    url: url(),
                    status: 404,
                }),
                DomainCode::Domain(CurseforgeErrorCode::NotFound),
                false,
                false,
            ),
            (
                http(warden_http::Error::UnexpectedStatus {
                    host: host(),
                    url: url(),
                    status: 400,
                    body: None,
                }),
                DomainCode::Domain(CurseforgeErrorCode::RequestRejected),
                false,
                false,
            ),
            (
                http(warden_http::Error::ResponseTooLarge {
                    host: host(),
                    url: url(),
                    limit: 1,
                }),
                DomainCode::Domain(CurseforgeErrorCode::InvalidResponse),
                false,
                false,
            ),
            (
                http(warden_http::Error::Timeout {
                    host: host(),
                    url: url(),
                    seconds: 30,
                }),
                DomainCode::Core(CoreErrorCode::Timeout),
                true,
                true,
            ),
            (
                http(warden_http::Error::Cancelled),
                DomainCode::Core(CoreErrorCode::Cancelled),
                false,
                false,
            ),
            (
                http(warden_http::Error::Internal("x".into())),
                DomainCode::Domain(CurseforgeErrorCode::Internal),
                false,
                false,
            ),
        ];
        for (error, code, retryable, offline) in cases {
            assert_eq!(error.code(), code, "{error}");
            assert_eq!(error.retryable(), retryable, "{error}");
            assert_eq!(error.is_offline(), offline, "{error}");
            assert!(!error.is_key_problem());
        }
    }

    #[test]
    fn chave_e_nao_encontrado_levam_os_parametros() {
        let error = Error::KeyMissing;
        assert_eq!(
            error.code(),
            DomainCode::Domain(CurseforgeErrorCode::KeyMissing)
        );
        assert!(error.is_key_problem());
        assert_eq!(error.detail(), None);
        assert!(!error.retryable());

        let error = Error::KeyInvalid { status: Some(403) };
        assert_eq!(
            error.code(),
            DomainCode::Domain(CurseforgeErrorCode::KeyInvalid)
        );
        assert!(error.is_key_problem());
        assert_eq!(error.params()["status"], "403");
        assert!(error.to_string().contains("403"));
        let error = Error::KeyInvalid { status: None };
        assert!(error.params().is_empty());
        assert!(error.to_string().contains("caracteres inválidos"));

        let error = Error::ModNotFound { id: 238_222 };
        assert_eq!(error.params()["modId"], "238222");

        let error = Error::FileNotFound {
            mod_id: Some(1),
            file_id: 2,
        };
        assert_eq!(error.params()["modId"], "1");
        assert_eq!(error.params()["fileId"], "2");
        let error = Error::FileNotFound {
            mod_id: None,
            file_id: 2,
        };
        assert!(!error.params().contains_key("modId"));

        let error = Error::DistributionBlocked {
            mod_id: 448_233,
            file_id: 7,
            file_name: Some("entityculling.jar".into()),
            page_url: Some("https://www.curseforge.com/minecraft/mc-mods/x/files/7".into()),
        };
        assert_eq!(
            error.code(),
            DomainCode::Domain(CurseforgeErrorCode::DistributionBlocked)
        );
        assert_eq!(error.params()["file"], "entityculling.jar");
        assert!(error.detail().unwrap().contains("/files/7"));
        assert!(!error.to_string().contains("/files/7"), "{error}");
        let debug = format!("{error:?}");
        assert!(!debug.contains("/files/7"), "{debug}");
        assert!(debug.contains("entityculling.jar"), "{debug}");
        for error in [
            Error::KeyMissing,
            Error::KeyInvalid { status: Some(401) },
            Error::ModNotFound { id: 1 },
            Error::FileNotFound {
                mod_id: None,
                file_id: 2,
            },
            Error::InvalidQuery("q".into()),
            Error::Http(warden_http::Error::Cancelled),
            Error::InvalidResponse("r".into()),
            Error::Internal("i".into()),
        ] {
            assert!(!format!("{error:?}").is_empty());
        }
        let error = Error::DistributionBlocked {
            mod_id: 1,
            file_id: 7,
            file_name: None,
            page_url: None,
        };
        assert_eq!(error.params()["file"], "7");

        let error = http(warden_http::Error::RateLimited {
            host: "api.curseforge.com".into(),
            url: "u".into(),
            wait: Some(Duration::from_secs(9)),
        });
        assert_eq!(error.params()["seconds"], "9");
        assert!(error.detail().unwrap().contains("429"));

        for (error, code) in [
            (
                Error::InvalidQuery("x".into()),
                CurseforgeErrorCode::InvalidQuery,
            ),
            (
                Error::InvalidResponse("faltou o campo".into()),
                CurseforgeErrorCode::InvalidResponse,
            ),
            (Error::Internal("x".into()), CurseforgeErrorCode::Internal),
        ] {
            assert_eq!(error.code(), DomainCode::Domain(code));
            assert!(error.params().is_empty());
        }
    }
}
