//! Comandos do domínio `java` (ARCHITECTURE §4.1 e §7.3; T21 → Teste → Java; L-01).
//!
//! - `java_choice`: o Java automático de um pack, com o motivo (usado por Ajustes do teste,
//!   P1-08, e pelo Testar, L-04).
//! - `java_runtimes_list`: a tabela de Java de Configurações (cada Java, os packs que o usam e
//!   por quê; CA-T21-04).
//! - `java_runtimes_check_updates`: "Procurar atualizações do Java", operação longa com
//!   progresso e cancelamento (painel Tarefas).
//! - `java_runtime_remove` e `java_runtimes_remove_unused`.
//!
//! Os packs da tabela vêm de [`JavaPackSources`]: o registro de packs (P1-07) registra quem
//! sabe listá-los com a versão do Minecraft, o loader e a escolha do usuário. Sem fonte
//! registrada, a tabela mostra só os Javas, sem packs.

use std::sync::{Arc, PoisonError, RwLock};

use tauri::State;
use warden_java::{
    JavaChoice, JavaChoiceRequest, JavaOverview, JavaRuntimes, PackJavaInput, RuntimeId,
    UpdateReport,
};

use crate::error::AppError;
use crate::operations::OperationKind;
use crate::state::AppState;

/// Tipo da operação "Procurar atualizações do Java".
pub(crate) const CHECK_UPDATES: OperationKind = OperationKind::new("java.checkUpdates");

impl From<warden_java::Error> for AppError {
    fn from(error: warden_java::Error) -> Self {
        Self::from_domain(&error)
    }
}

/// Quem sabe listar os packs para a tabela de Java.
pub trait JavaPackSource: Send + Sync {
    /// Os packs, com o pedido de escolha do Java de cada um.
    fn packs(&self) -> Result<Vec<PackJavaInput>, AppError>;
}

/// A fonte de packs registrada (registro acréscimo-apenas: a P1-07 registra a sua).
#[derive(Default)]
pub struct JavaPackSources {
    source: RwLock<Option<Arc<dyn JavaPackSource>>>,
}

impl std::fmt::Debug for JavaPackSources {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let registered = self
            .source
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .is_some();
        f.debug_struct("JavaPackSources")
            .field("registered", &registered)
            .finish()
    }
}

impl JavaPackSources {
    /// Registra (ou troca) a fonte de packs.
    pub fn register(&self, source: Arc<dyn JavaPackSource>) {
        *self.source.write().unwrap_or_else(PoisonError::into_inner) = Some(source);
    }

    /// Os packs da fonte registrada (vazio sem fonte).
    pub fn packs(&self) -> Result<Vec<PackJavaInput>, AppError> {
        let source = self
            .source
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        source.map_or_else(|| Ok(Vec::new()), |source| source.packs())
    }
}

/// Roda trabalho de disco fora da thread do IPC.
async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, AppError> + Send + 'static,
) -> Result<T, AppError> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|error| AppError::internal(format!("tarefa do Java interrompida: {error}")))?
}

/// O Java que o teste do pack vai usar e por quê (ADR-0029).
#[tauri::command]
#[specta::specta]
pub(crate) async fn java_choice(
    state: State<'_, AppState>,
    request: JavaChoiceRequest,
) -> Result<JavaChoice, AppError> {
    let java = state.java.clone();
    blocking(move || Ok(java.choose(&request)?)).await
}

/// A tabela de Java de Configurações.
#[tauri::command]
#[specta::specta]
pub(crate) async fn java_runtimes_list(
    state: State<'_, AppState>,
) -> Result<JavaOverview, AppError> {
    let packs = state.java_packs.packs()?;
    let java = state.java.clone();
    blocking(move || list_impl(&java, &packs)).await
}

/// "Procurar atualizações do Java": instala a atualização mais nova de cada Java instalado e
/// remove as antigas que nenhum jogo usa.
#[tauri::command]
#[specta::specta]
pub(crate) async fn java_runtimes_check_updates(
    state: State<'_, AppState>,
) -> Result<UpdateReport, AppError> {
    let handle = state.operations.start(CHECK_UPDATES, None, true);
    let token = handle.token().clone();
    // O cancelamento vai pelo token até a `warden-java`, que para entre as etapas e apaga a
    // pasta temporária da instalação em andamento (descartar a tarefa no meio deixaria a
    // pasta para a limpeza da próxima abertura).
    let result = check_updates_impl(&state.java, &handle, &token).await;
    handle.finish(result)
}

