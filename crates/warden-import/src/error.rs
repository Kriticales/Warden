//! Códigos de erro do domínio `import` (ARCHITECTURE §5).
//!
//! Criado pela F0-05 só com `INTERNAL`; a P1-19 acrescentou os demais (só acréscimo, ROADMAP
//! §1), as frases em `apps/desktop/src/i18n/errors/import.ts` e o [`Error`], que implementa
//! `warden_core::DomainError`.
//!
//! Mensagens e detalhes nunca levam a pasta do usuário: só o nome do arquivo escolhido ou
//! caminhos relativos ao modpack (QUALITY §10).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use warden_core::{DomainCode, DomainError};

/// Códigos do domínio `import`. O código é contrato: renomear é mudança de contrato;
/// acrescentar é permitido (só acréscimo, ROADMAP §1).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ImportErrorCode {
    /// Bug: invariante quebrada sem código específico.
    Internal,
    /// O arquivo ou a pasta não é nenhum dos formatos que o Warden importa.
    UnknownFormat,
    /// O arquivo está corrompido ou incompleto (zip ilegível, índice inválido).
    Unreadable,
    /// O arquivo tem caminhos perigosos (`..`, caminho absoluto ou letra de drive): recusado
    /// sem criar nada (CA-T24-02).
    UnsafePath,
    /// O modpack usa um loader que o Warden não suporta (Quilt, `LiteLoader`) ou mais de um.
    UnsupportedLoader,
    /// Zip da CurseForge sem a chave da CurseForge: nenhuma requisição foi feita.
    CurseforgeKeyMissing,
    /// O packwiz não conseguiu ler o zip da CurseForge.
    CurseforgeImportFailed,
    /// O arquivo é grande demais ou tem entradas demais (proteção contra *zip bomb*).
    TooLarge,
    /// O arquivo de origem mudou entre a revisão e a importação.
    SourceChanged,
    /// A revisão expirou (o app foi reaberto ou passou muito tempo): leia o arquivo de novo.
    PlanExpired,
    /// O modpack não diz a versão do Minecraft.
    MinecraftMissing,
}

/// Falha da importação. Os detalhes não contêm segredos nem a pasta do usuário.
#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct Error {
    code: ImportErrorCode,
    message: String,
    params: BTreeMap<String, String>,
    detail: Option<String>,
}

impl Error {
    /// Falha com código específico.
    pub fn new(code: ImportErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            params: BTreeMap::new(),
            detail: None,
        }
    }

    /// Falha sem código específico (`INTERNAL`).
    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(ImportErrorCode::Internal, message)
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

    /// O código.
    #[must_use]
    pub fn code(&self) -> ImportErrorCode {
        self.code
    }

    /// Os valores da frase.
    #[must_use]
    pub fn params(&self) -> &BTreeMap<String, String> {
        &self.params
    }
}

/// Resultado das operações de importação.
pub type Result<T> = std::result::Result<T, Error>;

impl DomainError for Error {
    type Code = ImportErrorCode;

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
