//! Comandos do domínio `scripts` (ARCHITECTURE §10.2; SPEC T26; C-07): o editor de scripts
//! KubeJS e CraftTweaker dentro de Configs.
//!
//! - `scripts_info`: as ferramentas do pack ("KubeJS 6") e os trechos prontos da versão;
//! - `scripts_ids`: os IDs de itens, blocos, tags e receitas dos jars do pack (cache por hash) e,
//!   com o jogo aberto e o KubeJS 7, os do servidor web do KubeJS;
//! - `scripts_problems`: os erros e avisos do último teste (logs da instância) ou, com o jogo
//!   aberto e o KubeJS 7, ao vivo;
//! - `scripts_reload_route` e `scripts_reload`: o caminho de "Recarregar no jogo" e a recarga;
//! - `scripts_copy_to_instance`: com o jogo aberto, copia para a instância de teste o script
//!   que acabou de ser salvo no pack, sem apagar uma mudança feita direto na instância;
//! - `scripts_open_vscode`: abre a pasta do jogo da instância de teste no VS Code.
//!
//! Os jars vêm do cache de downloads (pelo hash do metafile, sem abrir o jogo), da pasta da
//! instância de teste ou do próprio pack (arquivos locais); o jar do Minecraft, da pasta
//! `shared/versions/` do launcher, quando o jogo já foi instalado por um teste.
//!
//! O token do servidor web do KubeJS 7 é lido e usado só na `warden-scripts`: nenhum comando
//! daqui o devolve, e os erros levam só a rota e o status.
#![allow(clippy::needless_pass_by_value)] // argumentos pertencem ao contrato do Tauri

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::Serialize;
use tauri::State;
use warden_core::{AppPaths, PackId};
use warden_http::ExpectedHash;
use warden_instance::{DownloadCache, InstanceDirs};
use warden_jarmeta::{Limits, read_jar_file};
use warden_packwiz::{HashFormat, PackRead, read_pack};
use warden_scripts::detect::{DetectedMod, ScriptEnv, detect};
use warden_scripts::ids::{IdCache, ScriptIds, ScriptJar, build_ids};
use warden_scripts::problems::{ScriptProblems, read_problems};
use warden_scripts::reload::{
    ReloadContext, ReloadOutcome, ReloadRoute, ServerConsole, execute, plan_reload,
};
use warden_scripts::snippets::{Snippet, snippets_for};
use warden_scripts::sync::{InstanceCopy, copy_to_instance};
use warden_scripts::web::{KubejsWeb, read_web_server_config};
use warden_scripts::{vscode, web};

use crate::commands::inventory::pack_root;
use crate::error::{AppError, AppErrorCode};
use crate::events::GamePhase;
use crate::operations::OperationKind;
use crate::state::AppState;

/// Copiar um script salvo no pack para a instância de teste.
pub(crate) const SCRIPTS_COPY: OperationKind = OperationKind::new("scripts.copyToInstance");

/// As ferramentas de scripts do pack e os trechos prontos de cada uma.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ScriptsInfo {
    /// KubeJS e CraftTweaker encontrados.
    pub(crate) env: ScriptEnv,
    /// Trechos prontos do KubeJS da versão detectada.
    pub(crate) kubejs_snippets: Vec<Snippet>,
    /// Trechos prontos do CraftTweaker da versão detectada.
    pub(crate) crafttweaker_snippets: Vec<Snippet>,
    /// Se a instância de teste já existe (para "Abrir no VS Code").
    pub(crate) instance_ready: bool,
}

fn domain(error: warden_scripts::Error) -> AppError {
    AppError::from_domain(&error)
}

fn join_error(error: tauri::Error) -> AppError {
    AppError::internal(error.to_string())
}

/// Pasta do jogo da instância de teste.
fn game_dir(paths: &AppPaths, pack_id: PackId) -> PathBuf {
    InstanceDirs::for_pack(paths, pack_id).game_dir
}

