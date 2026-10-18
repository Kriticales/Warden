//! As ferramentas de leitura (ARCHITECTURE §9.5, tabela "Leitura"), sobre o que já existe no
//! `main`: o inventário (P1-08), os jars (D-05/D-07), o relatório do pré-teste (D-03), as sessões
//! do Testar e o "Por que travou" (L-04, D-03, D-06), as configs (C-01), o histórico (V-01) e o
//! grafo (D-07).
//!
//! Cada item devolvido tem o id citável da tabela da ARCHITECTURE; logs, crash reports e configs
//! saem uma linha por item (`#L<n>`), para o selo de evidência levar à linha. Tudo passa pelo
//! [`Redactor`](warden_diagnostics::Redactor) do pack antes de sair.

use std::collections::HashSet;
use std::fmt::Write as _;
use std::path::Path;

use serde_json::{Value, json};
use warden_ai::tools::{KnownMod, KnownMods, ToolItem, ToolOutput};
use warden_instance::InstanceDirs;
use warden_jarmeta::JarMetadata;
use warden_project::configs::ConfigOrigin;
use warden_project::inventory::{InventoryItem, ItemKind, ItemState};
use warden_versioning::Snapshot;

use super::{LOCAL_FILE_SESSION, PackTools};
use crate::error::AppError;
use crate::test_session::TestSessionSummary;

/// Linhas por busca no log ou nas configs.
const SEARCH_LIMIT: usize = 40;
/// Linhas por leitura de log.
const READ_LIMIT: i64 = 300;
/// Linhas de um crash report ou de uma config.
const FILE_LIMIT: usize = 400;
/// Maior log lido (o fim dele; ARCHITECTURE §9.5: até 20 MB).
pub(crate) const MAX_LOG_BYTES: u64 = 20 * 1024 * 1024;

/// Um mod do pack como a IA o vê.
#[derive(Debug, Clone)]
pub(super) struct ModEntry {
    item: InventoryItem,
    /// O id das ferramentas: o nome do metafile sem `.pw.toml` (`create`).
    slug: String,
    jar: Option<JarMetadata>,
}

/// Os mods do pack, lidos uma vez por rodada.
#[derive(Debug, Clone, Default)]
pub(super) struct PackMods {
    entries: Vec<ModEntry>,
    error: Option<String>,
}

/// `mods/create.pw.toml` → `create`; `mods/foo-1.0.jar` → `foo-1.0`.
pub(crate) fn slug_of(path: &str) -> String {
    let name = path.rsplit('/').next().unwrap_or(path);
    let name = name.strip_suffix(".pw.toml").unwrap_or(name);
    match name.rsplit_once('.') {
        Some((stem, extension))
            if ["jar", "zip"].contains(&extension.to_ascii_lowercase().as_str()) =>
        {
            stem.to_owned()
        }
        _ => name.to_owned(),
    }
}

impl PackMods {
    pub(super) async fn load(tools: &PackTools<'_>) -> Self {
        let inventory =
            match warden_project::inventory::inventory(&tools.root, Some(&tools.state.modrinth))
                .await
            {
                Ok(inventory) => inventory,
                Err(error) => {
                    return Self {
                        entries: Vec::new(),
                        error: Some(error.to_string()),
                    };
                }
            };
        let game_dir = InstanceDirs::for_pack(&tools.state.paths, tools.pack_id).game_dir;
        let root = tools.root.clone();
        let items = inventory.items;
        tauri::async_runtime::spawn_blocking(move || {
            let paths: Vec<(usize, std::path::PathBuf)> = items
                .iter()
                .enumerate()
                .filter(|(_, item)| item.kind == ItemKind::Mod && item.state == ItemState::Ok)
                .filter_map(|(index, item)| {
                    let places = crate::commands::graph::jar_locations(item, &root, &game_dir);
                    crate::commands::graph::existing(&places)
                        .map(|(path, ..)| (index, path.clone()))
                })
                .collect();
            let mut jars: Vec<Option<JarMetadata>> = vec![None; items.len()];
            for (index, meta) in crate::commands::graph::read_jars(paths) {
                jars[index] = Some(meta);
            }
            Self {
                entries: items
                    .into_iter()
                    .zip(jars)
                    .map(|(item, jar)| ModEntry {
                        slug: slug_of(&item.path),
                        item,
                        jar,
                    })
                    .collect(),
                error: None,
            }
        })
        .await
        .unwrap_or_default()
    }

