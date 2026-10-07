//! Crate `warden-instance`.
//!
//! Instância de teste: materialização do pack, linha de base e captura das mudanças
//! (ARCHITECTURE §8).
//!
//! - [`materialize`]: monta a instância a partir do pack com a semântica do packwiz-installer
//!   (ADR-0011): lado, opcionais, `preserve`, remoção do que saiu, cache por hash, CurseForge na
//!   hora com os cabeçalhos de download da `warden-curseforge`, pausa antes de sobrescrever
//!   arquivo alterado e lista de bloqueados para a T20. O teste de conformidade compara a
//!   árvore com a do packwiz-installer real.
//! - [`game_requirement`]: versão do Minecraft e versão **exata** do loader do `pack.toml`,
//!   com que o motor do launcher instala o jogo (D7).
//! - [`DownloadCache`]: `cache/downloads/` por SHA-256, com `index.sqlite` dos quatro hashes e
//!   da origem de cada arquivo (gancho 1.1 da ADR-0039).
//! - [`Manifest`], o que o Warden colocou na instância: `state/manifest.json`.
//! - [`OptionalChoices`], as escolhas dos opcionais: `state/optional-choices.json`.
//! - [`player_tools`]: o que das ferramentas do jogador (spark, Crash Assistant) entra em cada
//!   tipo de teste (D16).
//! - [`accept_manual_file`]: download manual de um mod bloqueado da CurseForge (T20).
//!
//! Limites: a rede é a da `warden-http` (P1-03) e a leitura do pack é a da `warden-packwiz`
//! (P1-01); esta crate não abre o jogo nem instala o loader (é o motor da `warden-launcher`,
//! que recebe o [`GameRequirement`]).

mod blocked;
mod downloads;
mod error;
mod manifest;
mod materialize;
mod optional_choices;
pub mod player_tools;

pub use blocked::{BlockedFile, accept_manual_file};
pub use downloads::{
    CachedFile, DownloadCache, Fetched, FileOrigin, INCOMING_DIR, INDEX_FILE, OriginSource,
    STALE_PART_AGE,
};
pub use error::{Error, FailedItem, FailureReason, InstanceErrorCode, Result};
pub use manifest::{
    EntryOrigin, MANIFEST_FILE, MANIFEST_SCHEMA, Manifest, ManifestEntry, ModrinthIds, modified_ns,
};
pub use materialize::{
    GameRequirement, InstallSide, InstanceDirs, LoaderRequirement, MaterializeOptions,
    ModifiedAction, ModifiedFile, OnModified, OptionalSelection, Outcome, Report, STAGE,
    STAGE_LABEL, Sources, game_requirement, materialize,
};
pub use optional_choices::{OPTIONAL_CHOICES_FILE, OptionalChoices};
pub use player_tools::PlayerToolsMode;

/// Pastas temporárias dos testes: dentro de `WARDEN_DATA_ROOT` quando definida (ADR-0053),
/// senão na pasta temporária do sistema.
#[cfg(test)]
pub(crate) mod test_support {
    /// Pasta temporária apagada no fim do teste.
    #[allow(clippy::expect_used)]
    pub(crate) fn temp_dir() -> tempfile::TempDir {
        match std::env::var_os(warden_core::DATA_ROOT_ENV).filter(|root| !root.is_empty()) {
            Some(root) => {
                std::fs::create_dir_all(&root).expect("criar WARDEN_DATA_ROOT");
                tempfile::tempdir_in(root).expect("pasta temporária")
            }
            None => tempfile::tempdir().expect("pasta temporária"),
        }
    }
}
