//! Publicar versões finais no GitHub para os jogadores (SPEC T18; ARCHITECTURE §11.1;
//! ADR-0028).
//!
//! - [`line`]: a linha de publicação local (`refs/warden/publish/main`), as tags de
//!   publicação, o `CHANGELOG.md` publicado e o estado "Release pendente".
//! - [`run`]: o envio protegido e a Release.
//! - [`secrets`]: a varredura que bloqueia a publicação.
//! - [`checks`]: as conferências plugáveis do `publish_plan` (gancho 1.2).
//! - `transport`: o protocolo "smart HTTP" do git sobre a `warden-http` (ADR-0059).

use std::collections::BTreeMap;

pub mod checks;
pub mod line;
pub mod run;
pub mod secrets;
mod transport;

/// O conteúdo de uma versão como vai para o GitHub: caminho relativo (com `/`) → bytes.
pub type PublishTree = BTreeMap<String, Vec<u8>>;

pub use checks::{PublishCheck, PublishChecks, PublishContext, PublishFindingKind, run_checks};
pub use line::{
    ADDED_FILES, GITATTRIBUTES, PUBLISH_LINE_REF, PreparedPublication, PublishRequest,
    PublishedVersion, RELEASE_PENDING_PREFIX,
};
pub use run::{Published, RemoteStatus, ReplaceRemote, publish, remote_status, retry_release};
pub use secrets::{SecretFinding, scan_secrets};