/// Um jar do pack para os scripts.
#[derive(Debug, Clone)]
struct PackJarFile {
    /// Nome do item ("KubeJS"), ou do arquivo para os locais.
    name: String,
    /// Nome do arquivo.
    file_name: String,
    /// Onde está.
    path: PathBuf,
    /// SHA-256, quando veio do cache de downloads.
    sha256: Option<String>,
}

/// Os jars de `mods/` do pack que existem neste computador.
fn pack_jars(paths: &AppPaths, root: &Path, pack: &PackRead, pack_id: PackId) -> Vec<PackJarFile> {
    let cache = DownloadCache::open(&paths.downloads_cache_dir()).ok();
    let game = game_dir(paths, pack_id);
    let index_dir = Path::new(&pack.pack.value.index.file)
        .parent()
        .map_or_else(|| root.to_path_buf(), |parent| root.join(parent));
    let mut jars = Vec::new();
    for (path, metafile) in pack.valid_metafiles() {
        let folder = path.rsplit_once('/').map_or("", |(folder, _)| folder);
        if !is_mods_jar(folder, &metafile.filename) {
            continue;
        }
        let from_cache = HashFormat::parse(&metafile.download.hash_format)
            .ok()
            .and_then(|format| {
                let expected = ExpectedHash::new(format, metafile.download.hash.clone());
                cache.as_ref()?.find(&expected).ok().flatten()
            });
        let found = if let Some(cached) = from_cache {
            Some((cached.path.clone(), Some(cached.hashes.sha256.clone())))
        } else {
            let relative = format!("{folder}/{}", metafile.filename);
            warden_core::resolve_inside(&game, &relative)
                .ok()
                .filter(|path| path.is_file())
                .map(|path| (path, None))
        };
        if let Some((path, sha256)) = found {
            jars.push(PackJarFile {
                name: metafile.name.clone(),
                file_name: metafile.filename.clone(),
                path,
                sha256,
            });
        }
    }
    if let Ok(index) = &pack.index {
        for entry in &index.value.files {
            if entry.metafile {
                continue;
            }
            let (folder, name) = entry.file.rsplit_once('/').unwrap_or(("", &entry.file));
            if !is_mods_jar(folder, name) {
                continue;
            }
            if let Ok(path) = warden_core::resolve_inside(&index_dir, &entry.file)
                && path.is_file()
            {
                jars.push(PackJarFile {
                    name: name.to_owned(),
                    file_name: name.to_owned(),
                    path,
                    sha256: None,
                });
            }
        }
    }
    jars
}

fn is_mods_jar(folder: &str, file_name: &str) -> bool {
    folder.eq_ignore_ascii_case("mods")
        && Path::new(file_name)
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("jar"))
}

/// O jar do Minecraft instalado pelo launcher, se já existe.
fn minecraft_jar(paths: &AppPaths, pack: &PackRead) -> Option<PathBuf> {
    let version = pack.pack.value.minecraft_version()?;
    if version.is_empty() || version.contains(['/', '\\']) || version.contains("..") {
        return None;
    }
    let path = warden_launcher::EngineDirs::for_app(paths)
        .versions
        .join(version)
        .join(format!("{version}.jar"));
    path.is_file().then_some(path)
}

/// As ferramentas do pack. Só os jars com cara de KubeJS ou CraftTweaker são abertos (o nome
/// do item ou do arquivo diz); o resto do pack nem é lido.
fn detect_tools(jars: &[PackJarFile]) -> ScriptEnv {
    let mut mods = Vec::new();
    for jar in jars {
        let name = format!("{} {}", jar.name, jar.file_name).to_ascii_lowercase();
        if !name.contains("kubejs") && !name.contains("crafttweaker") {
            continue;
        }
        // Um jar lido entra com o id de cada mod que declara: o `detect` reconhece o KubeJS e
        // o CraftTweaker pelo id e deixa de fora os addons (`kubejs_create`, `probejs`…). Só um
        // jar que não pôde ser lido cai no nome do arquivo.
        match read_jar_file(&jar.path, &Limits::default()) {
            Ok(meta) if !meta.mods.is_empty() => {
                mods.extend(meta.mods.into_iter().map(|module| DetectedMod {
                    id: Some(module.id),
                    version: module.version,
                    file_name: jar.file_name.clone(),
                }));
            }
            _ => mods.push(DetectedMod {
                id: None,
                version: None,
                file_name: jar.file_name.clone(),
            }),
        }
    }
    detect(&mods)
}