    pub(super) fn known(&self) -> KnownMods {
        let mut known = KnownMods::default();
        for entry in &self.entries {
            let item = KnownMod {
                id: entry.slug.clone(),
                name: entry.item.name.clone(),
                path: entry.item.path.clone(),
            };
            let jar_ids: Vec<&str> = entry
                .jar
                .iter()
                .flat_map(|jar| &jar.mods)
                .map(|module| module.id.as_str())
                .collect();
            let project = entry.item.project_id.as_deref();
            known.insert(&item, jar_ids.into_iter().chain(project));
        }
        known
    }

    /// O item do pack citado como `alias` (consultas e propostas da D-14).
    pub(super) fn find_item(&self, alias: &str) -> Option<&InventoryItem> {
        self.find(alias.strip_prefix("mod:").unwrap_or(alias))
            .map(|entry| &entry.item)
    }

    /// Todos os itens do pack.
    pub(super) fn items(&self) -> impl Iterator<Item = &InventoryItem> {
        self.entries.iter().map(|entry| &entry.item)
    }

    fn find(&self, alias: &str) -> Option<&ModEntry> {
        let alias = alias.trim().to_lowercase();
        self.entries.iter().find(|entry| {
            entry.slug.to_lowercase() == alias
                || entry.item.name.to_lowercase() == alias
                || entry.item.path.to_lowercase() == alias
                || entry
                    .jar
                    .iter()
                    .flat_map(|jar| &jar.mods)
                    .any(|module| module.id.to_lowercase() == alias)
        })
    }
}

fn text(value: impl serde::Serialize) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(ToOwned::to_owned))
        .unwrap_or_default()
}

fn mod_line(entry: &ModEntry) -> String {
    let item = &entry.item;
    let mut line = format!(
        "{} ({}) {}, tipo: {}, lado: {}, fonte: {}",
        entry.slug,
        item.name,
        item.version.as_deref().unwrap_or("versão desconhecida"),
        text(item.kind),
        text(item.side),
        text(item.source),
    );
    if item.state != ItemState::Ok {
        let _ = write!(line, ", situação: {}", text(item.state));
    }
    if item.optional {
        line.push_str(", opcional");
    }
    line
}

/// Executa uma ferramenta.
pub(super) async fn call(
    tools: &PackTools<'_>,
    name: &str,
    args: &Value,
) -> Result<ToolOutput, String> {
    let arg = |key: &str| {
        args.get(key)
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_owned()
    };
    let int = |key: &str| args.get(key).and_then(Value::as_i64);
    let output = match name {
        "get_pack_overview" => overview(tools).await,
        "list_mods" => list_mods(tools, &arg("filter")).await,
        "get_mod_details" => mod_details(tools, &arg("mod_id")).await,
        "get_findings" => findings(tools, &arg("severity")),
        "list_sessions" => list_sessions(tools),
        "get_session" => session(tools, &arg("session_id")).await,
        "get_crash_report" => crash_report(tools, &arg("session_id")),
        "search_log" => search_log(tools, &arg("session_id"), &arg("query")),
        "read_log" => read_log(
            tools,
            &arg("session_id"),
            int("from").unwrap_or(1),
            int("to").unwrap_or(1),
        ),
        "search_configs" => search_configs(tools, &arg("query")).await,
        "read_config" => read_config(tools, &arg("path")).await,
        "get_history" => history(tools, int("limit").unwrap_or(10)).await,
        "diff_versions" => diff(tools, &arg("from"), &arg("to")).await,
        "get_dependents" => graph(tools, &arg("mod_id"), true).await,
        "why_in_pack" => graph(tools, &arg("mod_id"), false).await,
        other => Err(format!("Ferramenta desconhecida: {other}")),
    };
    // Nada sai sem passar pela redação (inclusive as mensagens de erro, que podem ter caminhos).
    match output {
        Ok(mut output) => {
            for item in &mut output.items {
                item.text = tools.redactor.redact(&item.text).text;
            }
            Ok(output)
        }
        Err(message) => Err(tools.redactor.redact(&message).text),
    }
}

