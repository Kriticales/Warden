//! CurseForge em "Adicionar" (SPEC T08; ADR-0027; P1-10).
//!
//! - As fontes da CurseForge para a busca combinada e para o plano e a gravação, com a chave do
//!   usuário lida na hora do uso (cofre; nunca em log nem em erro). **Sem chave**, a fonte fica
//!   de fora com o aviso e nenhuma requisição sai (CA-T08-06). **Chave recusada** pela API: o
//!   aviso aparece e as buscas seguintes nem tentam, até a chave mudar.
//! - `curseforge_link_resolve`: o link da CurseForge colado no campo único. Link de projeto:
//!   achado pela API. Link de arquivo: lido pelo packwiz numa cópia do pack (o pack de verdade
//!   não é tocado) e seguido pelo caminho normal (`add_plan` e `add_apply`).
//!
//! Finos: leem a chave, pegam a trava do pack e chamam a `warden-project`.
#![allow(clippy::needless_pass_by_value)] // argumentos pertencem ao contrato do Tauri

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash as _, Hasher as _};
use std::sync::{Arc, Mutex, OnceLock};

use secrecy::{ExposeSecret as _, SecretString};
use tauri::State;
use warden_core::{CancellationToken, PackId};
use warden_curseforge::{CurseforgeClient, MemoryCache};
use warden_http::{HttpClient, HttpConfig, SystemTimer};
use warden_project::add::AddSource;
use warden_project::add::curseforge::CurseforgeAdd;
use warden_project::add::link_curseforge::{
    CurseforgeLink, LinkError, LinkTarget, import_file_link, parse_link, resolve_project,
};
use warden_project::search::curseforge::CurseforgeSearch;
use warden_project::search::{
    ActiveSource, ProjectPreview, SearchSource, SourceFailure, SourceId, SourcePage, SourceQuery,
    SourceWarningReason,
};
use warden_project::{ProjectErrorCode, Result as ProjectResult};
use warden_secrets::SecretKind;

use crate::commands::inventory::{pack_root, packwiz};
use crate::error::AppError;
use crate::operations::OperationKind;
use crate::state::AppState;

/// "Ler o link da CurseForge" (cópia do pack e `packwiz curseforge add`).
const LINK: OperationKind = OperationKind::new("add.curseforgeLink");

/// O cache da API da CurseForge: só em memória e o mesmo durante a sessão (termos da
/// CurseForge; os clientes são criados na hora, com a chave lida na hora).
fn shared_cache() -> MemoryCache {
    static CACHE: OnceLock<MemoryCache> = OnceLock::new();
    CACHE
        .get_or_init(|| MemoryCache::new(Arc::new(SystemTimer::new())))
        .clone()
}

/// A chave da CurseForge guardada (`None`: sem chave).
fn stored_key(state: &AppState) -> Result<Option<SecretString>, AppError> {
    state
        .secrets
        .get(SecretKind::Curseforge)
        .map(|key| key.filter(|key| !key.expose_secret().trim().is_empty()))
        .map_err(|error| AppError::from_domain(&error))
}

/// Cliente da CurseForge com a chave dada.
fn client(key: SecretString) -> Result<CurseforgeClient, AppError> {
    let http = HttpClient::new(HttpConfig::for_version(env!("CARGO_PKG_VERSION")))
        .map_err(|error| AppError::from_domain(&error))?;
    CurseforgeClient::new(http, Some(key))
        .map(|client| client.with_cache(shared_cache()))
        .map_err(|error| AppError::from_domain(&error))
}

/// Impressão curta da chave, para lembrar qual foi recusada sem guardar a chave.
fn fingerprint(key: &SecretString) -> u64 {
    let mut hasher = DefaultHasher::new();
    key.expose_secret().hash(&mut hasher);
    hasher.finish()
}

/// A chave que a CurseForge recusou nesta sessão.
fn rejected() -> &'static Mutex<Option<u64>> {
    static REJECTED: OnceLock<Mutex<Option<u64>>> = OnceLock::new();
    REJECTED.get_or_init(|| Mutex::new(None))
}

fn is_rejected(key: &SecretString) -> bool {
    let known = rejected()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    *known == Some(fingerprint(key))
}

fn remember_rejected(key: &SecretString) {
    *rejected()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(fingerprint(key));
}

/// A busca da CurseForge que lembra quando a API recusa a chave: as próximas buscas deixam a
/// fonte de fora sem pedir nada à API.
struct Watched {
    inner: CurseforgeSearch,
    key: SecretString,
}

#[async_trait::async_trait]
impl SearchSource for Watched {
    fn id(&self) -> SourceId {
        SourceId::Curseforge
    }

    async fn preview(
        &self,
        project_id: &str,
        cancel: &CancellationToken,
    ) -> ProjectResult<ProjectPreview> {
        self.inner.preview(project_id, cancel).await
    }

