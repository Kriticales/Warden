//! Crate `warden-export`.
//!
//! Pré-visualização e exportação do pack (pasta, zip, .mrpack, CurseForge e pacote para
//! servidor).
//!
//! Esqueleto criado pela tarefa F0-01. O conteúdo vem das tarefas que o `docs/ROADMAP.md`
//! atribui a esta crate; a responsabilidade e os limites estão na ARCHITECTURE §3.

#![cfg_attr(test, allow(linker_messages))]

mod error;
mod native;
mod prism;

pub use error::{Error, ExportErrorCode, Result};
pub use native::{
    ExportAlert, ExportFormat, ExportPreview, ExportResult, ExportSource, Preflight, PreviewFile,
    PreviewFolder, exclude_from_pack, export, preview,
};
pub use prism::{
    PrismAnalysis, PrismBlockedMod, PrismLocalFile, PrismOptions, PrismResult, PrismServices,
    PrismSwap, STAGE_RESOLVE, STAGE_RESOLVE_LABEL, analyze_prism, curseforge_file_url,
    export_prism,
};
