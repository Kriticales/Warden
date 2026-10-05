//! Erros do domínio `java` (ARCHITECTURE §5).
//!
//! As falhas de rede chegam da `warden-http` e viram códigos do Java que dizem de qual fonte
//! (Adoptium ou Mojang) era o pedido; sem conexão, tempo esgotado, cancelamento e falha de
//! disco continuam com os códigos comuns do domínio `core`. As frases ficam em
//! `apps/desktop/src/i18n/errors/java.ts`.

use std::collections::BTreeMap;
use std::io;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use warden_core::{CoreError, CoreErrorCode, DomainCode, DomainError, error_chain};
use warden_http::HttpErrorCode;

use crate::version::JavaVersion;

/// Códigos do domínio `java`. O código é contrato: renomear é mudança de contrato;
/// acrescentar é permitido (só acréscimo, ROADMAP §1).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum JavaErrorCode {
    /// Bug: invariante quebrada sem código específico.
    Internal,
    /// Este sistema ou processador não tem Java de 64 bits que o Warden saiba baixar.
    UnsupportedPlatform,
    /// A fonte não publica o Java pedido para este sistema.
    NoBuildAvailable,
    /// A fonte (Adoptium ou Mojang) está fora do ar, limitou os pedidos ou recusou o pedido.
    SourceUnavailable,
    /// A fonte respondeu algo que o Warden não entende.
    InvalidResponse,
    /// O arquivo baixado não confere com o hash ou o tamanho informados pela fonte.
    DownloadCorrupted,
    /// O pacote do Java está corrompido, tem caminhos perigosos ou não tem o programa `java`.
    ArchiveInvalid,
    /// O Java não abriu ou não respondeu como um Java.
    ValidationFailed,
    /// O Java é de 32 bits.
    #[serde(rename = "NOT_64_BIT")]
    Not64Bit,
    /// O Java instalado não é a versão pedida.
    UnexpectedVersion,
    /// O Java pedido não está instalado.
    RuntimeNotFound,
    /// O Java está sendo usado por um jogo aberto.
    RuntimeInUse,
    /// A pasta do Java não pôde ser movida para o lugar (outro programa a segurou).
    InstallBlocked,
    /// Não há regra para esta versão do Minecraft e o JSON da versão não foi lido.
    VersionRequirementUnknown,
    /// A tabela de compatibilidade embutida é inválida (bug).
    CompatibilityTableInvalid,
}

/// Fonte de um pedido de rede, para a frase dizer qual servidor falhou.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum JavaSource {
    /// Eclipse Adoptium (Temurin), a fonte padrão.
    Adoptium,
    /// Runtime oficial da Mojang, a alternativa.
    Mojang,
}

impl JavaSource {
    /// Nome para as frases ("Adoptium", "Mojang").
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Adoptium => "Adoptium",
            Self::Mojang => "Mojang",
        }
    }
}

