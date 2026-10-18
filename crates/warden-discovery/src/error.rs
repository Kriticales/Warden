//! Códigos de erro do domínio `discovery` (ARCHITECTURE §5).
//!
//! Criado pela F0-05 só com `INTERNAL`, para o `AppError` já conhecer todos os domínios. A
//! tarefa dona desta crate acrescenta os códigos (só acréscimo, ROADMAP §1), as frases em
//! `apps/desktop/src/i18n/errors/discovery.ts` e o `enum Error`, que implementa
//! `warden_core::DomainError`.

use serde::{Deserialize, Serialize};

/// Códigos do domínio `discovery`. O código é contrato: renomear é mudança de contrato;
/// acrescentar é permitido (só acréscimo, ROADMAP §1).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DiscoveryErrorCode {
    /// Bug: invariante quebrada sem código específico.
    Internal,
    /// O autor do modpack da CurseForge bloqueou apps de terceiros (`downloadUrl` nulo); parâmetro
    /// `page` com a página do modpack, quando conhecida (P1-17).
    ModpackBlocked,
    /// O servidor não aceita leitura por partes e o arquivo do modpack passa de 200 MB; parâmetro
    /// `size` em MB (SPEC CA-T08-15; P1-17).
    ModpackTooLarge,
    /// O arquivo do modpack não pôde ser lido (zip inválido, sem `modrinth.index.json` ou
    /// `manifest.json`, ou sem versões) (P1-17).
    ModpackUnreadable,
}