    async fn search(
        &self,
        query: &SourceQuery,
        cancel: &CancellationToken,
    ) -> Result<SourcePage, SourceFailure> {
        let result = self.inner.search(query, cancel).await;
        if matches!(&result, Err(failure) if failure.reason == SourceWarningReason::KeyRejected) {
            remember_rejected(&self.key);
        }
        result
    }
}

/// A CurseForge na busca combinada: consultada, ou de fora com o motivo (sem chave, chave
/// recusada). De fora nunca faz requisição.
pub(crate) fn search_source(state: &AppState) -> ActiveSource {
    let key = match stored_key(state) {
        Ok(Some(key)) => key,
        Ok(None) => {
            return ActiveSource::Off(SourceId::Curseforge, SourceWarningReason::KeyMissing);
        }
        Err(error) => {
            tracing::warn!(code = %error.code, "não foi possível ler a chave da CurseForge");
            return ActiveSource::Off(SourceId::Curseforge, SourceWarningReason::KeyMissing);
        }
    };
    if is_rejected(&key) {
        return ActiveSource::Off(SourceId::Curseforge, SourceWarningReason::KeyRejected);
    }
    match client(key.clone()) {
        Ok(client) => ActiveSource::Ready(Arc::new(Watched {
            inner: CurseforgeSearch::new(client),
            key,
        })),
        Err(error) => {
            tracing::warn!(code = %error.code, "cliente da CurseForge indisponível");
            ActiveSource::Off(SourceId::Curseforge, SourceWarningReason::Unavailable)
        }
    }
}

/// A CurseForge no plano e na gravação: ligada só com chave que a API não recusou.
pub(crate) fn add_source(state: &AppState) -> Option<Arc<dyn AddSource>> {
    let key = stored_key(state).ok().flatten()?;
    if is_rejected(&key) {
        return None;
    }
    let client = client(key).ok()?;
    Some(Arc::new(CurseforgeAdd::new(client)))
}

fn domain(error: warden_project::Error) -> AppError {
    AppError::from_domain(&error)
}

/// Lê o link da CurseForge colado no campo único e diz o que ele aponta.
///
/// - Link de **projeto**: o projeto, achado pela API; a tela abre a pré-visualização com o
///   seletor de versão.
/// - Link de **arquivo**: o packwiz lê o link numa cópia do pack (responde `n` à pergunta das
///   dependências); a tela abre o diálogo de dependências com aquele arquivo exato. Nada é
///   gravado no pack.
///
/// Sem chave da CurseForge: erro `CURSEFORGE_KEY_MISSING`, sem nenhuma requisição.
#[tauri::command]
#[specta::specta]
pub(crate) async fn curseforge_link_resolve(
    state: State<'_, AppState>,
    pack_id: PackId,
    url: String,
) -> Result<LinkTarget, AppError> {
    let link = parse_link(&url).ok_or_else(|| {
        domain(
            warden_project::Error::new(
                ProjectErrorCode::InvalidInput,
                "não é um link de mod, resource pack ou shader da CurseForge",
            )
            .param("field", "link"),
        )
    })?;
    let key = stored_key(&state)?.ok_or_else(|| {
        AppError::from_domain(&warden_curseforge::Error::KeyMissing)
    })?;
    let operation = state.operations.start(LINK, Some(pack_id), true);
    let result = async {
        match &link {
            CurseforgeLink::Project { .. } => {
                resolve_project(&client(key)?, &link, operation.token())
                    .await
                    .map_err(domain)
            }
            CurseforgeLink::File { .. } => {
                let _lock = state.locks.read_for(pack_id, &operation).await;
                let root = pack_root(&state, pack_id)?;
                let staging = state.paths.staging_dir(operation.id());
                import_file_link(
                    &root,
                    &staging,
                    &packwiz(&state)?,
                    Some(&key),
                    url.trim(),
                    operation.token(),
                )
                .await
                .map(|imported| imported.target())
                .map_err(|error| match error {
                    LinkError::Project(error) => domain(error),
                    LinkError::Packwiz(error) => AppError::from_domain(&error),
                })
            }
        }
    }
    .await;
    let result = if operation.is_cancelled() {
        Err(AppError::cancelled())
    } else {
        result
    };
    operation.finish(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_chave_recusada_e_lembrada_sem_guardar_a_chave() {
        let key = SecretString::from("$2a$10$chave-recusada-do-teste".to_owned());
        let other = SecretString::from("$2a$10$outra-chave-do-teste".to_owned());
        assert!(!is_rejected(&key));
        remember_rejected(&key);
        assert!(is_rejected(&key));
        assert!(!is_rejected(&other), "outra chave é tentada de novo");
        *rejected().lock().unwrap() = None;
    }
}
