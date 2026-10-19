//! Erros do domínio `ai` (ARCHITECTURE §5 e §9.5 "Erros e novas tentativas").
//!
//! Criado pela F0-05 só com `INTERNAL`; a D-04 acrescentou os códigos da IA (só acréscimo,
//! ROADMAP §1). As frases ficam em `apps/desktop/src/i18n/errors/ai.ts`.
//!
//! Nenhuma mensagem leva a chave: ela só vai no cabeçalho `x-goog-api-key`, e o corpo de um erro
//! da API passa por [`without_secret`] antes de entrar no detalhe.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use warden_core::{CoreErrorCode, DomainCode, DomainError};

/// Códigos do domínio `ai`. O código é contrato: renomear é mudança de contrato;
/// acrescentar é permitido (só acréscimo, ROADMAP §1).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AiErrorCode {
    /// Bug: invariante quebrada sem código específico.
    Internal,
    /// O Google recusou a chave do Gemini.
    KeyInvalid,
    /// O limite por minuto do plano gratuito não liberou depois das esperas.
    RateLimited,
    /// A cota diária (ou longa) do modelo acabou. Sem horário de volta (S-R5-4 §13).
    QuotaExceeded,
    /// O Google não respondeu dentro do tempo-limite, nem nas novas tentativas.
    Timeout,
    /// A resposta da IA veio num formato que não dá para usar, mesmo depois das novas tentativas.
    BadResponse,
    /// A IA passou do limite de consultas e não deu a resposta final.
    ToolLimit,
    /// O Google pediu cobrança ou pagamento (faturamento desligado, crédito, cartão).
    Billing,
    /// Ferramenta não permitida nesta conversa (vai para a IA, não para o usuário).
    PermissionDenied,
    /// O Google ficou sobrecarregado (503/500) em todas as novas tentativas.
    Unavailable,
    /// Nenhum modelo do Gemini disponível para esta chave serve para o diagnóstico.
    NoModel,
    /// O Google recusou o pedido por outro motivo (4xx).
    Rejected,
    /// A conversa não existe (apagada, ou o identificador está errado).
    ConversationNotFound,
    /// A IA ainda está respondendo nesta conversa.
    ConversationBusy,
    /// O texto inicial mostrado no consentimento não está mais guardado (o app reiniciou ou
    /// passou muito tempo): é preciso abrir o consentimento de novo.
    PreviewExpired,
    /// O arquivo de uma conversa está ilegível.
    ConversationUnreadable,
    /// A proposta não existe nesta conversa (D-14).
    ProposalNotFound,
    /// A proposta já foi aplicada ou descartada (D-14).
    ProposalDecided,
    /// O pack mudou depois que a diferença da proposta foi mostrada: é preciso revisar de novo
    /// (D-14).
    ProposalStale,
    /// A conversa começou com outro provedor de IA, e a chave dele não está configurada
    /// (ADR-0060: a conversa fica no provedor em que começou).
    ProviderKeyMissing,
}

