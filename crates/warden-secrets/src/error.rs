//! Erros do armazenamento de segredos. Nenhuma mensagem contém o valor de um segredo.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use warden_core::{CoreError, DomainCode, DomainError, error_chain};

use crate::kind::{BackendKind, SecretKind};

/// Códigos do domínio `secrets` (ARCHITECTURE §5). Só acréscimo.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SecretsErrorCode {
    /// Bug.
    Internal,
    /// O valor digitado não serve como chave (vazio, com quebra de linha, aspas simples ou
    /// longo demais).
    InvalidValue,
    /// A chave pedida não está configurada.
    NotConfigured,
    /// O cofre do sistema não está acessível (bloqueado, serviço parado, sem permissão).
    VaultUnavailable,
    /// O cofre do sistema recusou a operação.
    VaultFailed,
    /// O arquivo `.env` existe mas não pôde ser lido como texto.
    EnvFileUnreadable,
    /// Depois de gravar, a releitura não trouxe o mesmo valor.
    VerifyFailed,
    /// A troca de modo falhou; as chaves continuam no modo anterior.
    BackendSwitchFailed,
}

/// Erro de uma operação com segredos.
#[derive(Debug, thiserror::Error)]
pub enum SecretsError {
    /// Valor recusado por [`crate::normalize_secret`].
    #[error("valor inválido para {kind:?}: {reason}")]
    InvalidValue {
        /// Qual segredo.
        kind: SecretKind,
        /// Por quê (sem o valor).
        reason: &'static str,
    },
    /// Segredo ausente.
    #[error("{kind:?} não está configurado")]
    NotConfigured {
        /// Qual segredo.
        kind: SecretKind,
    },
    /// Cofre inacessível.
    #[error("cofre do sistema inacessível ({kind:?}): {message}")]
    VaultUnavailable {
        /// Qual segredo.
        kind: SecretKind,
        /// Mensagem do sistema (sem o valor).
        message: String,
    },
    /// Cofre recusou.
    #[error("o cofre do sistema falhou ({kind:?}): {message}")]
    VaultFailed {
        /// Qual segredo.
        kind: SecretKind,
        /// Mensagem do sistema (sem o valor).
        message: String,
    },
    /// `.env` ilegível.
    #[error("o arquivo .env não pôde ser lido como texto: {path}")]
    EnvFileUnreadable {
        /// Caminho do arquivo.
        path: PathBuf,
    },
    /// Releitura diferente do que foi gravado.
    #[error("a releitura de {kind:?} no armazenamento {backend:?} não confere")]
    VerifyFailed {
        /// Qual segredo.
        kind: SecretKind,
        /// Onde foi gravado.
        backend: BackendKind,
    },
    /// Troca de modo desfeita.
    #[error("a troca para {to:?} falhou; as chaves continuam em {from:?}")]
    BackendSwitchFailed {
        /// Modo anterior (continua valendo).
        from: BackendKind,
        /// Modo pedido.
        to: BackendKind,
        /// O que falhou.
        #[source]
        source: Box<SecretsError>,
    },
    /// Falha ao gravar o modo novo em `settings.json` durante a troca.
    #[error("falha ao gravar o modo das chaves nas configurações: {message}")]
    PersistFailed {
        /// Mensagem do erro (sem segredo: `settings.json` nunca contém valores).
        message: String,
    },
    /// Falha comum (disco, ponto de falha injetado…).
    #[error(transparent)]
    Core(#[from] CoreError),
}

impl DomainError for SecretsError {
    type Code = SecretsErrorCode;

    fn code(&self) -> DomainCode<SecretsErrorCode> {
        DomainCode::Domain(match self {
            Self::InvalidValue { .. } => SecretsErrorCode::InvalidValue,
            Self::NotConfigured { .. } => SecretsErrorCode::NotConfigured,
            Self::VaultUnavailable { .. } => SecretsErrorCode::VaultUnavailable,
            Self::VaultFailed { .. } => SecretsErrorCode::VaultFailed,
            Self::EnvFileUnreadable { .. } => SecretsErrorCode::EnvFileUnreadable,
            Self::VerifyFailed { .. } => SecretsErrorCode::VerifyFailed,
            Self::BackendSwitchFailed { .. } | Self::PersistFailed { .. } => {
                SecretsErrorCode::BackendSwitchFailed
            }
            Self::Core(error) => return core_code(error),
        })
    }