async fn check_updates_impl(
    java: &JavaRuntimes,
    progress: &dyn warden_core::ProgressSink,
    cancel: &warden_core::CancellationToken,
) -> Result<UpdateReport, AppError> {
    let report = java.check_updates(progress, cancel).await?;
    tracing::info!(
        updated = report.updated.len(),
        removed = report.removed.len(),
        failed = report.failed.len(),
        "atualizações do Java conferidas"
    );
    Ok(report)
}

/// Remove um Java e devolve a tabela atualizada.
#[tauri::command]
#[specta::specta]
pub(crate) async fn java_runtime_remove(
    state: State<'_, AppState>,
    id: RuntimeId,
) -> Result<JavaOverview, AppError> {
    let packs = state.java_packs.packs()?;
    let java = state.java.clone();
    blocking(move || {
        java.remove(&id)?;
        tracing::info!(%id, "Java removido");
        list_impl(&java, &packs)
    })
    .await
}

/// "Remover Javas sem uso" e devolve a tabela atualizada.
#[tauri::command]
#[specta::specta]
pub(crate) async fn java_runtimes_remove_unused(
    state: State<'_, AppState>,
) -> Result<JavaOverview, AppError> {
    let packs = state.java_packs.packs()?;
    let java = state.java.clone();
    blocking(move || {
        let removed = java.remove_unused(&packs)?;
        tracing::info!(removed = removed.len(), "Javas sem uso removidos");
        list_impl(&java, &packs)
    })
    .await
}

fn list_impl(java: &JavaRuntimes, packs: &[PackJavaInput]) -> Result<JavaOverview, AppError> {
    Ok(java.overview(packs)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ErrorCode;
    use crate::state::tests::test_state;
    use warden_core::PackId;
    use warden_java::{JavaChoiceReason, JavaErrorCode, LoaderKind};

    struct FixedPacks(Vec<PackJavaInput>);

    impl JavaPackSource for FixedPacks {
        fn packs(&self) -> Result<Vec<PackJavaInput>, AppError> {
            Ok(self.0.clone())
        }
    }

    #[test]
    fn sem_fonte_registrada_a_tabela_nao_tem_packs() {
        let dir = tempfile::tempdir().unwrap();
        let state = test_state(dir.path());
        assert!(state.java_packs.packs().unwrap().is_empty());
        let overview = list_impl(&state.java, &[]).unwrap();
        assert!(overview.runtimes.is_empty());
        assert!(overview.packs_to_download.is_empty());
    }

    #[test]
    fn fonte_registrada_entra_na_tabela_com_o_motivo() {
        let dir = tempfile::tempdir().unwrap();
        let state = test_state(dir.path());
        let pack = PackJavaInput {
            pack_id: PackId::new(),
            name: "GTNH".into(),
            request: JavaChoiceRequest::automatic(
                "1.7.10",
                LoaderKind::Forge,
                Some("10.13.4.1614"),
            ),
        };
        state.java_packs.register(Arc::new(FixedPacks(vec![pack])));
        let packs = state.java_packs.packs().unwrap();
        let overview = list_impl(&state.java, &packs).unwrap();
        assert_eq!(overview.packs_to_download.len(), 1);
        assert_eq!(
            overview.packs_to_download[0].choice.reason,
            JavaChoiceReason::ForgeLegacyJava8
        );
    }

    #[test]
    fn erro_do_java_vira_app_error_do_dominio() {
        let error = AppError::from(warden_java::Error::RuntimeInUse { id: "x".into() });
        assert_eq!(error.code, ErrorCode::Java(JavaErrorCode::RuntimeInUse));
        assert_eq!(error.params["id"], "x");
    }

    #[tokio::test]
    async fn procurar_atualizacoes_sem_javas_nao_pede_nada() {
        let dir = tempfile::tempdir().unwrap();
        let state = test_state(dir.path());
        let report = check_updates_impl(
            &state.java,
            &warden_core::NoProgress,
            &warden_core::CancellationToken::new(),
        )
        .await
        .unwrap();
        assert!(report.updated.is_empty() && report.failed.is_empty());
    }

    #[test]
    fn remover_java_inexistente() {
        let dir = tempfile::tempdir().unwrap();
        let state = test_state(dir.path());
        let id = RuntimeId::parse("temurin-21-jdk-21.0.12.1+1-x64").unwrap();
        let error = AppError::from(state.java.remove(&id).unwrap_err());
        assert_eq!(error.code, ErrorCode::Java(JavaErrorCode::RuntimeNotFound));
    }
}
