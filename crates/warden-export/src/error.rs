//! Códigos de erro do domínio `export` (ARCHITECTURE §5).
//!
//! Criado pela F0-05 só com `INTERNAL`, para o `AppError` já conhecer todos os domínios. A
//! tarefa dona desta crate acrescenta os códigos (só acréscimo, ROADMAP §1), as frases em
//! `apps/desktop/src/i18n/errors/export.ts` e o `enum Error`, que implementa
//! `warden_core::DomainError`.

use std::collections::BTreeMap;

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
    /// A instância pronta para o Prism exige `version` no `pack.toml`.
    PrismVersionRequired,
    /// Mod da CurseForge que o autor não deixa distribuir a apps de terceiros: o Prism não teria
    /// como baixar. Pode haver troca pelo Modrinth.
    PrismBlocked,
    /// Há arquivos de terceiros que iriam dentro do arquivo e a pessoa não confirmou a licença.
    PrismLocalNeedsConfirmation,
    /// Há mods da CurseForge e falta a chave para conferir se podem ser distribuídos.
    PrismCurseforgeKeyMissing,
    /// Modrinth ou CurseForge não responderam à conferência dos arquivos.
    PrismLookupFailed,
    /// Um arquivo do pack não pôde ser baixado ou conferido para entrar na instância.
    PrismDownloadFailed,
    /// O arquivo gerado não passou na validação e foi descartado.
    PrismInvalidOutput,
    /// O pack usa uma organização que a instância pronta não sabe reproduzir.
    PrismUnsupportedPack,
}

/// Falha de preparação ou gravação da exportação. Os detalhes não contêm segredos.
#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct Error {
    code: ExportErrorCode,
    message: String,
    params: BTreeMap<String, String>,
    detail: Option<String>,
}

impl Error {
    /// Falha com código específico.
    pub fn new(code: ExportErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            params: BTreeMap::new(),
            detail: None,
        }
    }

    /// Acrescenta um valor para a frase traduzida.
    #[must_use]
    pub fn with_param(mut self, name: &str, value: impl Into<String>) -> Self {
        self.params.insert(name.to_owned(), value.into());
        self
    }

    /// Acrescenta o detalhe técnico (sem segredos).
    #[must_use]
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// Se a falha tem este código.
    #[must_use]
    pub fn is_code(&self, code: ExportErrorCode) -> bool {
        self.code == code
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

    fn params(&self) -> BTreeMap<String, String> {
        self.params.clone()
    }

    fn detail(&self) -> Option<String> {
        self.detail
            .clone()
            .or_else(|| Some(warden_core::error_chain(self)))
    }
}
