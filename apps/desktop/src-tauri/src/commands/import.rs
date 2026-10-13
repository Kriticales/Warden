//! Importar modpack (P1-19; SPEC T24; ARCHITECTURE §12.2).
//!
//! - `import_detect`: o formato de uma pasta escolhida em "Abrir ou importar…" (pasta packwiz
//!   segue para Abrir pack; instâncias, para Importar modpack).
//! - `import_choose_file`: o diálogo nativo "Importar arquivo…" (`.mrpack` ou `.zip`).
//! - `import_analyze`: lê a origem e devolve o resumo da revisão. O plano fica guardado aqui,
//!   por um `token`; nada é gravado no destino (o zip da CurseForge usa uma cópia de staging
//!   no cache, apagada ao esquecer o plano). A chave da CurseForge nunca passa pela interface.
//! - `import_apply`: grava o pack novo pela rotina do T03, registra e cria o perfil do teste
//!   com a memória e os argumentos do modpack (L-08).
//! - `import_discard`: esquece o plano (Cancelar, sair da página).
#![allow(clippy::needless_pass_by_value)] // argumentos pertencem ao contrato do Tauri

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex, PoisonError};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State, WebviewWindow};
use tauri_plugin_dialog::DialogExt as _;
use warden_core::{OperationId, PackId};
use warden_import::{
    CurseforgeTools, ImportPlan, ImportProfile, ImportRequest, ImportServices, ImportSummary,
    ImportFormat, Stores,
};
use warden_secrets::SecretKind;

use crate::commands::add_curseforge::link_packwiz;
use crate::commands::inventory;
use crate::commands::packs::create_defaults;
use crate::error::AppError;
use crate::operations::OperationKind;
use crate::settings::TestMemory;
use crate::state::AppState;
use crate::test_profiles::{self, ProfileFields};

/// "Ler o modpack".
const ANALYZE: OperationKind = OperationKind::new("import.analyze");
/// "Importar modpack".
const APPLY: OperationKind = OperationKind::new("import.apply");

/// Os E2E não alcançam o diálogo nativo: em build de debug, com esta variável, o app lê o
/// arquivo escolhido do arquivo apontado por ela (o mesmo mecanismo de `pack_choose_folder`).
#[cfg(debug_assertions)]
const E2E_PICK_FOLDER_ENV: &str = "WARDEN_E2E_PICK_FOLDER";

/// Quanto tempo um plano vale sem uso.
const PLAN_TTL: Duration = Duration::from_hours(1);

/// Nome do perfil do teste criado com a memória e os argumentos do modpack.
pub(crate) const PROFILE_NAME: &str = "Do modpack importado";

struct Stored {
    plan: Arc<ImportPlan>,
    used: Instant,
}

static PLANS: LazyLock<Mutex<HashMap<String, Stored>>> = LazyLock::new(Mutex::default);

fn plans() -> std::sync::MutexGuard<'static, HashMap<String, Stored>> {
    PLANS.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Guarda o plano e devolve o token; descarta os vencidos.
fn remember(plan: ImportPlan) -> String {
    let token = OperationId::new().to_string();
    let mut map = plans();
    let expired: Vec<String> = map
        .iter()
        .filter(|(_, stored)| stored.used.elapsed() > PLAN_TTL)
        .map(|(token, _)| token.clone())
        .collect();
    for old in expired {
        if let Some(stored) = map.remove(&old) {
            stored.plan.discard();
        }
    }
    map.insert(
        token.clone(),
        Stored {
            plan: Arc::new(plan),
            used: Instant::now(),
        },
    );
    token
}

fn lookup(token: &str) -> Result<Arc<ImportPlan>, AppError> {
    let mut map = plans();
    let stored = map.get_mut(token).ok_or_else(|| {
        AppError::from_domain(&warden_import::Error::new(
            warden_import::ImportErrorCode::PlanExpired,
            "a revisão da importação expirou",
        ))
    })?;
    stored.used = Instant::now();
    Ok(Arc::clone(&stored.plan))
}

fn forget(token: &str) {
    if let Some(stored) = plans().remove(token) {
        stored.plan.discard();
    }
}

fn domain(error: &warden_import::Error) -> AppError {
    AppError::from_domain(error)
}

/// O resumo da revisão, com o token do plano.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ImportAnalysis {
    /// Identifica o plano em `import_apply` e `import_discard`.
    pub(crate) token: String,
    /// O que foi reconhecido e o que acontece com cada arquivo.
    pub(crate) summary: ImportSummary,
}