/// Erro das funções desta crate.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Chave recusada (400 `API_KEY_INVALID`, 401 ou 403).
    #[error("o Google recusou a chave do Gemini (HTTP {status})")]
    KeyInvalid {
        /// Código HTTP.
        status: u16,
    },
    /// 429 por minuto que continuou depois das esperas.
    #[error("o limite por minuto do Gemini continuou depois de {waits} esperas")]
    RateLimited {
        /// Quantas vezes o Warden esperou.
        waits: u32,
    },
    /// 429 diário (ou com espera longa).
    #[error("a cota do modelo {model} acabou (plano gratuito: {free_tier})")]
    QuotaExceeded {
        /// O modelo.
        model: String,
        /// O `quotaId` diz `FreeTier`.
        free_tier: bool,
    },
    /// Tempo esgotado em todas as tentativas.
    #[error("o Google não respondeu em {seconds} s, nem nas novas tentativas")]
    Timeout {
        /// O tempo-limite por requisição.
        seconds: u64,
    },
    /// 503/500 em todas as tentativas.
    #[error("o Google respondeu {status} (sobrecarregado) em todas as tentativas")]
    Unavailable {
        /// O último código.
        status: u16,
    },
    /// Resposta que não dá para usar.
    #[error("resposta inválida da IA: {reason}")]
    BadResponse {
        /// O que estava errado.
        reason: String,
    },
    /// Passou do limite de consultas sem resposta final.
    #[error("a IA passou de {rounds} rodadas de consulta sem responder")]
    ToolLimit {
        /// O limite.
        rounds: u32,
    },
    /// Sinal de cobrança.
    #[error("o Google pediu cobrança: {detail}")]
    Billing {
        /// O corpo do erro, sem a chave.
        detail: String,
    },
    /// Outro 4xx.
    #[error("o Google recusou o pedido (HTTP {status}): {detail}")]
    Rejected {
        /// Código HTTP.
        status: u16,
        /// O corpo do erro, sem a chave.
        detail: String,
    },
    /// Nenhum modelo serve.
    #[error("nenhum modelo do Gemini disponível para esta chave serve para o diagnóstico")]
    NoModel,
    /// Conversa inexistente.
    #[error("a conversa {id} não existe")]
    ConversationNotFound {
        /// O identificador.
        id: String,
    },
    /// A IA ainda está respondendo.
    #[error("a IA ainda está respondendo na conversa {id}")]
    ConversationBusy {
        /// O identificador.
        id: String,
    },
    /// O texto do consentimento expirou.
    #[error("o texto inicial do consentimento não está mais guardado")]
    PreviewExpired,
    /// Conversa ilegível.
    #[error("a conversa {} está ilegível: {reason}", .path.display())]
    ConversationUnreadable {
        /// O arquivo (nos dados do Warden).
        path: PathBuf,
        /// O erro de leitura.
        reason: String,
    },
    /// Proposta inexistente (D-14).
    #[error("a proposta {id} não existe nesta conversa")]
    ProposalNotFound {
        /// O id (`p1`).
        id: String,
    },
    /// Proposta já decidida (D-14).
    #[error("a proposta {id} já foi aplicada ou descartada")]
    ProposalDecided {
        /// O id.
        id: String,
    },
    /// O pack mudou desde a diferença mostrada (D-14).
    #[error("o pack mudou desde que a diferença da proposta {id} foi mostrada")]
    ProposalStale {
        /// O id.
        id: String,
    },
    /// A conversa é de outro provedor, sem chave configurada.
    #[error("a conversa começou com {}, e a chave dele não está configurada", .provider.display_name())]
    ProviderKeyMissing {
        /// O provedor da conversa.
        provider: crate::providers::AiProvider,
    },
    /// Falha de rede, tempo esgotado ou cancelamento vindos do cliente HTTP.
    #[error(transparent)]
    Http(#[from] warden_http::Error),
    /// Falha de disco nas conversas.
    #[error(transparent)]
    Core(#[from] warden_core::CoreError),
    /// Falha de disco ao ler ou apagar uma conversa.
    #[error("falha ao {action} {}", .path.display())]
    Io {
        /// O que se tentava fazer.
        action: &'static str,
        /// O caminho.
        path: PathBuf,
        /// O erro do sistema.
        #[source]
        source: std::io::Error,
    },
    /// Cancelado (conversa apagada enquanto a IA respondia).
    #[error("operação cancelada")]
    Cancelled,
    /// Invariante quebrada.
    #[error("{0}")]
    Internal(String),
}

/// Resultado das funções desta crate.
pub type Result<T> = std::result::Result<T, Error>;

impl DomainError for Error {
    type Code = AiErrorCode;

    fn code(&self) -> DomainCode<AiErrorCode> {
        DomainCode::Domain(match self {
            Self::KeyInvalid { .. } => AiErrorCode::KeyInvalid,
            Self::RateLimited { .. } => AiErrorCode::RateLimited,
            Self::QuotaExceeded { .. } => AiErrorCode::QuotaExceeded,
            Self::Timeout { .. } => AiErrorCode::Timeout,
            Self::Unavailable { .. } => AiErrorCode::Unavailable,
            Self::BadResponse { .. } => AiErrorCode::BadResponse,
            Self::ToolLimit { .. } => AiErrorCode::ToolLimit,
            Self::Billing { .. } => AiErrorCode::Billing,
            Self::Rejected { .. } => AiErrorCode::Rejected,
            Self::NoModel => AiErrorCode::NoModel,
            Self::ConversationNotFound { .. } => AiErrorCode::ConversationNotFound,
            Self::ConversationBusy { .. } => AiErrorCode::ConversationBusy,
            Self::PreviewExpired => AiErrorCode::PreviewExpired,
            Self::ConversationUnreadable { .. } => AiErrorCode::ConversationUnreadable,
            Self::ProposalNotFound { .. } => AiErrorCode::ProposalNotFound,
            Self::ProposalDecided { .. } => AiErrorCode::ProposalDecided,
            Self::ProposalStale { .. } => AiErrorCode::ProposalStale,
            Self::ProviderKeyMissing { .. } => AiErrorCode::ProviderKeyMissing,
            Self::Http(error) => return http_code(error),
            Self::Core(error) => return DomainCode::Core(core_code(error)),
            Self::Io { .. } => return DomainCode::Core(CoreErrorCode::Io),
            Self::Cancelled => return DomainCode::Core(CoreErrorCode::Cancelled),
            Self::Internal(_) => AiErrorCode::Internal,
        })
    }

    /// `model` (cota), `status` (HTTP), `seconds` (tempo-limite), `waits` (esperas) e
    /// `provider`/`providerName` (chave do provedor da conversa).
    fn params(&self) -> BTreeMap<String, String> {
        let mut params = BTreeMap::new();
        match self {
            Self::QuotaExceeded { model, free_tier } => {
                params.insert("model".to_owned(), model.clone());
                params.insert("freeTier".to_owned(), free_tier.to_string());
            }
            Self::KeyInvalid { status }
            | Self::Unavailable { status }
            | Self::Rejected { status, .. } => {
                params.insert("status".to_owned(), status.to_string());
            }
            Self::Timeout { seconds } => {
                params.insert("seconds".to_owned(), seconds.to_string());
            }
            Self::RateLimited { waits } => {
                params.insert("waits".to_owned(), waits.to_string());
            }
            Self::ToolLimit { rounds } => {
                params.insert("rounds".to_owned(), rounds.to_string());
            }
            Self::Io { path, .. } | Self::ConversationUnreadable { path, .. } => {
                // Já sem a pasta do usuário: `ai/<pack>/<arquivo>`.
                params.insert("path".to_owned(), path.display().to_string());
            }
            Self::ProviderKeyMissing { provider } => {
                params.insert("provider".to_owned(), provider.id().to_owned());
                params.insert(
                    "providerName".to_owned(),
                    provider.display_name().to_owned(),
                );
            }
            Self::Http(error) => return error.params(),
            _ => {}
        }
        params
    }

    fn retryable(&self) -> bool {
        match self {
            Self::RateLimited { .. }
            | Self::Timeout { .. }
            | Self::Unavailable { .. }
            | Self::BadResponse { .. }
            | Self::ToolLimit { .. }
            | Self::ConversationBusy { .. } => true,
            Self::Http(error) => error.retryable(),
            _ => false,
        }
    }
}

fn http_code(error: &warden_http::Error) -> DomainCode<AiErrorCode> {
    match error.code() {
        DomainCode::Core(code) => DomainCode::Core(code),
        DomainCode::Domain(_) => DomainCode::Domain(AiErrorCode::BadResponse),
    }
}

fn core_code(error: &warden_core::CoreError) -> CoreErrorCode {
    match error.code() {
        DomainCode::Core(code) => code,
        DomainCode::Domain(never) => match never {},
    }
}

/// Troca o valor da chave por `<segredo>` (o corpo de um erro da API pode ecoar o pedido).
#[must_use]
pub fn without_secret(text: &str, secret: &str) -> String {
    if secret.trim().is_empty() {
        text.to_owned()
    } else {
        text.replace(secret, "<segredo>")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codigos_e_parametros() {
        let quota = Error::QuotaExceeded {
            model: "modelo-x".into(),
            free_tier: true,
        };
        assert_eq!(quota.code(), DomainCode::Domain(AiErrorCode::QuotaExceeded));
        assert_eq!(quota.params()["model"], "modelo-x");
        assert!(!quota.retryable());
        assert!(
            Error::Unavailable { status: 503 }.retryable(),
            "sobrecarga passa: dá para tentar de novo"
        );
        assert_eq!(
            Error::Cancelled.code(),
            DomainCode::Core(CoreErrorCode::Cancelled)
        );
        let json = serde_json::to_value(AiErrorCode::QuotaExceeded).unwrap();
        assert_eq!(json, "QUOTA_EXCEEDED");
        let missing = Error::ProviderKeyMissing {
            provider: crate::providers::AiProvider::Openai,
        };
        assert_eq!(
            missing.code(),
            DomainCode::Domain(AiErrorCode::ProviderKeyMissing)
        );
        assert_eq!(missing.params()["provider"], "openai");
        assert_eq!(missing.params()["providerName"], "OpenAI");
        assert!(!missing.retryable());
        assert_eq!(
            serde_json::to_value(AiErrorCode::ProviderKeyMissing).unwrap(),
            "PROVIDER_KEY_MISSING"
        );
    }

    #[test]
    fn chave_nunca_vai_para_o_detalhe() {
        assert_eq!(
            without_secret("eco: AIza-abc e mais", "AIza-abc"),
            "eco: <segredo> e mais"
        );
        assert_eq!(without_secret("nada", ""), "nada");
    }
}