pub(super) fn failed(error: &AppError) -> String {
    format!(
        "Não foi possível ler ({}{}).",
        text(error.code),
        error
            .detail
            .as_deref()
            .map(|detail| format!(": {detail}"))
            .unwrap_or_default()
    )
}

fn plain(items: Vec<ToolItem>) -> ToolOutput {
    ToolOutput {
        items,
        ..ToolOutput::default()
    }
}

fn game(items: Vec<ToolItem>) -> ToolOutput {
    ToolOutput {
        items,
        game_data: true,
        ..ToolOutput::default()
    }
}

async fn overview(tools: &PackTools<'_>) -> Result<ToolOutput, String> {
    let root = tools.root.clone();
    let pack = tauri::async_runtime::spawn_blocking(move || warden_packwiz::read_pack(&root))
        .await
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())?;
    let manifest = &pack.pack.value;
    let mods = tools.pack_mods().await;
    let count = |kind: ItemKind| mods.entries.iter().filter(|e| e.item.kind == kind).count();
    let loaders: Vec<String> = manifest
        .loaders()
        .into_iter()
        .map(|(loader, version)| format!("{} {version}", loader.key()))
        .collect();
    let mut lines = vec![
        format!(
            "Pack: {}{}",
            manifest.name,
            if manifest.version.is_empty() {
                String::new()
            } else {
                format!(", versão {}", manifest.version)
            }
        ),
        format!(
            "Minecraft {}{}",
            manifest.minecraft_version().unwrap_or("?"),
            if loaders.is_empty() {
                ", sem loader".to_owned()
            } else {
                format!(", {}", loaders.join(", "))
            }
        ),
        format!(
            "Itens: {} mods, {} resource packs, {} shaders, {} outros",
            count(ItemKind::Mod),
            count(ItemKind::ResourcePack),
            count(ItemKind::Shader),
            count(ItemKind::Other),
        ),
    ];
    if let Some(error) = &mods.error {
        lines.push(format!("A lista de itens não pôde ser lida: {error}"));
    }
    let sessions = sessions(tools)?;
    match sessions.first() {
        Some(last) => {
            lines.push(format!(
                "Java {} e memória {} no último teste",
                last.java_major,
                last.memory_mb
                    .map_or_else(|| "automática".to_owned(), |mb| format!("{mb} MB"))
            ));
            lines.push(format!("Último teste: {}", session_line(last)));
        }
        None => lines.push("Nenhum teste feito neste computador.".to_owned()),
    }
    Ok(plain(vec![ToolItem::new(
        "pack:overview",
        lines.join("\n"),
    )]))
}

async fn list_mods(tools: &PackTools<'_>, filter: &str) -> Result<ToolOutput, String> {
    let mods = tools.pack_mods().await;
    if let Some(error) = &mods.error {
        return Err(format!("A lista de itens não pôde ser lida: {error}"));
    }
    let filter = filter.to_lowercase();
    Ok(plain(
        mods.entries
            .iter()
            .filter(|entry| {
                filter.is_empty()
                    || entry.slug.to_lowercase().contains(&filter)
                    || entry.item.name.to_lowercase().contains(&filter)
            })
            .map(|entry| ToolItem::new(format!("mod:{}", entry.slug), mod_line(entry)))
            .collect(),
    ))
}