    fn params(&self) -> BTreeMap<String, String> {
        let mut params = BTreeMap::new();
        let kind = match self {
            Self::InvalidValue { kind, .. }
            | Self::NotConfigured { kind }
            | Self::VaultUnavailable { kind, .. }
            | Self::VaultFailed { kind, .. }
            | Self::VerifyFailed { kind, .. } => Some(*kind),
            _ => None,
        };
        if let Some(kind) = kind {
            params.insert("secret".to_owned(), kind_param(kind).to_owned());
        }
        match self {
            Self::InvalidValue { reason, .. } => {
                params.insert("reason".to_owned(), (*reason).to_owned());
            }
            Self::EnvFileUnreadable { path } => {
                params.insert("path".to_owned(), path.display().to_string());
            }
            Self::Core(error) => params.extend(error.params()),
            _ => {}
        }
        params
    }

    fn detail(&self) -> Option<String> {
        Some(error_chain(self))
    }

    fn retryable(&self) -> bool {
        match self {
            Self::VaultUnavailable { .. } | Self::BackendSwitchFailed { .. } => true,
            Self::Core(error) => error.retryable(),
            _ => false,
        }
    }
}

fn core_code(error: &CoreError) -> DomainCode<SecretsErrorCode> {
    match error.code() {
        DomainCode::Core(code) => DomainCode::Core(code),
        DomainCode::Domain(never) => match never {},
    }
}

/// Nome do segredo nos parâmetros da frase (a interface traduz).
const fn kind_param(kind: SecretKind) -> &'static str {
    match kind {
        SecretKind::Curseforge => "curseforge",
        SecretKind::Gemini => "gemini",
        SecretKind::Github => "github",
    }
}

impl From<std::io::Error> for SecretsError {
    /// Só para os pontos de falha injetada, que devolvem `io::Error`.
    fn from(error: std::io::Error) -> Self {
        Self::Core(CoreError::Internal(error.to_string()))
    }
}

/// Atalho para `CoreErrorCode` no código de testes.
#[cfg(test)]
pub(crate) const fn core(code: warden_core::CoreErrorCode) -> DomainCode<SecretsErrorCode> {
    DomainCode::Core(code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codigos_estaveis() {
        let json = serde_json::to_value([
            SecretsErrorCode::Internal,
            SecretsErrorCode::InvalidValue,
            SecretsErrorCode::NotConfigured,
            SecretsErrorCode::VaultUnavailable,
            SecretsErrorCode::VaultFailed,
            SecretsErrorCode::EnvFileUnreadable,
            SecretsErrorCode::VerifyFailed,
            SecretsErrorCode::BackendSwitchFailed,
        ])
        .unwrap();
        assert_eq!(
            json,
            serde_json::json!([
                "INTERNAL",
                "INVALID_VALUE",
                "NOT_CONFIGURED",
                "VAULT_UNAVAILABLE",
                "VAULT_FAILED",
                "ENV_FILE_UNREADABLE",
                "VERIFY_FAILED",
                "BACKEND_SWITCH_FAILED"
            ])
        );
    }

    #[test]
    fn parametros_e_novas_tentativas() {
        let error = SecretsError::InvalidValue {
            kind: SecretKind::Gemini,
            reason: "vazio",
        };
        assert_eq!(
            error.code(),
            DomainCode::Domain(SecretsErrorCode::InvalidValue)
        );
        assert_eq!(error.params()["secret"], "gemini");
        assert_eq!(error.params()["reason"], "vazio");
        assert!(!error.retryable());

        let switch = SecretsError::BackendSwitchFailed {
            from: BackendKind::Keyring,
            to: BackendKind::Envfile,
            source: Box::new(SecretsError::NotConfigured {
                kind: SecretKind::Github,
            }),
        };
        assert_eq!(
            switch.code(),
            DomainCode::Domain(SecretsErrorCode::BackendSwitchFailed)
        );
        assert!(switch.retryable());
        assert!(
            switch
                .detail()
                .unwrap()
                .contains("causa: Github não está configurado")
        );

        let core_error = SecretsError::from(std::io::Error::other("falha injetada"));
        assert_eq!(
            core_error.code(),
            core(warden_core::CoreErrorCode::Internal)
        );

        let unreadable = SecretsError::EnvFileUnreadable {
            path: PathBuf::from("C:/x/.env"),
        };
        assert_eq!(unreadable.params()["path"], "C:/x/.env");
    }
}