/// Lê o pack e detecta as ferramentas (bloqueante: rodar fora do laço assíncrono).
fn read_env(
    paths: &AppPaths,
    root: &Path,
    pack_id: PackId,
) -> Result<(PackRead, ScriptEnv), AppError> {
    let pack = read_pack(root).map_err(|error| AppError::from_domain(&error))?;
    let jars = pack_jars(paths, root, &pack, pack_id);
    let env = detect_tools(&jars);
    Ok((pack, env))
}

/// O jogo do teste deste pack está aberto.
fn game_running(state: &AppState, pack_id: PackId) -> bool {
    state
        .tests
        .current()
        .is_some_and(|game| game.pack_id == pack_id && game.state == GamePhase::Running)
}

/// **Ponto de extensão da L-09:** o console (stdin) do servidor local aberto para o pack.
/// Enquanto o servidor local não existe, nunca há console e a recarga cai no comando para
/// copiar. A L-09 devolve aqui o console do servidor aberto (um
/// [`ServerConsole`](warden_scripts::reload::ServerConsole)), e `scripts_reload_route` e
/// `scripts_reload` passam a mandar o comando por ele, sem outra mudança.
fn server_console(_state: &AppState, _pack_id: PackId) -> Option<Arc<dyn ServerConsole>> {
    None
}

/// O cliente do servidor web do KubeJS da instância, se ela tem a configuração.
fn kubejs_web(game: &Path) -> Option<KubejsWeb> {
    let config = read_web_server_config(game)?;
    KubejsWeb::new(config, env!("CARGO_PKG_VERSION")).ok()
}

/// KubeJS 7 ou mais novo.
fn has_kubejs_web(env: &ScriptEnv) -> bool {
    env.kubejs
        .as_ref()
        .and_then(|tool| tool.major)
        .is_some_and(|major| major >= 7)
}

/// As ferramentas de scripts do pack e os trechos prontos da versão.
#[tauri::command]
#[specta::specta]
pub(crate) async fn scripts_info(
    state: State<'_, AppState>,
    pack_id: PackId,
) -> Result<ScriptsInfo, AppError> {
    let _lock = state.locks.read(pack_id).await;
    let root = pack_root(&state, pack_id)?;
    let paths = state.paths.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let (_, env) = read_env(&paths, &root, pack_id)?;
        Ok(ScriptsInfo {
            kubejs_snippets: env.kubejs.as_ref().map(snippets_for).unwrap_or_default(),
            crafttweaker_snippets: env
                .crafttweaker
                .as_ref()
                .map(snippets_for)
                .unwrap_or_default(),
            instance_ready: game_dir(&paths, pack_id).is_dir(),
            env,
        })
    })
    .await
    .map_err(join_error)?
}

/// Os IDs para o autocompletar. Com o jogo aberto e o KubeJS 7, acrescenta os do jogo.
#[tauri::command]
#[specta::specta]
pub(crate) async fn scripts_ids(
    state: State<'_, AppState>,
    pack_id: PackId,
) -> Result<ScriptIds, AppError> {
    let lock = state.locks.read(pack_id).await;
    let root = pack_root(&state, pack_id)?;
    let paths = state.paths.clone();
    let (mut ids, env) = tauri::async_runtime::spawn_blocking(move || {
        let (pack, env) = read_env(&paths, &root, pack_id)?;
        let mut jars = Vec::new();
        if let Some(path) = minecraft_jar(&paths, &pack) {
            jars.push(ScriptJar {
                source: "Minecraft".to_owned(),
                path,
                sha256: None,
            });
        }
        jars.extend(
            pack_jars(&paths, &root, &pack, pack_id)
                .into_iter()
                .map(|jar| ScriptJar {
                    source: jar.name,
                    path: jar.path,
                    sha256: jar.sha256,
                }),
        );
        let cache = IdCache::in_cache_dir(&paths.cache_dir());
        let build = build_ids(&jars, Some(&cache), &Limits::default());
        tracing::debug!(
            jars = build.stats.jars,
            do_cache = build.stats.cache_hits,
            abertos = build.stats.jars_opened,
            "IDs dos scripts"
        );
        Ok::<_, AppError>((build.ids, env))
    })
    .await
    .map_err(join_error)??;
    drop(lock);
    if has_kubejs_web(&env)
        && game_running(&state, pack_id)
        && let Some(web) = kubejs_web(&game_dir(&state.paths, pack_id))
    {
        match web.ids().await {
            Ok(group) => ids.groups.push(group),
            Err(error) => tracing::debug!(%error, "IDs do jogo aberto indisponíveis"),
        }
    }
    Ok(ids)
}