async fn mod_details(tools: &PackTools<'_>, id: &str) -> Result<ToolOutput, String> {
    let mods = tools.pack_mods().await;
    let entry = mods
        .find(id)
        .ok_or_else(|| format!("O mod '{id}' não está no pack."))?;
    let mut lines = vec![
        mod_line(entry),
        format!(
            "Arquivo: {} ({})",
            entry.item.file_name.as_deref().unwrap_or("?"),
            entry.item.path
        ),
    ];
    match &entry.jar {
        Some(jar) => {
            for module in &jar.mods {
                let declared = json!({
                    "id": module.id,
                    "nome": module.name,
                    "versão": module.version,
                    "loader": module.loader,
                    "ambiente": module.environment,
                    "minecraft": module.minecraft,
                    "versãoDoLoader": module.loader_version,
                    "java": module.java,
                    "dependências": module.dependencies,
                    "mixins": module.mixins,
                });
                lines.push(format!("Declarado no jar: {declared}"));
            }
            if let Some(class) = &jar.class_version {
                lines.push(format!("Compilado para: {}", json!(class)));
            }
        }
        None => lines.push(
            "O jar não está neste computador (o Warden lê o jar depois do primeiro teste)."
                .to_owned(),
        ),
    }
    let mut items = vec![ToolItem::new(
        format!("jar:{}", entry.slug),
        lines.join("\n"),
    )];
    // Os achados do Warden que envolvem o mod.
    let jar_ids: HashSet<String> = entry
        .jar
        .iter()
        .flat_map(|jar| &jar.mods)
        .map(|module| module.id.clone())
        .collect();
    let report = load_findings(tools);
    for (index, finding) in report.iter().enumerate() {
        let involved = finding.finding.items().iter().any(|item| match item {
            warden_diagnostics::ItemRef::Metafile { path } => path == &entry.item.path,
            warden_diagnostics::ItemRef::ModId { id } => jar_ids.contains(id),
            warden_diagnostics::ItemRef::JarFile { .. } => false,
        });
        if involved {
            items.push(finding_item(index, finding));
        }
    }
    Ok(plain(items))
}

fn load_findings(
    tools: &PackTools<'_>,
) -> Vec<crate::commands::diagnostics::report::ReportFinding> {
    use crate::commands::diagnostics::report::{apply_ignored, load_report, read_ignored};
    let Some(mut report) = load_report(tools.state.paths.data_dir(), tools.pack_id) else {
        return Vec::new();
    };
    apply_ignored(&mut report.findings, &read_ignored(&tools.root));
    report.findings.retain(|finding| !finding.ignored);
    report.findings
}

fn finding_item(
    index: usize,
    finding: &crate::commands::diagnostics::report::ReportFinding,
) -> ToolItem {
    let rule = text(finding.finding.rule());
    ToolItem::new(
        format!("finding:{rule}#{}", index + 1),
        serde_json::to_string(&finding.finding).unwrap_or_default(),
    )
}

#[allow(clippy::unnecessary_wraps)] // mesma assinatura das outras ferramentas
fn findings(tools: &PackTools<'_>, severity: &str) -> Result<ToolOutput, String> {
    let all = load_findings(tools);
    if all.is_empty() {
        return Ok(plain(vec![ToolItem::new(
            "finding:nenhum",
            "Nenhum achado: o pack não foi verificado neste computador ou está sem problemas conhecidos.",
        )]));
    }
    Ok(plain(
        all.iter()
            .enumerate()
            .filter(|(_, finding)| {
                severity.is_empty() || text(finding.finding.severity()) == severity
            })
            .map(|(index, finding)| finding_item(index, finding))
            .collect(),
    ))
}

fn sessions(tools: &PackTools<'_>) -> Result<Vec<TestSessionSummary>, String> {
    crate::test_session::sessions_of(&tools.state.paths, tools.pack_id)
        .map_err(|error| failed(&error))
}

fn session_line(session: &TestSessionSummary) -> String {
    let started = warden_launcher::session::rfc3339_utc(
        std::time::UNIX_EPOCH + std::time::Duration::from_millis(session.started_at_ms),
    );
    let mut line = format!(
        "{started}, versão do pack {}, {} depois de {} s",
        session.pack_version.as_deref().unwrap_or("sem versão"),
        text(session.outcome),
        session.duration_ms / 1000,
    );
    if let Some(code) = session.exit_code {
        let _ = write!(line, ", código de saída {code}");
    }
    let _ = write!(
        line,
        ", Minecraft {}{}, Java {}",
        session.minecraft,
        session
            .loader
            .as_deref()
            .map(|loader| format!(
                ", {loader} {}",
                session.loader_version.as_deref().unwrap_or("")
            ))
            .unwrap_or_default(),
        session.java_major
    );
    if !session.crash_reports.is_empty() {
        let _ = write!(line, ", {} crash report(s)", session.crash_reports.len());
    }
    line
}

