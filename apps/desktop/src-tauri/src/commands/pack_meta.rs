//! Informações do pack e Ajustes do teste neste computador (SPEC T11; P1-08).
//!
//! - `pack_meta_get` / `pack_update_meta`: nome, autor e descrição, gravados no `pack.toml`
//!   (só a linha que mudou; CA-T11-01). O nome também atualiza o registro local, para Meus
//!   packs mostrar o nome novo mesmo se a pasta sumir depois.
//! - `pack_test_settings_get` / `pack_test_settings_set`: memória, Java e argumentos da JVM do
//!   teste. Ficam em `packs.json` (dado deste computador, ADR-0025), nunca no pack, e não contam
//!   como alteração não salva. O efeito no jogo chega com o Testar (L-04), que os lê daqui.
//! - `instance_recreate`: "Recriar instância de teste" (apaga a instância; os mundos de teste
//!   ficam se o usuário pedir). O Testar (L-04) reaproveita este comando no menu ▾.
#![allow(clippy::needless_pass_by_value)] // argumentos pertencem ao contrato do Tauri

use std::fs;
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use tauri::{AppHandle, State};
use warden_core::PackId;
use warden_java::RuntimeId;
use warden_project::ProjectErrorCode;
use warden_project::meta::{MetaUpdate, PackMeta};

use crate::commands::inventory::{notify_changed, pack_root};
use crate::error::AppError;
use crate::events::PackArea;
use crate::settings::TestMemory;
use crate::state::AppState;

/// Campo de `PackRecord.extra` (em `packs.json`) com os ajustes do teste do pack.
pub(crate) const TEST_SETTINGS_KEY: &str = "testSettings";

/// Tamanho máximo dos argumentos extras da JVM, em caracteres.
const MAX_JVM_ARGS: usize = 4000;

fn domain(error: warden_project::Error) -> AppError {
    AppError::from_domain(&error)
}

/// Nome, autor, descrição e versões do `pack.toml` (diálogo Informações do pack).
#[tauri::command]
#[specta::specta]
pub(crate) async fn pack_meta_get(
    state: State<'_, AppState>,
    pack_id: PackId,
) -> Result<PackMeta, AppError> {
    let _lock = state.locks.read(pack_id).await;
    let root = pack_root(&state, pack_id)?;
    warden_project::meta::read_meta(&root).map_err(domain)
}

/// Grava nome, autor e descrição no `pack.toml`, mudando só as linhas alteradas.
#[tauri::command]
#[specta::specta]
pub(crate) async fn pack_update_meta(
    app: AppHandle,
    state: State<'_, AppState>,
    pack_id: PackId,
    update: MetaUpdate,
) -> Result<PackMeta, AppError> {
    let meta = update_meta_impl(&state, pack_id, &update).await?;
    notify_changed(&app, pack_id, vec![PackArea::Meta]);
    Ok(meta)
}

async fn update_meta_impl(
    state: &AppState,
    pack_id: PackId,
    update: &MetaUpdate,
) -> Result<PackMeta, AppError> {
    let _lock = state.locks.write(pack_id).await;
    let root = pack_root(state, pack_id)?;
    let meta = warden_project::meta::update_meta(&root, update)
        .await
        .map_err(domain)?;
    state.packs.rename(pack_id, &meta.name).map_err(domain)?;
    Ok(meta)
}

/// Ajustes do teste de um pack neste computador. Campos que esta versão não conhece (de uma
/// versão mais nova, por exemplo os perfis da L-08) são preservados ao gravar.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct TestSettings {
    /// Memória do teste: Automático ou valor fixo.
    pub(crate) memory: TestMemory,
    /// Java escolhido pelo usuário (`null` = Automático).
    pub(crate) java: Option<RuntimeId>,
    /// Argumentos extras da JVM, como o usuário digitou.
    pub(crate) jvm_args: String,
    /// Campos desconhecidos, preservados.
    #[serde(flatten)]
    #[specta(skip)]
    pub(crate) unknown: Map<String, Value>,
}

/// O que o diálogo Ajustes do teste mostra.
#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TestSettingsView {
    /// Os ajustes gravados (ou os padrões).
    pub(crate) settings: TestSettings,
    /// Se a instância de teste já existe (é criada no primeiro teste).
    pub(crate) instance_exists: bool,
    /// Se a instância tem mundos de teste (`saves/` com algo dentro).
    pub(crate) has_worlds: bool,
}

