//! Comandos do raio-x de mixins (SPEC T07 "O que este mod altera no jogo"; ARCHITECTURE §9.6;
//! D-10).
//!
//! - `xray_mod`: o relatório de um mod do pack (resumo, sobreposições com outros mods e todas as
//!   partes do jogo que ele altera), para o bloco dos detalhes do item.
//!
//! Finos: a leitura e o cruzamento são da `warden-mixin`, os dados curados e o contexto do pack
//! são da `warden-diagnostics` (`pretest::mixin_overlap`, o mesmo do aviso `W_MIXIN_OVERLAP` da
//! seção Problemas). Os jars vêm da instância de teste (e o arquivo guardado no próprio pack),
//! as opções desligadas das configs da instância e o elevador "travamento" da última sessão do
//! Testar, se ela travou. O cache fica em `cache/xray/<pack>.bin`. Só leitura.
#![allow(clippy::needless_pass_by_value)] // argumentos pertencem ao contrato do Tauri

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::State;
use warden_core::PackId;
use warden_diagnostics::pretest::mixin_overlap;
use warden_instance::InstanceDirs;
use warden_launcher::outcome::Outcome;
use warden_mixin::cache::XrayCache;
use warden_mixin::overlap::{CrashCitation, crash_citations};
use warden_mixin::pack::{CacheSlot, ModReport, PackJar};
use warden_project::ProjectErrorCode;
use warden_diagnostics::Finding;
use warden_project::inventory::{InventoryItem, ItemKind, ItemState};

use crate::commands::graph::{existing, jar_locations};
use crate::commands::inventory::pack_root;
use crate::error::AppError;
use crate::state::AppState;

/// Maior log lido para achar as configs citadas (o resto do arquivo é ignorado).
const MAX_LOG_BYTES: u64 = 32 * 1024 * 1024;

/// Situação do raio-x de um item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) enum XrayState {
    /// Lido.
    Ready,
    /// O jar ainda não está no computador (o pack nunca foi testado depois de adicionar o mod).
    NoJar,
}

/// "O que este mod altera no jogo".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct XrayModView {
    /// Situação.
    pub(crate) state: XrayState,
    /// O relatório (só em [`XrayState::Ready`]).
    pub(crate) report: Option<ModReport>,
    /// Nome de cada item citado (caminho no pack → nome na lista).
    pub(crate) names: BTreeMap<String, String>,
    /// O loader do pack é Forge ou NeoForge (para a frase dos mods do Sinytra Connector).
    pub(crate) native_loader: bool,
}

/// O raio-x de um mod do pack. Erro `ITEM_NOT_FOUND` se o item não é um mod da lista.
#[tauri::command]
#[specta::specta]
pub(crate) async fn xray_mod(
    state: State<'_, AppState>,
    pack_id: PackId,
    path: String,
) -> Result<XrayModView, AppError> {
    xray_mod_impl(&state, pack_id, &path).await
}

/// O raio-x de um mod (também para a ferramenta `get_mixin_report` da IA, D-14).
pub(crate) async fn xray_mod_impl(
    state: &AppState,
    pack_id: PackId,
    path: &str,
) -> Result<XrayModView, AppError> {
    let (root, inventory, loader) = {
        let _lock = state.locks.read(pack_id).await;
        let root = pack_root(state, pack_id)?;
        let inventory = warden_project::inventory::inventory(&root, Some(&state.modrinth))
            .await
            .map_err(|error| AppError::from_domain(&error))?;
        let loader = warden_project::meta::read_meta(&root)
            .map_err(|error| AppError::from_domain(&error))?
            .loader;
        (root, inventory, loader)
    };
    let not_found = || AppError::new(ProjectErrorCode::ItemNotFound).with_param("path", path);
    let item = inventory
        .items
        .iter()
        .find(|item| item.path == path)
        .ok_or_else(not_found)?;
    if item.kind != ItemKind::Mod {
        return Err(not_found());
    }
    let dirs = InstanceDirs::for_pack(&state.paths, pack_id);
    let game_dir = dirs.game_dir.clone();
    let jars = pack_jars(&inventory.items, &root, &game_dir);
    let names: BTreeMap<String, String> = inventory
        .items
        .iter()
        .map(|item| (item.path.clone(), item.name.clone()))
        .collect();
    let loader = crate::commands::graph::loader_of(loader.as_deref());
    let native_loader = matches!(
        loader,
        warden_jarmeta::Loader::Forge | warden_jarmeta::Loader::NeoForge
    );
    if !jars.iter().any(|jar| jar.item == path) {
        return Ok(XrayModView {
            state: XrayState::NoJar,
            report: None,
            names,
            native_loader,
        });
    }

    let crash = last_crash_citations(state, pack_id);
    let cache_dir = state.paths.cache_dir();
    let key = pack_id.to_string();
    let item_path = path.to_owned();
    let report = tokio::task::spawn_blocking(move || {
        let cache = XrayCache::in_cache_dir(&cache_dir);
        let config_dir = game_dir.join("config");
        let xray = mixin_overlap::xray(
            &jars,
            loader,
            Some(&config_dir),
            Some(CacheSlot {
                cache: &cache,
                key: &key,
            }),
            crash,
        )
        .map_err(|error| AppError::from_domain(&error))?;
        tracing::debug!(stats = ?xray.scan.stats, overlaps = xray.overlaps.len(), "raio-x de mixins");
        Ok::<_, AppError>(xray.mod_report(&item_path))
    })
    .await
    .map_err(|error| AppError::internal(format!("raio-x de mixins: {error}")))??;

    Ok(XrayModView {
        state: if report.is_some() {
            XrayState::Ready
        } else {
            XrayState::NoJar
        },
        report,
        names,
        native_loader,
    })
}