/// O que a pessoa confirmou na revisão.
#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ImportApply {
    /// Nome do pack novo.
    pub(crate) name: String,
    /// Pasta escolhida; `null` = `<pasta dos packs>/<slug do nome>`.
    pub(crate) destination: Option<PathBuf>,
    /// Itens de "não entram" marcados para entrar.
    pub(crate) include: Vec<String>,
    /// "Levar suas preferências para o pack?".
    pub(crate) take_preferences: bool,
}

/// O pack importado.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ImportResult {
    /// O pack novo.
    pub(crate) pack_id: PackId,
    /// Quantos itens ficaram para revisar (revisão e arquivos locais).
    pub(crate) review: u32,
    /// Se o perfil do teste com a memória do modpack foi criado (e ficou ativo).
    pub(crate) profile_created: bool,
}

/// O formato de uma pasta ou arquivo escolhido.
#[tauri::command]
#[specta::specta]
pub(crate) fn import_detect(path: PathBuf) -> Result<ImportFormat, AppError> {
    ensure_absolute(&path)?;
    warden_import::detect(&path).map_err(|error| domain(&error))
}

/// Abre o diálogo nativo de arquivo (`.mrpack` ou `.zip`). `null` = a pessoa desistiu.
#[tauri::command]
#[specta::specta]
pub(crate) async fn import_choose_file(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, AppState>,
) -> Result<Option<PathBuf>, AppError> {
    #[cfg(debug_assertions)]
    if let Some(file) = std::env::var_os(E2E_PICK_FOLDER_ENV).filter(|value| !value.is_empty()) {
        let picked = std::fs::read_to_string(&file).unwrap_or_default();
        let picked = picked.trim();
        return Ok((!picked.is_empty()).then(|| PathBuf::from(picked)));
    }
    let start = create_defaults(&state).packs_dir;
    let (sender, receiver) = tokio::sync::oneshot::channel();
    let mut dialog = app
        .dialog()
        .file()
        .set_title("Escolha o modpack (.mrpack ou zip da CurseForge ou do Prism)")
        .add_filter("Modpack (.mrpack, .zip)", &["mrpack", "zip"])
        .set_parent(&window);
    if let Some(start) = start.ancestors().find(|dir| dir.is_dir()) {
        dialog = dialog.set_directory(start);
    }
    dialog.pick_file(move |picked| {
        let _ = sender.send(picked.and_then(|path| path.as_path().map(Path::to_path_buf)));
    });
    receiver
        .await
        .map_err(|error| AppError::internal(format!("diálogo de arquivo interrompido: {error}")))
}

/// Lê o modpack e devolve a revisão. Nada é gravado no destino.
#[tauri::command]
#[specta::specta]
pub(crate) async fn import_analyze(
    state: State<'_, AppState>,
    path: PathBuf,
) -> Result<ImportAnalysis, AppError> {
    ensure_absolute(&path)?;
    let curseforge = inventory::curseforge(&state)?;
    let key = state
        .secrets
        .get(SecretKind::Curseforge)
        .map_err(|error| AppError::from_domain(&error))?;
    let packwiz = link_packwiz(&state)?;
    let staging = state.paths.cache_dir().join("staging");
    let operation = state.operations.start(ANALYZE, None, true);
    let token = operation.token().clone();
    let services = ImportServices {
        stores: Stores {
            modrinth: Some(&state.modrinth),
            curseforge: curseforge.has_key().then_some(&curseforge),
        },
        curseforge: CurseforgeTools {
            packwiz: &packwiz,
            key: key.as_ref(),
            staging_parent: &staging,
        },
    };
    let result = warden_import::analyze(&path, &services, &token)
        .await
        .map(|plan| {
            let summary = plan.summary.clone();
            ImportAnalysis {
                token: remember(plan),
                summary,
            }
        })
        .map_err(|error| {
            if token.is_cancelled() {
                AppError::cancelled()
            } else {
                domain(&error)
            }
        });
    operation.finish(result)
}

