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
//! - [`take_baseline`], [`capture_after_exit`] e [`capture_view`]: "O que mudou durante o
//!   teste" (T15, C-03): a linha de base antes do jogo, a comparação depois, o conflito de três
//!   vias com o pack e o que gravar no pack ([`plan_apply`]); a lista de sempre ignorados é
//!   `data/capture-ignore.txt` ([`CaptureIgnore`]).
//! - [`accept_manual_file`]: download manual de um mod bloqueado da CurseForge (T20).
//! - [`ManualDownloads`], [`provide_file`] e [`DownloadsWatch`]: a lista da T20 com o estado de
//!   cada item, o arquivo escolhido conferido pelo hash antes de qualquer cópia e o vigia da
//!   pasta Downloads (L-06).
//! - [`InstallerCache`], [`CleanInstance`] e [`player_install`]: "Testar como o jogador
//!   recebe" (L-07): o bootstrap e o installer fixados, a instância limpa temporária, a
//!   instalação pelo link do pack e o que o jogador recebeu × o pack.
//! - [`subset`]: a instância de cada rodada da busca do culpado (D-12), com os jars por link
//!   físico a partir da base, configs copiadas e o mundo de teste numa cópia descartável.
//!
//! Limites: a rede é a da `warden-http` (P1-03) e a leitura do pack é a da `warden-packwiz`
//! (P1-01); esta crate não abre o jogo nem instala o loader (é o motor da `warden-launcher`,
//! que recebe o [`GameRequirement`]).

mod baseline;
mod blocked;
mod capture;
mod clean_instance;
mod downloads;
mod error;
mod installer_cache;
mod manifest;
mod manual_downloads;
mod materialize;
mod optional_choices;
pub mod player_install;
pub mod player_tools;
pub mod subset;
mod three_way;

pub use baseline::{
    BASELINE_DIR, Baseline, CaptureIgnore, FileState, MAX_TEXT_BYTES, PackStart, take_baseline,
};
pub use blocked::{BlockedFile, accept_manual_file};
pub use capture::{
    ApplyPlan, CAPTURE_FILE, Capture, CaptureApplyRequest, CaptureChange, CaptureDecision,
    CaptureError, CaptureGroup, CaptureOption, CaptureOutcome, CaptureResolution, CaptureStatus,
    CaptureValue, CapturedFile, VOLATILE_OPTIONS, capture_after_exit, discard, finish_apply,
    load_capture, plan_apply, set_option_lines,
};
pub use clean_instance::{CLEAN_PREFIX, CleanInstance, KEPT_DIR, keep_logs};
pub use downloads::{
    CachedFile, DownloadCache, Fetched, FileOrigin, INCOMING_DIR, INDEX_FILE, OriginSource,
    STALE_PART_AGE,
};
pub use error::{Error, FailedItem, FailureReason, InstanceErrorCode, Result};
pub use installer_cache::{
    BOOTSTRAP, INSTALLER, InstallerCache, InstallerJars, PinnedJar, STAGE_INSTALLER,
    STAGE_INSTALLER_LABEL,
};
pub use manifest::{
    EntryOrigin, MANIFEST_FILE, MANIFEST_SCHEMA, Manifest, ManifestEntry, ModrinthIds, modified_ns,
};
pub use manual_downloads::{
    DownloadsWatch, FoundVia, MAX_WATCHED_FILE_BYTES, ManualDownloadItem, ManualDownloadStatus,
    ManualDownloads, SETTLE, WatchContext, display_name, file_matches, find_by_name, identify,
    is_candidate, provide_file,
};
pub use materialize::{
    GameRequirement, InstallSide, InstanceDirs, LoaderRequirement, MaterializeOptions,
    ModifiedAction, ModifiedFile, OnModified, OptionalSelection, Outcome, Report, STAGE,
    STAGE_LABEL, Sources, game_requirement, materialize,
};
pub use optional_choices::{OPTIONAL_CHOICES_FILE, OptionalChoices};
pub use player_tools::PlayerToolsMode;
pub use three_way::{
    CaptureConflict, CaptureFileView, CaptureTexts, CaptureView, capture_texts, capture_view, merge,
};

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