fn list_sessions(tools: &PackTools<'_>) -> Result<ToolOutput, String> {
    let all = sessions(tools)?;
    if all.is_empty() {
        return Ok(plain(vec![ToolItem::new(
            "session:nenhuma",
            "Nenhuma sessão de teste gravada.",
        )]));
    }
    Ok(plain(
        all.iter()
            .take(30)
            .map(|session| ToolItem::new(format!("session:{}", session.id), session_line(session)))
            .collect(),
    ))
}

fn find_session(tools: &PackTools<'_>, id: &str) -> Result<TestSessionSummary, String> {
    sessions(tools)?
        .into_iter()
        .find(|session| session.id == id)
        .ok_or_else(|| format!("A sessão '{id}' não existe (use list_sessions)."))
}

async fn session(tools: &PackTools<'_>, id: &str) -> Result<ToolOutput, String> {
    let summary = find_session(tools, id)?;
    let mut items = vec![ToolItem::new(
        format!("session:{id}"),
        session_line(&summary),
    )];
    if summary.outcome == warden_launcher::outcome::Outcome::Crashed {
        match crate::commands::diagnostics::crash::crash_of_session(tools.state, tools.pack_id, id)
            .await
        {
            Ok(diagnosis) => {
                items.push(ToolItem::new(
                    format!("session:{id}#analise"),
                    format!(
                        "Análise do Warden: {}",
                        if diagnosis.found_cause {
                            "causa encontrada"
                        } else {
                            "causa não encontrada"
                        }
                    ),
                ));
                for (index, finding) in diagnosis.findings.iter().enumerate() {
                    items.push(ToolItem::new(
                        format!("session:{id}#achado{}", index + 1),
                        serde_json::to_string(&finding.finding).unwrap_or_default(),
                    ));
                }
            }
            Err(error) => items.push(ToolItem::new(
                format!("session:{id}#analise"),
                failed(&error),
            )),
        }
    }
    Ok(game(items))
}

fn crash_report(tools: &PackTools<'_>, id: &str) -> Result<ToolOutput, String> {
    let summary = find_session(tools, id)?;
    let Some(relative) = summary.crash_reports.first() else {
        return Err(format!("A sessão '{id}' não deixou crash report."));
    };
    let game_dir = InstanceDirs::for_pack(&tools.state.paths, tools.pack_id).game_dir;
    let path = warden_core::resolve_inside(&game_dir, relative)
        .map_err(|_| "Caminho do crash report inválido.".to_owned())?;
    let bytes = std::fs::read(&path).map_err(|_| {
        format!("O crash report da sessão '{id}' não existe mais neste computador.")
    })?;
    let redacted = tools.redactor.redact(&String::from_utf8_lossy(&bytes)).text;
    let file = relative.rsplit('/').next().unwrap_or(relative);
    Ok(game(
        redacted
            .lines()
            .take(FILE_LIMIT)
            .enumerate()
            .map(|(index, line)| ToolItem::new(format!("crash:{id}/{file}#L{}", index + 1), line))
            .collect(),
    ))
}