/// Os erros e avisos dos scripts: ao vivo com o KubeJS 7 e o jogo aberto; senão, dos logs do
/// último teste.
#[tauri::command]
#[specta::specta]
pub(crate) async fn scripts_problems(
    state: State<'_, AppState>,
    pack_id: PackId,
) -> Result<ScriptProblems, AppError> {
    let root = pack_root(&state, pack_id)?;
    let paths = state.paths.clone();
    let (env, mut from_logs) = tauri::async_runtime::spawn_blocking(move || {
        let (_, env) = read_env(&paths, &root, pack_id)?;
        Ok::<_, AppError>((env, read_problems(&game_dir(&paths, pack_id))))
    })
    .await
    .map_err(join_error)??;
    if has_kubejs_web(&env)
        && game_running(&state, pack_id)
        && let Some(web) = kubejs_web(&game_dir(&state.paths, pack_id))
    {
        match web.problems().await {
            Ok(live) => {
                // Os do KubeJS vêm ao vivo; os do CraftTweaker continuam do log.
                let mut problems = live;
                problems.extend(from_logs.problems.into_iter().filter(|problem| {
                    problem
                        .log_file
                        .as_deref()
                        .is_some_and(|file| !file.starts_with("logs/kubejs/"))
                }));
                from_logs.problems = problems;
                from_logs.live = true;
            }
            Err(error) => tracing::debug!(%error, "erros ao vivo indisponíveis; lendo os logs"),
        }
    }
    Ok(from_logs)
}

/// O caminho de recarga do script e o contexto usado.
async fn route_for(
    state: &AppState,
    pack_id: PackId,
    path: String,
) -> Result<(ReloadRoute, Option<Arc<dyn ServerConsole>>), AppError> {
    let root = pack_root(state, pack_id)?;
    let paths = state.paths.clone();
    let console = server_console(state, pack_id);
    let has_console = console.is_some();
    let running = game_running(state, pack_id);
    let route = tauri::async_runtime::spawn_blocking(move || {
        let (_, env) = read_env(&paths, &root, pack_id)?;
        let game = game_dir(&paths, pack_id);
        let context = ReloadContext {
            game_running: running,
            server_console: has_console,
            web_server: game.join(web::WEB_SERVER_FILE).is_file()
                && read_web_server_config(&game).is_some(),
        };
        Ok::<_, AppError>(plan_reload(&env, &path, context))
    })
    .await
    .map_err(join_error)??;
    Ok((route, console))
}

/// Como "Recarregar no jogo" funcionaria agora para o script `path` (relativo ao pack).
#[tauri::command]
#[specta::specta]
pub(crate) async fn scripts_reload_route(
    state: State<'_, AppState>,
    pack_id: PackId,
    path: String,
) -> Result<ReloadRoute, AppError> {
    Ok(route_for(&state, pack_id, path).await?.0)
}

/// "Recarregar no jogo": pelo servidor web do KubeJS 7 ou pelo console do servidor local; nos
/// outros casos devolve o caminho (o comando para copiar) sem fazer nada.
#[tauri::command]
#[specta::specta]
pub(crate) async fn scripts_reload(
    state: State<'_, AppState>,
    pack_id: PackId,
    path: String,
) -> Result<ReloadOutcome, AppError> {
    let (route, console) = route_for(&state, pack_id, path).await?;
    let web = match &route {
        ReloadRoute::WebServer { .. } => kubejs_web(&game_dir(&state.paths, pack_id)),
        _ => None,
    };
    execute(route, web.as_ref(), console.as_deref())
        .await
        .map_err(domain)
}

