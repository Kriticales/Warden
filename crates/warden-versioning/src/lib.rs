//! Crate `warden-versioning`: o histórico de versões do pack sobre git embutido
//! (ARCHITECTURE §11; ADR-0015).
//!
//! O dono não vê git: para ele existem "Salvar versão", versões salvas, versão final,
//! "Voltar para esta versão" e pontos de segurança (QUALITY §8.2). Por baixo:
//!
//! - [`PackRepo`]: abre ou cria o repositório na raiz do pack, com o ponto inicial ("Pack
//!   criado" ou "Pack importado"), e recusa escritas com o histórico em estado especial
//!   (merge, rebase, conflitos, HEAD destacado).
//! - Alterações não salvas ([`PackRepo::unsaved_changes`]): árvore de trabalho contra o último
//!   commit.
//! - Versões ([`PackRepo::save_version`], [`PackRepo::versions`], [`PackRepo::set_final`]):
//!   commit "Versão X.Y.Z" + tag anotada `vX.Y.Z`; versão final em `refs/warden/final/*`.
//! - Pontos de segurança ([`PackRepo::create_safety_point`]): commits fora da branch em
//!   `refs/warden/safety/*`.
//! - Restauração transacional ([`PackRepo::restore`]): se falhar no meio, o pack volta ao
//!   estado anterior completo.
//!
//! Git embutido com a `git2` (libgit2 compilada junto): o computador do usuário não precisa
//! ter git instalado. No Windows a compilação usa o compilador C do MSVC, já exigido pelo
//! Rust; nada é instalado. Nenhuma configuração global de fim de linha vale nos repositórios
//! criados pelo Warden (`core.autocrlf = false`); o `.gitattributes` do pack manda.
//!
//! Limites: esta crate só toca no repositório e, na restauração, nos arquivos do pack que a
//! versão de destino define. Gravar `pack.toml` e `CHANGELOG.md` é da `PackTransaction`
//! (P1-07/V-02); publicar no GitHub é da V-03. As operações são bloqueantes (a `warden-app`
//! as chama em `spawn_blocking`) e esperam a trava de escrita do pack (ARCHITECTURE §15).

// A libgit2 compilada com o MSVC declara as funções com `__declspec(dllexport)`, e o linker
// avisa ao criar a biblioteca de importação de cada executável de teste (inofensivo).
#![cfg_attr(test, allow(linker_messages))]

mod error;
mod moment;
mod repo;
mod restore;
mod safety;
mod versions;
mod worktree;

pub use error::{Error, Result, VersioningErrorCode};
pub use moment::Moment;
pub use repo::{
    CREATED_MESSAGE, IMPORTED_MESSAGE, Identity, InitialPoint, LOCAL_EMAIL, MAIN_BRANCH, PackRepo,
};
pub use restore::{RestoreReport, RestoreTarget};
pub use safety::{SAFETY_REF_PREFIX, SafetyPoint, SafetyReason};
pub use versions::{
    FINAL_REF_PREFIX, PUBLISH_TAG_PREFIX, SaveVersion, SavedVersion, parse_version, tag_name,
};
pub use worktree::{FileChange, FileChangeKind};