/// Lê o fim de um arquivo de log (até [`MAX_LOG_BYTES`]) como texto.
pub(crate) fn read_log_text(path: &Path) -> std::io::Result<String> {
    use std::io::{Read as _, Seek as _, SeekFrom};
    let mut file = std::fs::File::open(path)?;
    let size = file.metadata()?.len();
    if size > MAX_LOG_BYTES {
        file.seek(SeekFrom::Start(size - MAX_LOG_BYTES))?;
    }
    let mut bytes = Vec::new();
    file.take(MAX_LOG_BYTES).read_to_end(&mut bytes)?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// As linhas do log da sessão (ou do arquivo escolhido), já redigidas.
fn log_lines(
    tools: &PackTools<'_>,
    id: &str,
) -> Result<(String, std::sync::Arc<Vec<String>>), String> {
    let file = if id == LOCAL_FILE_SESSION {
        "arquivo"
    } else {
        warden_launcher::session::OUTPUT_LOG
    };
    if let Some(lines) = tools.cached_log(id) {
        return Ok((file.to_owned(), lines));
    }
    let text = if id == LOCAL_FILE_SESSION {
        let path = tools
            .local_log
            .as_ref()
            .ok_or("Esta conversa não tem arquivo escolhido no computador.")?;
        // Já redigido ao entrar na conversa.
        std::fs::read_to_string(path).map_err(|_| "O arquivo da conversa sumiu.".to_owned())?
    } else {
        find_session(tools, id)?;
        let dir = crate::test_session::session_dir(&tools.state.paths, tools.pack_id, id)
            .map_err(|error| failed(&error))?;
        let raw = read_log_text(&dir.output_log())
            .map_err(|_| format!("Sem log guardado para a sessão '{id}'."))?;
        tools.redactor.redact(&raw).text
    };
    let lines = text.lines().map(ToOwned::to_owned).collect();
    Ok((file.to_owned(), tools.store_log(id, lines)))
}

fn search_log(tools: &PackTools<'_>, id: &str, query: &str) -> Result<ToolOutput, String> {
    if query.is_empty() {
        return Err("query vazia".to_owned());
    }
    let (file, lines) = log_lines(tools, id)?;
    let query = query.to_lowercase();
    let found: Vec<ToolItem> = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| line.to_lowercase().contains(&query))
        .take(SEARCH_LIMIT)
        .map(|(index, line)| ToolItem::new(format!("log:{id}/{file}#L{}", index + 1), line))
        .collect();
    if found.is_empty() {
        return Ok(game(vec![ToolItem::new(
            format!("log:{id}/{file}"),
            format!(
                "Nenhuma linha contém o texto ({} linhas no log).",
                lines.len()
            ),
        )]));
    }
    Ok(game(found))
}

fn read_log(tools: &PackTools<'_>, id: &str, from: i64, to: i64) -> Result<ToolOutput, String> {
    if from < 1 || to < from || to - from >= READ_LIMIT {
        return Err(format!(
            "Intervalo inválido: from >= 1, to >= from e no máximo {READ_LIMIT} linhas."
        ));
    }
    let (file, lines) = log_lines(tools, id)?;
    let start = usize::try_from(from - 1).unwrap_or(usize::MAX);
    let end = usize::try_from(to).unwrap_or(usize::MAX).min(lines.len());
    if start >= lines.len() {
        return Err(format!("O log tem só {} linhas.", lines.len()));
    }
    Ok(game(
        lines[start..end]
            .iter()
            .enumerate()
            .map(|(offset, line)| {
                ToolItem::new(format!("log:{id}/{file}#L{}", start + offset + 1), line)
            })
            .collect(),
    ))
}

/// As configs do pack, como texto (as binárias e as grandes demais ficam de fora).
async fn config_files(tools: &PackTools<'_>) -> Result<Vec<(String, String)>, String> {
    let root = tools.root.clone();
    let _lock = tools.state.locks.read(tools.pack_id).await;
    tauri::async_runtime::spawn_blocking(move || {
        let list = warden_project::configs::list_files(&root, ConfigOrigin::Pack)
            .map_err(|error| error.to_string())?;
        Ok(list
            .files
            .iter()
            .filter(|file| !file.binary)
            .filter_map(|file| {
                warden_project::configs::read_file(&root, ConfigOrigin::Pack, &file.path)
                    .ok()
                    .and_then(|content| content.text.map(|text| (content.path, text)))
            })
            .collect())
    })
    .await
    .map_err(|error| error.to_string())?
}

async fn search_configs(tools: &PackTools<'_>, query: &str) -> Result<ToolOutput, String> {
    if query.is_empty() {
        return Err("query vazia".to_owned());
    }
    let query = query.to_lowercase();
    let files = config_files(tools).await?;
    let found: Vec<ToolItem> = files
        .iter()
        .flat_map(|(path, text)| {
            text.lines()
                .enumerate()
                .map(move |(index, line)| (path, index, line))
        })
        .filter(|(path, _, line)| {
            line.to_lowercase().contains(&query) || path.to_lowercase().contains(&query)
        })
        .take(SEARCH_LIMIT)
        .map(|(path, index, line)| ToolItem::new(format!("config:{path}#L{}", index + 1), line))
        .collect();
    if found.is_empty() {
        return Ok(game(vec![ToolItem::new(
            "config:busca",
            format!("Nenhuma linha das {} configs contém o texto.", files.len()),
        )]));
    }
    Ok(game(found))
}

