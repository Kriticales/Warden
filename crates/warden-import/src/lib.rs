//! Crate `warden-import`.
//!
//! Importação de .mrpack, zip da CurseForge e instâncias de outros launchers para o formato
//! packwiz (P1-19; SPEC T24; ADR-0035; ARCHITECTURE §12.2).
//!
//! Duas etapas, com a revisão da pessoa no meio:
//!
//! 1. [`analyze`]: reconhece o formato pelo conteúdo, lê a origem (sem nunca alterá-la), separa
//!    o que vira referência à fonte, o que fica como arquivo local (com o motivo), o que precisa
//!    de revisão e o que não entra (higiene), e devolve um [`ImportPlan`]. Nada é gravado no
//!    destino; só o zip da CurseForge usa uma cópia de staging no cache do app.
//! 2. [`apply`]: grava o pack novo numa pasta temporária, confere com o packwiz e só então o
//!    põe no lugar e registra. Cancelar ou falhar no meio não deixa pack pela metade.

mod apply;
mod archive;
mod build;
mod common;
mod curseforge;
mod detect;
mod error;
mod lookup;
mod model;
mod mrpack;
mod prism;

use std::path::Path;

pub use apply::{ImportRequest, ImportedPack, apply};
pub use archive::unsafe_reason;
pub use curseforge::CurseforgeTools;
pub use detect::detect;
pub use error::{Error, ImportErrorCode, Result};
pub use lookup::Stores;
pub use model::{
    ExcludedItem, ExcludedReason, ImportPlan, ImportProfile, ImportSummary, ItemSource,
    LocalItem, LocalReason, PackLicense, ReferenceItem, ReviewItem, ReviewReason, SourceKind,
};
use warden_core::CancellationToken;

/// O que a análise usa: as lojas (procura pelo hash, lado e licença) e o packwiz com a chave
/// (zip da CurseForge).
pub struct ImportServices<'a> {
    /// Modrinth e CurseForge.
    pub stores: Stores<'a>,
    /// Packwiz, chave e pasta de staging.
    pub curseforge: CurseforgeTools<'a>,
}

/// Lê a origem e monta o plano da importação, sem gravar nada no destino.
///
/// Erros: [`ImportErrorCode::UnknownFormat`] (inclusive pasta packwiz, que vai para Abrir
/// pack), [`ImportErrorCode::Unreadable`], [`ImportErrorCode::UnsafePath`],
/// [`ImportErrorCode::TooLarge`], [`ImportErrorCode::UnsupportedLoader`],
/// [`ImportErrorCode::MinecraftMissing`], [`ImportErrorCode::CurseforgeKeyMissing`],
/// [`ImportErrorCode::CurseforgeImportFailed`].
pub async fn analyze(
    path: &Path,
    services: &ImportServices<'_>,
    cancel: &CancellationToken,
) -> Result<ImportPlan> {
    let (source, detected) = detect::open(path)?;
    let mut source = source.within(&detected.root);
    let stores = services.stores;
    let draft = match detected.kind {
        SourceKind::Packwiz => {
            return Err(Error::new(
                ImportErrorCode::UnknownFormat,
                "pasta packwiz: abre pelo Abrir pack",
            )
            .with_param("name", source.file_name())
            .with_param("kind", "packwiz"));
        }
        SourceKind::Mrpack => mrpack::analyze(&mut source, path, stores, cancel).await?,
        SourceKind::CurseforgeZip => {
            curseforge::analyze_zip(&mut source, path, stores, &services.curseforge, cancel)
                .await?
        }
        SourceKind::Prism => prism::analyze(&mut source, path, &detected.root)?,
        SourceKind::CurseforgeApp => {
            curseforge::analyze_app(&mut source, path, stores, cancel).await?
        }
    };
    let staging = draft.staging.clone();
    let plan = build::finish(draft, &mut source, stores, cancel).await;
    if plan.is_err()
        && let Some(staging) = staging
    {
        let _ = std::fs::remove_dir_all(staging);
    }
    plan
}