/// Erro das funções desta crate.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Sistema ou processador sem Java de 64 bits suportado.
    #[error("sistema sem Java suportado: {os} {arch}")]
    UnsupportedPlatform {
        /// Sistema operacional.
        os: String,
        /// Processador.
        arch: String,
    },
    /// A fonte não tem o Java pedido.
    #[error("{} não publica Java {major}{} para {platform}", .source_name.label(), cap_suffix(*.max_update))]
    NoBuild {
        /// Fonte consultada.
        source_name: JavaSource,
        /// Major pedido.
        major: u32,
        /// Teto da atualização, quando houver.
        max_update: Option<u32>,
        /// Sistema e processador (`windows-x64`).
        platform: String,
    },
    /// Falha de rede ou de download num pedido a uma fonte.
    #[error("{}: {error}", .source_name.label())]
    Http {
        /// Fonte do pedido.
        source_name: JavaSource,
        /// Erro do cliente HTTP.
        #[source]
        error: warden_http::Error,
    },
    /// Resposta que não pôde ser usada.
    #[error("resposta inválida de {}: {message}", .source_name.label())]
    InvalidResponse {
        /// Fonte do pedido.
        source_name: JavaSource,
        /// O que estava errado.
        message: String,
    },
    /// Pacote do Java inválido ou perigoso.
    #[error("pacote do Java inválido ({}): {message}", .path.display())]
    Archive {
        /// O pacote.
        path: PathBuf,
        /// O que estava errado.
        message: String,
    },
    /// O Java não respondeu como esperado.
    #[error("o Java em {} não pôde ser validado: {message}", .path.display())]
    Validation {
        /// O programa `java`.
        path: PathBuf,
        /// O que deu errado.
        message: String,
        /// Começo da saída do programa (até 4 KB), para "Detalhes técnicos".
        output: Option<String>,
    },
    /// Java de 32 bits.
    #[error("o Java em {} é de 32 bits ({arch})", .path.display())]
    Not64Bit {
        /// O programa `java`.
        path: PathBuf,
        /// `os.arch` informado.
        arch: String,
    },
    /// O Java não é a versão pedida.
    #[error("esperado Java {expected_major}{}, encontrado {found}", cap_suffix(*.max_update))]
    UnexpectedVersion {
        /// Major esperado.
        expected_major: u32,
        /// Teto da atualização, quando houver.
        max_update: Option<u32>,
        /// Versão encontrada.
        found: JavaVersion,
    },
    /// Runtime inexistente.
    #[error("Java {id:?} não está instalado")]
    RuntimeNotFound {
        /// Identificador pedido.
        id: String,
    },
    /// Runtime usado por um jogo aberto.
    #[error("o Java {id:?} está sendo usado por um jogo aberto")]
    RuntimeInUse {
        /// Identificador.
        id: String,
    },
    /// Não foi possível mover a pasta do Java para o lugar.
    #[error("não foi possível mover {} para o lugar", .path.display())]
    InstallBlocked {
        /// Pasta de destino.
        path: PathBuf,
        /// Erro do sistema depois das novas tentativas.
        #[source]
        source: io::Error,
    },
    /// Sem regra e sem JSON da versão.
    #[error("sem regra de Java para o Minecraft {minecraft:?} e sem o JSON da versão")]
    VersionRequirementUnknown {
        /// Versão do Minecraft.
        minecraft: String,
    },
    /// Tabela de compatibilidade inválida.
    #[error("tabela de compatibilidade do Java inválida: {0}")]
    CompatibilityTable(String),
    /// Falha de disco, caminho fora da pasta ou outro erro comum.
    #[error(transparent)]
    Core(#[from] CoreError),
    /// Cancelado pelo usuário.
    #[error("operação cancelada")]
    Cancelled,
    /// Invariante quebrada.
    #[error("erro interno: {0}")]
    Internal(String),
}

/// Atalho para os resultados da crate.
pub type Result<T, E = Error> = std::result::Result<T, E>;

fn cap_suffix(max_update: Option<u32>) -> String {
    max_update.map_or_else(String::new, |cap| format!(" (até a atualização {cap})"))
}

impl Error {
    /// Atalho para um erro de disco ([`CoreError::Io`]).
    pub(crate) fn io(action: &'static str, path: impl Into<PathBuf>, source: io::Error) -> Self {
        Self::Core(CoreError::io(action, path, source))
    }

    /// Atalho para um erro de rede de uma fonte.
    pub(crate) fn http(source_name: JavaSource) -> impl FnOnce(warden_http::Error) -> Self {
        move |error| match error {
            warden_http::Error::Cancelled => Self::Cancelled,
            error => Self::Http { source_name, error },
        }
    }

    /// Se a falha é da fonte (fora do ar, sem conexão, sem o build, resposta estranha): nesses
    /// casos o Warden tenta a outra fonte.
    #[must_use]
    pub fn is_source_failure(&self) -> bool {
        match self {
            Self::NoBuild { .. } | Self::InvalidResponse { .. } => true,
            Self::Http { error, .. } => !matches!(
                error,
                warden_http::Error::Cancelled | warden_http::Error::Io { .. }
            ),
            _ => false,
        }
    }
}

impl DomainError for Error {
    type Code = JavaErrorCode;

