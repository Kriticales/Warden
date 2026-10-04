//! Erro único que atravessa o IPC (ARCHITECTURE §5; ADR-0018).
//!
//! Versão mínima da F0-01: só o domínio `app`, com o código `INTERNAL`. A F0-05 acrescenta os
//! demais domínios, `operationId` e a conversão a partir dos erros das crates.

use std::collections::BTreeMap;

use serde::Serialize;

/// Erro devolvido por todo comando (`Result<T, AppError>`). A interface mostra a frase do
/// catálogo para `code`, com `params`, e `detail` em "Detalhes técnicos".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type, thiserror::Error)]
#[serde(rename_all = "camelCase")]
#[error("{code}")]
pub struct AppError {
    /// Código estável, por domínio.
    pub code: ErrorCode,
    /// Valores para a frase traduzida (nome do mod, versão…).
    pub params: BTreeMap<String, String>,
    /// Evidência técnica (saída de ferramenta, status HTTP, caminho), sem segredos.
    pub detail: Option<String>,
    /// Se repetir a mesma ação pode dar certo.
    pub retryable: bool,
}

impl AppError {
    /// Erro de bug: invariante quebrada, sem código específico ainda.
    #[must_use]
    pub fn internal(detail: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::App(AppErrorCode::Internal),
            params: BTreeMap::new(),
            detail: Some(detail.into()),
            retryable: false,
        }
    }
}

/// Código de erro com o domínio: `{ "domain": "app", "code": "INTERNAL" }` no JSON.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(tag = "domain", content = "code", rename_all = "camelCase")]
pub enum ErrorCode {
    /// Erros do próprio app (`warden-app`).
    App(AppErrorCode),
}

impl std::fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::App(code) => write!(f, "app.{}", code.as_str()),
        }
    }
}

/// Códigos do domínio `app`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AppErrorCode {
    /// Bug: todo `INTERNAL` visto em teste ou uso vira tarefa para um código específico.
    Internal,
}

impl AppErrorCode {
    /// O código como aparece no JSON.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Internal => "INTERNAL",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializa_com_dominio_e_codigo() {
        let error = AppError::internal("falhou");
        let json = serde_json::to_value(&error).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "code": { "domain": "app", "code": "INTERNAL" },
                "params": {},
                "detail": "falhou",
                "retryable": false,
            })
        );
    }

    #[test]
    fn as_str_bate_com_o_json() {
        let json = serde_json::to_value(AppErrorCode::Internal).unwrap();
        assert_eq!(json, AppErrorCode::Internal.as_str());
        assert_eq!(AppError::internal("x").to_string(), "app.INTERNAL");
    }
}