/// Os ajustes do teste do pack e a situação da instância.
#[tauri::command]
#[specta::specta]
pub(crate) fn pack_test_settings_get(
    state: State<'_, AppState>,
    pack_id: PackId,
) -> Result<TestSettingsView, AppError> {
    test_settings_view(&state, pack_id)
}

fn test_settings_view(state: &AppState, pack_id: PackId) -> Result<TestSettingsView, AppError> {
    let settings = read_test_settings(state, pack_id)?;
    let instance = state.paths.instance_dir(pack_id);
    Ok(TestSettingsView {
        settings,
        instance_exists: instance.is_dir(),
        has_worlds: has_entries(&saves_dir(&instance)),
    })
}

/// Lê os ajustes do teste de `packs.json`. Valor ilegível (editado à mão) vale como padrão.
pub(crate) fn read_test_settings(
    state: &AppState,
    pack_id: PackId,
) -> Result<TestSettings, AppError> {
    let record = state.packs.get(pack_id).map_err(domain)?;
    Ok(record
        .extra
        .get(TEST_SETTINGS_KEY)
        .and_then(|value| serde_json::from_value(value.clone()).ok())
        .unwrap_or_default())
}

/// Grava os ajustes do teste (só em `packs.json`; o pack não muda).
#[tauri::command]
#[specta::specta]
pub(crate) fn pack_test_settings_set(
    state: State<'_, AppState>,
    pack_id: PackId,
    settings: TestSettings,
) -> Result<TestSettingsView, AppError> {
    write_test_settings(&state, pack_id, settings)?;
    test_settings_view(&state, pack_id)
}

fn write_test_settings(
    state: &AppState,
    pack_id: PackId,
    mut settings: TestSettings,
) -> Result<(), AppError> {
    let invalid =
        |field: &str| AppError::new(ProjectErrorCode::InvalidInput).with_param("field", field);
    if let TestMemory::Fixed { mb } = settings.memory
        && !(512..=65_536).contains(&mb)
    {
        return Err(invalid("memory"));
    }
    if settings.jvm_args.chars().count() > MAX_JVM_ARGS || settings.jvm_args.contains('\0') {
        return Err(invalid("jvmArgs"));
    }
    settings.jvm_args = settings.jvm_args.trim().to_owned();
    // Preserva o que outra versão gravou e esta não conhece.
    let previous = read_test_settings(state, pack_id)?;
    for (key, value) in previous.unknown {
        settings.unknown.entry(key).or_insert(value);
    }
    let value = serde_json::to_value(&settings)
        .map_err(|error| AppError::internal(format!("ajustes do teste: {error}")))?;
    state
        .packs
        .set_extra(pack_id, TEST_SETTINGS_KEY, Some(value))
        .map_err(domain)
}

/// "Recriar instância de teste": apaga a instância do pack, que é refeita no próximo teste.
/// Com `keep_worlds`, a pasta `saves/` (os mundos de teste) fica.
#[tauri::command]
#[specta::specta]
pub(crate) async fn instance_recreate(
    state: State<'_, AppState>,
    pack_id: PackId,
    keep_worlds: bool,
) -> Result<(), AppError> {
    state.packs.get(pack_id).map_err(domain)?;
    let _lock = state.locks.write(pack_id).await;
    let instance = state.paths.instance_dir(pack_id);
    tauri::async_runtime::spawn_blocking(move || recreate_instance(&instance, keep_worlds))
        .await
        .map_err(|error| AppError::internal(format!("recriar a instância: {error}")))?
}

fn saves_dir(instance: &Path) -> std::path::PathBuf {
    instance.join("minecraft").join("saves")
}

fn has_entries(dir: &Path) -> bool {
    fs::read_dir(dir).is_ok_and(|mut entries| entries.next().is_some())
}

