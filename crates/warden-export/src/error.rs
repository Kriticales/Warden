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
    /// O destino escolhido já existe e não é uma pasta vazia (ou é um link simbólico).
    DestinationNotEmpty,
    /// O destino escolhido fica dentro da pasta do pack.
    DestinationInsidePack,
    /// O `packwiz refresh` mudou o pack na cópia: o índice estava desatualizado.
    PackOutOfDate,
    /// O caminho tem colchetes, que o `.packwizignore` lê como padrão: excluir à mão.
    ExcludeNeedsManualRule,
}

/// Falha de preparação ou gravação da exportação. Os detalhes não contêm segredos.
#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct Error {
    code: ExportErrorCode,
    message: String,
}

impl Error {
    /// Falha com código específico.
    pub fn new(code: ExportErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    /// Falha sem código específico (`INTERNAL`).
    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(ExportErrorCode::Internal, message)
    }
}

/// Resultado das operações de exportação.
pub type Result<T> = std::result::Result<T, Error>;

impl DomainError for Error {
    type Code = ExportErrorCode;

    fn code(&self) -> DomainCode<Self::Code> {
        DomainCode::Domain(self.code)
    }
}