    fn code(&self) -> DomainCode<JavaErrorCode> {
        DomainCode::Domain(match self {
            Self::UnsupportedPlatform { .. } => JavaErrorCode::UnsupportedPlatform,
            Self::NoBuild { .. } => JavaErrorCode::NoBuildAvailable,
            Self::InvalidResponse { .. } => JavaErrorCode::InvalidResponse,
            Self::Archive { .. } => JavaErrorCode::ArchiveInvalid,
            Self::Validation { .. } => JavaErrorCode::ValidationFailed,
            Self::Not64Bit { .. } => JavaErrorCode::Not64Bit,
            Self::UnexpectedVersion { .. } => JavaErrorCode::UnexpectedVersion,
            Self::RuntimeNotFound { .. } => JavaErrorCode::RuntimeNotFound,
            Self::RuntimeInUse { .. } => JavaErrorCode::RuntimeInUse,
            Self::InstallBlocked { .. } => JavaErrorCode::InstallBlocked,
            Self::VersionRequirementUnknown { .. } => JavaErrorCode::VersionRequirementUnknown,
            Self::CompatibilityTable(_) => JavaErrorCode::CompatibilityTableInvalid,
            Self::Internal(_) => JavaErrorCode::Internal,
            Self::Cancelled => return DomainCode::Core(CoreErrorCode::Cancelled),
            Self::Core(error) => {
                return match error.code() {
                    DomainCode::Core(code) => DomainCode::Core(code),
                    DomainCode::Domain(never) => match never {},
                };
            }
            Self::Http { error, .. } => match error.code() {
                DomainCode::Core(code) => return DomainCode::Core(code),
                DomainCode::Domain(code) => match code {
                    HttpErrorCode::RateLimited
                    | HttpErrorCode::ServerError
                    | HttpErrorCode::NotFound
                    | HttpErrorCode::UnexpectedStatus => JavaErrorCode::SourceUnavailable,
                    HttpErrorCode::HashMismatch | HttpErrorCode::SizeMismatch => {
                        JavaErrorCode::DownloadCorrupted
                    }
                    HttpErrorCode::InvalidResponse
                    | HttpErrorCode::ResponseTooLarge
                    | HttpErrorCode::InvalidUrl
                    | HttpErrorCode::TooManyRedirects
                    | HttpErrorCode::InsecureRedirect => JavaErrorCode::InvalidResponse,
                    HttpErrorCode::Internal => JavaErrorCode::Internal,
                },
            },
        })
    }

    /// Parâmetros: `source` (Adoptium/Mojang) nos de rede; `major`, `maxUpdate` e `platform`
    /// no "sem build"; `path` nos de pacote e validação; `id` nos de runtime; `expected` e
    /// `found` na versão inesperada; `minecraft` na falta de regra.
    fn params(&self) -> BTreeMap<String, String> {
        let mut params = BTreeMap::new();
        let mut put = |name: &str, value: String| {
            params.insert(name.to_owned(), value);
        };
        match self {
            Self::UnsupportedPlatform { os, arch } => {
                put("platform", format!("{os} {arch}"));
            }
            Self::NoBuild {
                source_name,
                major,
                max_update,
                platform,
            } => {
                put("source", source_name.label().to_owned());
                put("major", major.to_string());
                put("platform", platform.clone());
                if let Some(cap) = max_update {
                    put("maxUpdate", cap.to_string());
                }
            }
            Self::Http { source_name, error } => {
                for (name, value) in error.params() {
                    put(&name, value);
                }
                put("source", source_name.label().to_owned());
            }
            Self::InvalidResponse { source_name, .. } => {
                put("source", source_name.label().to_owned());
            }
            Self::Archive { path, .. }
            | Self::Validation { path, .. }
            | Self::Not64Bit { path, .. }
            | Self::InstallBlocked { path, .. } => put("path", path.display().to_string()),
            Self::UnexpectedVersion {
                expected_major,
                found,
                ..
            } => {
                put("expected", expected_major.to_string());
                put("found", found.to_string());
            }
            Self::RuntimeNotFound { id } | Self::RuntimeInUse { id } => put("id", id.clone()),
            Self::VersionRequirementUnknown { minecraft } => put("minecraft", minecraft.clone()),
            Self::Core(error) => {
                for (name, value) in error.params() {
                    put(&name, value);
                }
            }
            Self::CompatibilityTable(_) | Self::Cancelled | Self::Internal(_) => {}
        }
        params
    }

    fn detail(&self) -> Option<String> {
        match self {
            Self::Cancelled => None,
            Self::Validation {
                output: Some(output),
                ..
            } => Some(format!("{}\nsaída do Java:\n{output}", error_chain(self))),
            _ => Some(error_chain(self)),
        }
    }