/// Os jars dos mods do pack que existem no computador: na instância de teste e, para arquivo
/// guardado no pack, na pasta do pack (os mesmos lugares do grafo de dependências).
pub(crate) fn pack_jars(items: &[InventoryItem], root: &Path, game_dir: &Path) -> Vec<PackJar> {
    items
        .iter()
        .filter(|item| item.kind == ItemKind::Mod && item.state == ItemState::Ok)
        .filter_map(|item| {
            let places = jar_locations(item, root, game_dir);
            existing(&places).map(|(found, _, _)| PackJar::new(item.path.clone(), found.clone()))
        })
        .collect()
}

/// O que o pré-teste precisa para o aviso `W_MIXIN_OVERLAP` (montado fora da thread de disco).
pub(crate) struct PretestXray {
    jars: Vec<PackJar>,
    config_dir: PathBuf,
    cache_dir: PathBuf,
    key: String,
    crash: Vec<CrashCitation>,
}

impl PretestXray {
    /// Os jars do pack, as configs da instância, o cache e o último travamento.
    pub(crate) fn new(state: &AppState, pack_id: PackId, items: &[InventoryItem], root: &Path) -> Self {
        let game_dir = InstanceDirs::for_pack(&state.paths, pack_id).game_dir;
        Self {
            jars: pack_jars(items, root, &game_dir),
            config_dir: game_dir.join("config"),
            cache_dir: state.paths.cache_dir(),
            key: pack_id.to_string(),
            crash: last_crash_citations(state, pack_id),
        }
    }

    /// Os avisos "alteram o mesmo ponto do jogo" (trabalho de disco: chamar fora do IPC).
    pub(crate) fn findings(self, loader: warden_jarmeta::Loader) -> Result<Vec<Finding>, AppError> {
        if self.jars.len() < 2 {
            return Ok(Vec::new());
        }
        let cache = XrayCache::in_cache_dir(&self.cache_dir);
        let xray = mixin_overlap::xray(
            &self.jars,
            loader,
            Some(&self.config_dir),
            Some(CacheSlot {
                cache: &cache,
                key: &self.key,
            }),
            self.crash,
        )
        .map_err(|error| AppError::from_domain(&error))?;
        tracing::debug!(stats = ?xray.scan.stats, overlaps = xray.overlaps.len(), "raio-x de mixins do pré-teste");
        mixin_overlap::findings(&xray.overlaps).map_err(|error| AppError::from_domain(&error))
    }
}

/// As configs de mixin citadas pela última sessão do Testar, se ela travou: o `output.log` da
/// sessão e os crash reports dela, com os trechos já sem dados pessoais. Os nomes dos arquivos
/// são relativos à sessão (`output.log`) ou à pasta do jogo (`crash-reports/…`).
pub(crate) fn last_crash_citations(state: &AppState, pack_id: PackId) -> Vec<CrashCitation> {
    let Ok(sessions) = crate::test_session::sessions_of(&state.paths, pack_id) else {
        return Vec::new();
    };
    let Some(last) = sessions.first().filter(|s| s.outcome == Outcome::Crashed) else {
        return Vec::new();
    };
    let Ok(session) = crate::test_session::session_dir(&state.paths, pack_id, &last.id) else {
        return Vec::new();
    };
    let game_dir = InstanceDirs::for_pack(&state.paths, pack_id).game_dir;
    let mut files = vec![("output.log".to_owned(), session.path().join("output.log"))];
    for report in &last.crash_reports {
        if let Ok(path) = warden_core::resolve_inside(&game_dir, report) {
            files.push((report.clone(), path));
        }
    }
    let player = state.settings.get().player_name;
    let redactor = crate::commands::report::redactor(&player, &state.secrets);
    let mut out: Vec<CrashCitation> = Vec::new();
    for (name, path) in files {
        let Some(text) = read_capped(&path) else {
            continue;
        };
        for mut citation in crash_citations(&text, &name) {
            if out.iter().any(|c| c.config == citation.config) {
                continue;
            }
            citation.excerpt = redactor.redact(&citation.excerpt).text;
            out.push(citation);
        }
    }
    out
}

fn read_capped(path: &Path) -> Option<String> {
    use std::io::Read as _;
    let file = std::fs::File::open(path).ok()?;
    let mut bytes = Vec::new();
    file.take(MAX_LOG_BYTES).read_to_end(&mut bytes).ok()?;
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

#[cfg(test)]
mod tests;