/// Depois de salvar o script `path` no pack com o jogo aberto, copia o texto para a instância
/// de teste (SPEC T26: "salvar grava na instância de teste"). `previous_hash` é o `hash` do
/// arquivo do pack antes de salvar: a instância só é substituída se ainda tinha esse texto, se
/// não tinha o arquivo ou com `overwrite` (a pessoa escolheu substituir). Uma mudança feita
/// direto na instância volta como [`InstanceCopy::InstanceChanged`], sem nada gravado.
#[tauri::command]
#[specta::specta]
pub(crate) async fn scripts_copy_to_instance(
    state: State<'_, AppState>,
    pack_id: PackId,
    path: String,
    previous_hash: String,
    overwrite: bool,
) -> Result<InstanceCopy, AppError> {
    let handle = state.operations.start(SCRIPTS_COPY, Some(pack_id), false);
    let result = async {
        let _lock = state.locks.write_for(pack_id, &handle).await;
        let root = pack_root(&state, pack_id)?;
        let game = game_dir(&state.paths, pack_id);
        tauri::async_runtime::spawn_blocking(move || {
            copy_to_instance(&root, &game, &path, &previous_hash, overwrite).map_err(domain)
        })
        .await
        .map_err(join_error)?
    }
    .await;
    handle.finish(result)
}