async fn read_config(tools: &PackTools<'_>, path: &str) -> Result<ToolOutput, String> {
    let root = tools.root.clone();
    let relative = path.to_owned();
    let _lock = tools.state.locks.read(tools.pack_id).await;
    let content = tauri::async_runtime::spawn_blocking(move || {
        warden_project::configs::read_file(&root, ConfigOrigin::Pack, &relative)
    })
    .await
    .map_err(|error| error.to_string())?
    .map_err(|_| format!("A config '{path}' não existe ou não pode ser lida."))?;
    let text = content
        .text
        .ok_or_else(|| format!("A config '{path}' não é texto ou é grande demais."))?;
    Ok(game(
        text.lines()
            .take(FILE_LIMIT)
            .enumerate()
            .map(|(index, line)| {
                ToolItem::new(format!("config:{}#L{}", content.path, index + 1), line)
            })
            .collect(),
    ))
}

async fn history(tools: &PackTools<'_>, limit: i64) -> Result<ToolOutput, String> {
    let root = tools.root.clone();
    let limit = usize::try_from(limit.clamp(1, 20)).unwrap_or(10);
    let versions = tauri::async_runtime::spawn_blocking(move || {
        warden_versioning::PackRepo::open(&root).and_then(|repo| repo.versions())
    })
    .await
    .map_err(|error| error.to_string())?
    .map_err(|_| "O pack ainda não tem histórico de versões.".to_owned())?;
    // Da maior para a menor; as etiquetas de outras linhas de um pack importado não contam.
    let versions: Vec<_> = versions.into_iter().filter(|v| v.reachable).collect();
    if versions.is_empty() {
        return Ok(plain(vec![ToolItem::new(
            "version:nenhuma",
            "Nenhuma versão salva.",
        )]));
    }
    Ok(plain(
        versions
            .into_iter()
            .take(limit)
            .map(|version| {
                let summary: String = version
                    .message
                    .lines()
                    .take(12)
                    .collect::<Vec<_>>()
                    .join("\n");
                ToolItem::new(
                    format!("version:{}", version.version),
                    format!("{} ({}): {}", version.version, version.date, summary),
                )
            })
            .collect(),
    ))
}

async fn diff(tools: &PackTools<'_>, from: &str, to: &str) -> Result<ToolOutput, String> {
    let from = from.trim_start_matches('v');
    let to = to.trim_start_matches('v');
    if from.is_empty() || to.is_empty() {
        return Err("Informe from e to.".to_owned());
    }
    let current = matches!(to, "atual" | "agora" | "current");
    let changes = crate::commands::versioning::changes_between(
        Some(&tools.state.modrinth),
        Some((&tools.state.locks, tools.pack_id)),
        &tools.root,
        Snapshot::Version(from.to_owned()),
        if current {
            Snapshot::WorkingTree
        } else {
            Snapshot::Version(to.to_owned())
        },
    )
    .await
    .map_err(|_| format!("Não há como comparar '{from}' e '{to}' (use get_history)."))?;
    let summary = json!({
        "minecraft": changes.minecraft,
        "loaders": changes.loaders,
        "itens": changes.items,
        "configs": changes.configs,
    });
    Ok(plain(vec![ToolItem::new(
        format!("version:{from}..{}", if current { "atual" } else { to }),
        summary.to_string(),
    )]))
}

async fn graph(tools: &PackTools<'_>, id: &str, dependents: bool) -> Result<ToolOutput, String> {
    let mods = tools.pack_mods().await;
    let entry = mods
        .find(id)
        .ok_or_else(|| format!("O mod '{id}' não está no pack."))?;
    let path = entry.item.path.clone();
    let text = if dependents {
        crate::commands::graph::dependents_impl(tools.state, tools.pack_id, &[path])
            .await
            .map(|report| serde_json::to_string(&report).unwrap_or_default())
    } else {
        crate::commands::graph::why_impl(tools.state, tools.pack_id, &path)
            .await
            .map(|report| serde_json::to_string(&report).unwrap_or_default())
    }
    .map_err(|error| failed(&error))?;
    Ok(plain(vec![ToolItem::new(
        format!("graph:{}", entry.slug),
        text,
    )]))
}
