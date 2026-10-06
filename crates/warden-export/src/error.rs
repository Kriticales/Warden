//! Códigos de erro do domínio `export` (ARCHITECTURE §5).
//!
//! Criado pela F0-05 só com `INTERNAL`, para o `AppError` já conhecer todos os domínios. A
//! tarefa dona desta crate acrescenta os códigos (só acréscimo, ROADMAP §1), as frases em
//! `apps/desktop/src/i18n/errors/export.ts` e o `enum Error`, que implementa
//! `warden_core::DomainError`.

use serde::{Deserialize, Serialize};
use warden_core::{DomainCode, DomainError};

/// Códigos do domínio `export`. O código é contrato: renomear é mudança de contrato;
/// acrescentar é permitido (só acréscimo, ROADMAP §1).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ExportErrorCode {
    /// Bug: invariante quebrada sem código específico.
    Internal,
}

/// Falha de preparação ou gravação da exportação. Os detalhes não contêm segredos.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct Error(pub String);

/// Resultado das operações de exportação.
pub type Result<T> = std::result::Result<T, Error>;

impl DomainError for Error {
    type Code = ExportErrorCode;

    fn code(&self) -> DomainCode<Self::Code> {
        DomainCode::Domain(ExportErrorCode::Internal)
    }
}