/// Abre a pasta do jogo da instância de teste no VS Code.
/// Erros: `app.TEST_INSTANCE_MISSING`, `scripts.VS_CODE_NOT_FOUND`, `scripts.VS_CODE_FAILED`.
#[tauri::command]
#[specta::specta]
pub(crate) fn scripts_open_vscode(
    state: State<'_, AppState>,
    pack_id: PackId,
) -> Result<(), AppError> {
    state
        .packs
        .get(pack_id)
        .map_err(|error| AppError::from_domain(&error))?;
    let game = game_dir(&state.paths, pack_id);
    if !game.is_dir() {
        return Err(AppError::new(AppErrorCode::TestInstanceMissing));
    }
    let path_var = std::env::var_os("PATH");
    let code = vscode::find_vscode(path_var.as_deref(), &vscode::default_locations())
        .ok_or_else(|| domain(warden_scripts::Error::VsCodeNotFound))?;
    vscode::open_folder(&code, &game).map_err(domain)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;
    use std::io::{Cursor, Write};

    use warden_scripts::detect::ScriptTool;
    use zip::write::SimpleFileOptions;

    use super::*;
    use crate::state::tests::test_state;

    fn jar_bytes(entries: &[(&str, &str)]) -> Vec<u8> {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (name, text) in entries {
            writer
                .start_file(*name, SimpleFileOptions::default())
                .unwrap();
            writer.write_all(text.as_bytes()).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    fn kubejs_jar(version: &str) -> Vec<u8> {
        jar_bytes(&[(
            "META-INF/mods.toml",
            &format!(
                "modLoader=\"javafml\"\nloaderVersion=\"[47,)\"\nlicense=\"LGPL\"\n[[mods]]\nmodId=\"kubejs\"\nversion=\"{version}\"\ndisplayName=\"KubeJS\"\n"
            ),
        )])
    }

    /// Um pack Forge 1.20.1 com o KubeJS local em `mods/` e um mod de conteúdo.
    fn pack_with_local_jars(dir: &Path, kubejs_version: &str) -> (AppState, PackId, PathBuf) {
        let state = test_state(dir);
        let root = dir.join("pack");
        fs::create_dir_all(root.join("mods")).unwrap();
        fs::write(
            root.join("pack.toml"),
            "name = \"Vale\"\npack-format = \"packwiz:1.1.0\"\n[index]\nfile = \"index.toml\"\nhash-format = \"sha256\"\nhash = \"\"\n[versions]\nminecraft = \"1.20.1\"\nforge = \"47.3.0\"\n",
        )
        .unwrap();
        fs::write(
            root.join("mods/kubejs-forge-2001.6.5-build.26.jar"),
            kubejs_jar(kubejs_version),
        )
        .unwrap();
        fs::write(
            root.join("mods/conteudo.jar"),
            jar_bytes(&[
                ("data/forge/tags/items/ingots/zinc.json", "{}"),
                ("assets/conteudo/models/item/engrenagem.json", "{}"),
            ]),
        )
        .unwrap();
        fs::write(
            root.join("index.toml"),
            "hash-format = \"sha256\"\n\n[[files]]\nfile = \"mods/kubejs-forge-2001.6.5-build.26.jar\"\nhash = \"\"\n\n[[files]]\nfile = \"mods/conteudo.jar\"\nhash = \"\"\n",
        )
        .unwrap();
        let record = warden_project::registry::PackRecord {
            id: PackId::new(),
            name: "Vale".into(),
            path: root.clone(),
            last_test: None,
            extra: BTreeMap::new(),
        };
        let id = record.id;
        state.packs.insert(record).unwrap();
        (state, id, root)
    }

    /// CA-T26-01 (domínio): KubeJS 6 num pack Forge 1.20.1, trechos da versão e o
    /// `minecraft:copper_ingot` vindo do jar do Minecraft instalado.
    #[test]
    fn detecta_o_kubejs_6_e_monta_os_ids_dos_jars() {
        let dir = tempfile::tempdir().unwrap();
        let (state, id, root) = pack_with_local_jars(dir.path(), "2001.6.5-build.26");
        let versions = warden_launcher::EngineDirs::for_app(&state.paths)
            .versions
            .join("1.20.1");
        fs::create_dir_all(&versions).unwrap();
        fs::write(
            versions.join("1.20.1.jar"),
            jar_bytes(&[
                ("assets/minecraft/models/item/copper_ingot.json", "{}"),
                ("assets/minecraft/models/item/copper_block.json", "{}"),
                ("assets/minecraft/blockstates/copper_ore.json", "{}"),
            ]),
        )
        .unwrap();

        let (pack, env) = read_env(&state.paths, &root, id).unwrap();
        let kubejs = env.kubejs.clone().unwrap();
        assert_eq!(kubejs.tool, ScriptTool::Kubejs);
        assert_eq!(kubejs.major, Some(6));
        assert!(env.crafttweaker.is_none());
        let snippets = snippets_for(&kubejs);
        assert!(snippets.iter().any(|s| s.body.starts_with("ServerEvents.recipes")));

        let jars = pack_jars(&state.paths, &root, &pack, id);
        assert_eq!(jars.len(), 2);
        let mut script_jars = vec![ScriptJar {
            source: "Minecraft".into(),
            path: minecraft_jar(&state.paths, &pack).unwrap(),
            sha256: None,
        }];
        script_jars.extend(jars.into_iter().map(|jar| ScriptJar {
            source: jar.name,
            path: jar.path,
            sha256: jar.sha256,
        }));
        let cache = IdCache::in_cache_dir(&state.paths.cache_dir());
        let build = build_ids(&script_jars, Some(&cache), &Limits::default());
        let all: Vec<&str> = build
            .ids
            .groups
            .iter()
            .flat_map(|g| g.items.iter().map(String::as_str))
            .collect();
        let copper: Vec<&str> = all
            .iter()
            .copied()
            .filter(|id| id.starts_with("minecraft:copper_"))
            .collect();
        assert_eq!(copper, ["minecraft:copper_block", "minecraft:copper_ingot"]);
        assert!(all.contains(&"conteudo:engrenagem"));
        // Segunda vez: tudo do cache.
        let again = build_ids(&script_jars, Some(&cache), &Limits::default());
        assert_eq!(again.stats.jars_opened, 0);
    }

    #[test]
    fn rota_sem_jogo_e_com_o_jogo_aberto() {
        let dir = tempfile::tempdir().unwrap();
        let (state, id, _) = pack_with_local_jars(dir.path(), "2001.6.5-build.26");
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let (route, console) = runtime
            .block_on(route_for(&state, id, "kubejs/server_scripts/receitas.js".into()))
            .unwrap();
        assert_eq!(route, ReloadRoute::NoGame);
        assert!(console.is_none());
        let (route, _) = runtime
            .block_on(route_for(&state, id, "config/a.toml".into()))
            .unwrap();
        assert_eq!(route, ReloadRoute::NotAScript);
    }

    #[test]
    fn versao_do_kubejs_pelo_nome_do_arquivo_quando_o_descritor_nao_diz() {
        let dir = tempfile::tempdir().unwrap();
        let (state, id, root) = pack_with_local_jars(dir.path(), "${file.jarVersion}");
        let (_, env) = read_env(&state.paths, &root, id).unwrap();
        assert_eq!(env.kubejs.unwrap().major, Some(6));
    }

    fn addon_jar(mod_id: &str, version: &str) -> Vec<u8> {
        jar_bytes(&[(
            "META-INF/mods.toml",
            &format!(
                "modLoader=\"javafml\"\nloaderVersion=\"[47,)\"\nlicense=\"LGPL\"\n[[mods]]\nmodId=\"{mod_id}\"\nversion=\"{version}\"\n[[dependencies.{mod_id}]]\nmodId=\"kubejs\"\nmandatory=true\nversionRange=\"[2001.6,)\"\nordering=\"AFTER\"\nside=\"BOTH\"\n"
            ),
        )])
    }

    /// Defeito da 1ª revisão: KubeJS 6 com o KubeJS Create (e outros addons comuns) aparecia
    /// como "KubeJS 2", porque o packwiz lista `kubejs-create` antes de `kubejs`. O pack real,
    /// nas duas ordens do `index.toml`.
    #[test]
    fn kubejs_6_com_addons_nas_duas_ordens() {
        let addons = [
            (
                "kubejs-create-forge-2001.2.5-build.2.jar",
                "kubejs_create",
                "2001.2.5-build.2",
            ),
            (
                "kubejs-thermal-forge-2001.1.10-build.3.jar",
                "kubejs_thermal",
                "2001.1.10-build.3",
            ),
            ("probejs-6.0.1-forge.jar", "probejs", "6.0.1"),
            (
                "kubejsadditions-forge-4.3.3.jar",
                "kubejsadditions",
                "4.3.3",
            ),
        ];
        for reverse in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let (state, id, root) = pack_with_local_jars(dir.path(), "2001.6.5-build.26");
            let mut files = vec!["mods/kubejs-forge-2001.6.5-build.26.jar".to_owned()];
            for (file, mod_id, version) in addons {
                fs::write(root.join("mods").join(file), addon_jar(mod_id, version)).unwrap();
                files.push(format!("mods/{file}"));
            }
            files.sort();
            if reverse {
                files.reverse();
            }
            let mut index = String::new();
            for file in &files {
                index.push_str("\n[[files]]\nfile = \"");
                index.push_str(file);
                index.push_str("\"\nhash = \"\"\n");
            }
            fs::write(
                root.join("index.toml"),
                format!("hash-format = \"sha256\"\n{index}"),
            )
            .unwrap();
            let (pack, env) = read_env(&state.paths, &root, id).unwrap();
            assert_eq!(pack_jars(&state.paths, &root, &pack, id).len(), 5);
            let kubejs = env.kubejs.unwrap();
            assert_eq!(kubejs.major, Some(6), "ordem invertida: {reverse}");
            assert_eq!(kubejs.version.as_deref(), Some("2001.6.5-build.26"));
            assert!(env.crafttweaker.is_none());
        }
    }

    #[test]
    fn vscode_sem_instancia() {
        let dir = tempfile::tempdir().unwrap();
        let (state, id, _) = pack_with_local_jars(dir.path(), "2001.6.5-build.26");
        assert!(!game_dir(&state.paths, id).is_dir());
        let missing = PackId::new();
        assert!(state.packs.get(missing).is_err());
    }
}
