//! Erros estáveis do serviço de packs.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use warden_core::{DomainCode, DomainError};

/// Resultado do serviço de packs.
pub type Result<T> = std::result::Result<T, Error>;

/// Códigos do domínio `project`; somente acréscimos são compatíveis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ProjectErrorCode {
    /// Invariante interna quebrada.
    Internal,
    /// Nome ou campo inválido.
    InvalidInput,
    /// Destino já contém arquivos.
    DestinationNotEmpty,
    /// Pack não registrado.
    PackNotFound,
    /// Pasta de um pack registrado desapareceu.
    FolderMissing,
    /// Manifesto não é um packwiz válido.
    InvalidPack,
    /// Loader não suportado ou múltiplo.
    UnsupportedLoader,
    /// A versão escolhida não pertence ao catálogo do loader.
    InvalidLoaderVersion,
    /// Pasta ou identificador já registrado.
    AlreadyRegistered,
    /// Arquivo mudou entre planejamento e escrita.
    PackChangedExternally,
    /// Repositório está em estado somente leitura.
    ReadOnly,
    /// A confirmação para apagar não corresponde ao nome do pack.
    TrashConfirmation,
    /// O item pedido não está no inventário do pack.
    ItemNotFound,
}

/// Erro do serviço de packs, com parâmetro opcional para a frase da interface.
#[derive(Debug, thiserror::Error)]
#[error("{detail}")]
pub struct Error {
    /// Código estável.
    pub code: ProjectErrorCode,
    /// Detalhe técnico, sem segredos.
    pub detail: String,
    /// Parâmetros para tradução.
    pub params: BTreeMap<String, String>,
}

impl Error {
    /// Cria um erro com detalhe técnico.
    pub fn new(code: ProjectErrorCode, detail: impl Into<String>) -> Self {
        Self {
            code,
            detail: detail.into(),
            params: BTreeMap::new(),
        }
    }

    /// Acrescenta um parâmetro de tradução.
    #[must_use]
    pub fn param(mut self, key: &str, value: impl Into<String>) -> Self {
        self.params.insert(key.to_owned(), value.into());
        self
    }
}

impl DomainError for Error {
    type Code = ProjectErrorCode;

    fn code(&self) -> DomainCode<Self::Code> {
        DomainCode::Domain(self.code)
    }
    fn params(&self) -> BTreeMap<String, String> {
        self.params.clone()
    }
}