fn recreate_instance(instance: &Path, keep_worlds: bool) -> Result<(), AppError> {
    let io_error = |error: io::Error| AppError::internal(format!("recriar a instância: {error}"));
    if !instance.is_dir() {
        return Ok(());
    }
    if !keep_worlds {
        return fs::remove_dir_all(instance).map_err(io_error);
    }
    let game = instance.join("minecraft");
    for entry in fs::read_dir(instance).map_err(io_error)? {
        let path = entry.map_err(io_error)?.path();
        if path != game {
            remove_any(&path).map_err(io_error)?;
        }
    }
    if game.is_dir() {
        for entry in fs::read_dir(&game).map_err(io_error)? {
            let path = entry.map_err(io_error)?.path();
            if path.file_name().is_none_or(|name| name != "saves") {
                remove_any(&path).map_err(io_error)?;
            }
        }
    }
    Ok(())
}

fn remove_any(path: &Path) -> io::Result<()> {
    if path.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use warden_project::registry::PackRecord;

    use super::*;
    use crate::error::ErrorCode;
    use crate::state::tests::test_state;

    fn register(state: &AppState, root: &Path) -> PackId {
        fs::create_dir_all(root).unwrap();
        let mut manifest = warden_packwiz::PackManifest::new("Vale Sereno", "1.20.1");
        manifest.author = "Autor".into();
        fs::write(root.join("pack.toml"), manifest.to_toml_string()).unwrap();
        fs::write(
            root.join("index.toml"),
            warden_packwiz::PackIndex::default().to_toml_string(),
        )
        .unwrap();
        let record = PackRecord {
            id: PackId::new(),
            name: "Vale Sereno".into(),
            path: root.to_path_buf(),
            last_test: None,
            extra: BTreeMap::new(),
        };
        let id = record.id;
        state.packs.insert(record).unwrap();
        id
    }

    #[tokio::test]
    async fn renomear_grava_o_pack_toml_e_o_registro() {
        let dir = tempfile::tempdir().unwrap();
        let state = test_state(dir.path());
        let root = dir.path().join("pack");
        let id = register(&state, &root);
        let update = MetaUpdate {
            name: "Vale Novo".into(),
            author: "Autor".into(),
            description: String::new(),
        };
        let meta = update_meta_impl(&state, id, &update).await.unwrap();
        assert_eq!(meta.name, "Vale Novo");
        assert!(
            fs::read_to_string(root.join("pack.toml"))
                .unwrap()
                .contains("name = \"Vale Novo\"")
        );
        assert_eq!(state.packs.get(id).unwrap().name, "Vale Novo");
        // Nome vazio é recusado sem escrever nada.
        let empty = MetaUpdate {
            name: "  ".into(),
            ..update
        };
        let error = update_meta_impl(&state, id, &empty).await.unwrap_err();
        assert_eq!(
            error.code,
            ErrorCode::Project(ProjectErrorCode::InvalidInput)
        );
        assert_eq!(state.packs.get(id).unwrap().name, "Vale Novo");
    }

    #[test]
    fn ajustes_do_teste_ficam_no_registro_e_nao_no_pack() {
        let dir = tempfile::tempdir().unwrap();
        let state = test_state(dir.path());
        let root = dir.path().join("pack");
        let id = register(&state, &root);
        let before: Vec<_> = fs::read_dir(&root)
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();

        let view = test_settings_view(&state, id).unwrap();
        assert_eq!(view.settings, TestSettings::default());
        assert!(!view.instance_exists);

        let settings = TestSettings {
            memory: TestMemory::Fixed { mb: 6144 },
            java: None,
            jvm_args: "  -XX:+UseG1GC  ".into(),
            unknown: Map::new(),
        };
        write_test_settings(&state, id, settings).unwrap();
        let saved = read_test_settings(&state, id).unwrap();
        assert_eq!(saved.memory, TestMemory::Fixed { mb: 6144 });
        assert_eq!(saved.jvm_args, "-XX:+UseG1GC");
        let after: Vec<_> = fs::read_dir(&root)
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        assert_eq!(before, after, "nenhum arquivo do pack muda");
        let json = fs::read_to_string(state.paths.packs_registry_file()).unwrap();
        assert!(json.contains("\"testSettings\""));
        assert!(json.contains("\"jvmArgs\": \"-XX:+UseG1GC\""));
    }

    #[test]
    fn ajustes_preservam_campos_desconhecidos_e_validam_a_memoria() {
        let dir = tempfile::tempdir().unwrap();
        let state = test_state(dir.path());
        let id = register(&state, &dir.path().join("pack"));
        state
            .packs
            .set_extra(
                id,
                TEST_SETTINGS_KEY,
                Some(
                    serde_json::json!({ "jvmArgs": "-Da=1", "profiles": [{ "name": "PC fraco" }] }),
                ),
            )
            .unwrap();
        let mut settings = read_test_settings(&state, id).unwrap();
        assert_eq!(settings.jvm_args, "-Da=1");
        settings.jvm_args = "-Db=2".into();
        settings.unknown.clear();
        write_test_settings(&state, id, settings).unwrap();
        let record = state.packs.get(id).unwrap();
        assert_eq!(
            record.extra[TEST_SETTINGS_KEY]["profiles"][0]["name"],
            "PC fraco"
        );

        let too_little = TestSettings {
            memory: TestMemory::Fixed { mb: 128 },
            ..TestSettings::default()
        };
        let error = write_test_settings(&state, id, too_little).unwrap_err();
        assert_eq!(error.params["field"], "memory");
        let huge = TestSettings {
            jvm_args: "x".repeat(MAX_JVM_ARGS + 1),
            ..TestSettings::default()
        };
        assert_eq!(
            write_test_settings(&state, id, huge).unwrap_err().params["field"],
            "jvmArgs"
        );
        // Valor ilegível gravado à mão vale como padrão.
        state
            .packs
            .set_extra(id, TEST_SETTINGS_KEY, Some(serde_json::json!("lixo")))
            .unwrap();
        assert_eq!(read_test_settings(&state, id).unwrap().jvm_args, "");
    }

    /// CA-T11-03 (lado do Rust): as três decisões que o teste de componente de Ajustes do teste
    /// usa (`features/pack-editor/editor.fixtures.ts`, `DECISIONS`) são as da tabela real da L-01.
    #[test]
    fn politica_do_java_das_decisoes_usadas_pela_interface() {
        use warden_java::{JavaChoiceReason, JavaChoiceRequest, LoaderKind};
        let dir = tempfile::tempdir().unwrap();
        let state = test_state(dir.path());
        let cases = [
            (
                JavaChoiceRequest::automatic("1.20.1", LoaderKind::Forge, Some("47.3.0")),
                17,
                JavaChoiceReason::NewestProvenForRange,
                "1.17 a 1.20.4",
            ),
            (
                JavaChoiceRequest::automatic("1.7.10", LoaderKind::Forge, Some("10.13.4.1614")),
                8,
                JavaChoiceReason::ForgeLegacyJava8,
                "até o 1.12.2",
            ),
            (
                JavaChoiceRequest::automatic("26.1", LoaderKind::Vanilla, None),
                25,
                JavaChoiceReason::NewestAvailable,
                "26.x",
            ),
        ];
        for (request, major, reason, range) in cases {
            let choice = state.java.choose(&request).unwrap();
            assert_eq!(choice.major, major, "{request:?}");
            assert_eq!(choice.automatic.reason, reason, "{request:?}");
            assert_eq!(choice.automatic.range_label.as_deref(), Some(range));
            assert_eq!(choice.automatic.newest_major, 25);
        }
    }

    #[test]
    fn recriar_instancia_mantem_so_os_mundos_quando_pedido() {
        let dir = tempfile::tempdir().unwrap();
        let instance = dir.path().join("instancia");
        let game = instance.join("minecraft");
        fs::create_dir_all(game.join("saves/Mundo de teste")).unwrap();
        fs::create_dir_all(game.join("mods")).unwrap();
        fs::create_dir_all(instance.join("state")).unwrap();
        fs::write(game.join("options.txt"), "x").unwrap();
        fs::write(game.join("saves/Mundo de teste/level.dat"), "x").unwrap();
        assert!(has_entries(&saves_dir(&instance)));

        recreate_instance(&instance, true).unwrap();
        assert!(game.join("saves/Mundo de teste/level.dat").is_file());
        assert!(!game.join("mods").exists());
        assert!(!game.join("options.txt").exists());
        assert!(!instance.join("state").exists());

        recreate_instance(&instance, false).unwrap();
        assert!(!instance.exists());
        // Sem instância, não há o que fazer.
        recreate_instance(&instance, false).unwrap();
    }
}
