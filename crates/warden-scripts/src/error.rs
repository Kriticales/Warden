//! Códigos de erro do domínio `scripts` (ARCHITECTURE §5).
//!
//! Criado pela F0-05 só com `INTERNAL`, para o `AppError` já conhecer todos os domínios. A
//! tarefa dona desta crate acrescenta os códigos (só acréscimo, ROADMAP §1), as frases em
//! `apps/desktop/src/i18n/errors/scripts.ts` e o `enum Error`, que implementa
//! `warden_core::DomainError`.

use serde::{Deserialize, Serialize};

/// Códigos do domínio `scripts`. O código é contrato: renomear é mudança de contrato;
/// acrescentar é permitido (só acréscimo, ROADMAP §1).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ScriptsErrorCode {
    /// Bug: invariante quebrada sem código específico.
    Internal,
}