    fn retryable(&self) -> bool {
        match self {
            Self::Http { error, .. } => error.retryable(),
            Self::Core(error) => error.retryable(),
            Self::InstallBlocked { .. } | Self::Validation { .. } => true,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codigos_em_maiusculas_com_sublinhado() {
        let json = serde_json::to_value([
            JavaErrorCode::Internal,
            JavaErrorCode::UnsupportedPlatform,
            JavaErrorCode::NoBuildAvailable,
            JavaErrorCode::SourceUnavailable,
            JavaErrorCode::InvalidResponse,
            JavaErrorCode::DownloadCorrupted,
            JavaErrorCode::ArchiveInvalid,
            JavaErrorCode::ValidationFailed,
            JavaErrorCode::Not64Bit,
            JavaErrorCode::UnexpectedVersion,
            JavaErrorCode::RuntimeNotFound,
            JavaErrorCode::RuntimeInUse,
            JavaErrorCode::InstallBlocked,
            JavaErrorCode::VersionRequirementUnknown,
            JavaErrorCode::CompatibilityTableInvalid,
        ])
        .unwrap();
        assert_eq!(
            json,
            serde_json::json!([
                "INTERNAL",
                "UNSUPPORTED_PLATFORM",
                "NO_BUILD_AVAILABLE",
                "SOURCE_UNAVAILABLE",
                "INVALID_RESPONSE",
                "DOWNLOAD_CORRUPTED",
                "ARCHIVE_INVALID",
                "VALIDATION_FAILED",
                "NOT_64_BIT",
                "UNEXPECTED_VERSION",
                "RUNTIME_NOT_FOUND",
                "RUNTIME_IN_USE",
                "INSTALL_BLOCKED",
                "VERSION_REQUIREMENT_UNKNOWN",
                "COMPATIBILITY_TABLE_INVALID",
            ])
        );
    }

    #[test]
    fn rede_vira_codigo_da_fonte() {
        let error = Error::Http {
            source_name: JavaSource::Adoptium,
            error: warden_http::Error::ServerError {
                host: "api.adoptium.net".into(),
                url: "https://api.adoptium.net/v3/".into(),
                status: 503,
            },
        };
        assert_eq!(
            error.code(),
            DomainCode::Domain(JavaErrorCode::SourceUnavailable)
        );
        assert_eq!(error.params()["source"], "Adoptium");
        assert!(error.is_source_failure());

        let timeout = Error::Http {
            source_name: JavaSource::Mojang,
            error: warden_http::Error::Timeout {
                host: "piston-meta.mojang.com".into(),
                url: "https://piston-meta.mojang.com/".into(),
                seconds: 30,
            },
        };
        assert_eq!(timeout.code(), DomainCode::Core(CoreErrorCode::Timeout));
        assert!(timeout.is_source_failure());
        assert!(timeout.retryable());

        let cancelled = Error::http(JavaSource::Adoptium)(warden_http::Error::Cancelled);
        assert!(matches!(cancelled, Error::Cancelled));
        assert_eq!(cancelled.code(), DomainCode::Core(CoreErrorCode::Cancelled));
        assert_eq!(cancelled.detail(), None);
        assert!(!cancelled.is_source_failure());
    }

    #[test]
    fn parametros_e_detalhes() {
        let error = Error::NoBuild {
            source_name: JavaSource::Adoptium,
            major: 8,
            max_update: Some(312),
            platform: "windows-x64".into(),
        };
        assert_eq!(
            error.code(),
            DomainCode::Domain(JavaErrorCode::NoBuildAvailable)
        );
        assert_eq!(error.params()["maxUpdate"], "312");
        assert!(error.to_string().contains("até a atualização 312"));

        let validation = Error::Validation {
            path: "C:/runtimes/x/bin/java.exe".into(),
            message: "saída sem java.version".into(),
            output: Some("Error: Could not create the Java Virtual Machine.".into()),
        };
        assert!(validation.detail().unwrap().contains("Could not create"));
        assert_eq!(validation.params()["path"], "C:/runtimes/x/bin/java.exe");

        let unexpected = Error::UnexpectedVersion {
            expected_major: 17,
            max_update: None,
            found: JavaVersion::new(21, 0, 7, 0, 6),
        };
        assert_eq!(unexpected.params()["found"], "21.0.7");

        let io = Error::io("ler", "C:/x", io::Error::other("falhou"));
        assert_eq!(io.code(), DomainCode::Core(CoreErrorCode::Io));
        assert_eq!(io.params()["path"], "C:/x");
    }
}