/// Grava o pack novo, registra e cria o perfil do teste do modpack.
#[tauri::command]
#[specta::specta]
pub(crate) async fn import_apply(
    state: State<'_, AppState>,
    token: String,
    request: ImportApply,
) -> Result<ImportResult, AppError> {
    let plan = lookup(&token)?;
    let packs_dir = create_defaults(&state).packs_dir;
    let destination = warden_project::create::resolve_destination(
        &request.name,
        request.destination.as_deref(),
        &packs_dir,
    )
    .map_err(|error| AppError::from_domain(&error))?;
    let packwiz = inventory::packwiz(&state)?;
    let operation = state.operations.start(APPLY, None, true);
    let cancel = operation.token().clone();
    let result = warden_import::apply(
        &plan,
        &ImportRequest {
            name: request.name.trim().to_owned(),
            destination,
            include: request.include,
            take_preferences: request.take_preferences,
            scratch: state.paths.cache_dir().join("staging"),
        },
        &state.packs,
        &packwiz,
        &operation,
        &cancel,
    )
    .await
    .map_err(|error| {
        if cancel.is_cancelled() {
            AppError::cancelled()
        } else {
            domain(&error)
        }
    });
    let result = result.map(|created| {
        let profile_created = plan
            .summary
            .profile
            .as_ref()
            .is_some_and(|profile| create_profile(&state, created.id, profile));
        forget(&token);
        ImportResult {
            pack_id: created.id,
            review: u32::try_from(plan.review_count()).unwrap_or(u32::MAX),
            profile_created,
        }
    });
    operation.finish(result)
}

/// Esquece o plano (e apaga a cópia de staging do zip da CurseForge).
#[tauri::command]
#[specta::specta]
pub(crate) fn import_discard(token: String) {
    forget(&token);
}

/// Perfil do teste com a memória e os argumentos do modpack, ativo. Falha não desfaz a
/// importação: fica no registro e a tela diz que o perfil não foi criado.
pub(crate) fn create_profile(state: &AppState, pack_id: PackId, profile: &ImportProfile) -> bool {
    let fields = ProfileFields {
        memory: profile
            .memory_mb
            .map_or(TestMemory::Auto, |mb| TestMemory::Fixed { mb }),
        jvm_args: profile.jvm_args.clone(),
        ..ProfileFields::default()
    };
    let created = test_profiles::create_profile(state, pack_id, PROFILE_NAME, fields)
        .and_then(|created| test_profiles::select_profile(state, pack_id, &created.id));
    match created {
        Ok(()) => true,
        Err(error) => {
            tracing::warn!(code = ?error.code, "perfil do modpack importado não criado");
            false
        }
    }
}

fn ensure_absolute(path: &Path) -> Result<(), AppError> {
    if path.is_absolute() {
        Ok(())
    } else {
        Err(AppError::internal("caminho relativo na importação"))
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::state::tests::test_state;

    fn fake_pack(state: &AppState, root: &Path) -> PackId {
        std::fs::create_dir_all(root).unwrap();
        let id = PackId::new();
        state
            .packs
            .insert(warden_project::registry::PackRecord {
                id,
                name: "Importado".into(),
                path: root.to_path_buf(),
                last_test: None,
                extra: std::collections::BTreeMap::new(),
            })
            .unwrap();
        id
    }

    #[test]
    fn perfil_do_modpack_fica_ativo_com_a_memoria_e_os_argumentos() {
        let temp = tempfile::tempdir().unwrap();
        let state = test_state(temp.path());
        let pack = fake_pack(&state, &temp.path().join("pack"));
        assert!(create_profile(
            &state,
            pack,
            &ImportProfile {
                memory_mb: Some(6144),
                jvm_args: "-XX:+UseG1GC".into(),
            },
        ));
        let profiles = test_profiles::profiles_of(&state, pack).unwrap();
        let active = profiles.get(&profiles.active).unwrap();
        assert_eq!(active.name, PROFILE_NAME);
        assert_eq!(active.fields.memory, TestMemory::Fixed { mb: 6144 });
        assert_eq!(active.fields.jvm_args, "-XX:+UseG1GC");
    }

    #[test]
    fn memoria_fora_da_faixa_nao_quebra_a_importacao() {
        let temp = tempfile::tempdir().unwrap();
        let state = test_state(temp.path());
        let pack = fake_pack(&state, &temp.path().join("pack"));
        assert!(!create_profile(
            &state,
            pack,
            &ImportProfile {
                memory_mb: Some(10_000_000),
                jvm_args: String::new(),
            },
        ));
    }

    #[test]
    fn plano_esquecido_ou_desconhecido_expira() {
        let error = lookup("nao-existe").unwrap_err();
        assert_eq!(
            serde_json::to_value(error.code).unwrap()["code"],
            "PLAN_EXPIRED"
        );
        assert!(import_detect(PathBuf::from("relativo")).is_err());
    }
}
