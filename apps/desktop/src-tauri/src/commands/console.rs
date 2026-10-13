//! Comandos do console agrupado (SPEC T13 "Mostrar: Agrupado por mod · Só problemas";
//! ARCHITECTURE §9.8; ROADMAP D-09).
//!
//! - `console_grouped`: os grupos (um por mod, "Jogo e loader" e "Desconhecido") com os itens
//!   dos grupos abertos, ou a lista "Só problemas", de uma sessão do Testar (a aberta ou uma
//!   gravada).
//! - `console_grouped_lines`: as linhas originais de um item ou de um grupo ("Copiar" e o
//!   stack trace aberto).
//!
//! O agrupamento é da `warden-diagnostics::console`; aqui ele é alimentado pelo `output.log` da
//! sessão, lido aos poucos a partir de onde parou (a sessão aberta cresce enquanto o jogo
//! escreve), com o mesmo leitor do console linha a linha. O índice de pacotes da D-05 (com o
//! cache por hash dos jars) dá os donos; o agrupador de cada sessão fica em memória enquanto
//! é usado (as últimas [`KEEP`] sessões), e a interface pede de novo a cada lote do jogo
//! aberto. O log gravado não muda.
#![allow(clippy::needless_pass_by_value)] // argumentos pertencem ao contrato do Tauri

use std::collections::HashMap;
use std::fs::File;
use std::io::{Read as _, Seek as _, SeekFrom};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};

use serde::{Deserialize, Serialize};
use tauri::State;
use warden_core::PackId;
use warden_diagnostics::console::{
    ConsoleGrouper, GroupKind, GroupSummary, ItemView, LineLevel, LinesTarget, LogEntry,
    ModDirectory, OriginalLine,
};
use warden_diagnostics::pretest::build_index;
use warden_instance::InstanceDirs;
use warden_launcher::decode::{decode_line, trim_line_end};
use warden_launcher::log::{LogLevel, LogLine, LogParser, LogSource};

use crate::commands::diagnostics::input::{index_input, read_snapshot};
use crate::error::AppError;
use crate::state::AppState;
use crate::test_session::{ConsoleLevel, ConsoleLine, ConsoleOrigin, session_dir};

/// Sessões agrupadas guardadas em memória.
const KEEP: usize = 3;
/// Bytes lidos do `output.log` por vez.
const CHUNK: usize = 4 * 1024 * 1024;
/// Itens de um grupo aberto, no máximo, por pedido.
const MAX_LIMIT: u32 = 100_000;

/// O que mostrar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) enum GroupedMode {
    /// Agrupado por mod.
    ByMod,
    /// Só problemas.
    Problems,
}

/// Um grupo aberto, com quantos itens mostrar.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OpenGroup {
    /// Chave do grupo.
    pub(crate) key: String,
    /// Itens a mostrar ("Mostrar mais" aumenta).
    pub(crate) limit: u32,
}

/// O pedido da tela.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GroupedRequest {
    /// Agrupado por mod ou só problemas.
    pub(crate) mode: GroupedMode,
    /// Grupos abertos (agrupado por mod).
    pub(crate) open: Vec<OpenGroup>,
    /// Itens de "Só problemas" a mostrar.
    pub(crate) limit: u32,
    /// Busca (sem diferença de maiúsculas).
    pub(crate) query: String,
}

/// Um grupo na tela.
#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GroupRow {
    /// O grupo.
    pub(crate) group: GroupSummary,
    /// Nome do item do pack (ou o id do mod); `None` em "Jogo e loader" e "Desconhecido".
    pub(crate) name: Option<String>,
    /// Itens (só nos grupos abertos), até o limite pedido.
    pub(crate) items: Vec<ItemView>,
}

/// A tela do console agrupado.
#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GroupedView {
    /// Linhas lidas do log.
    pub(crate) lines: u32,
    /// Grupos (agrupado por mod; vazio em "Só problemas").
    pub(crate) groups: Vec<GroupRow>,
    /// Itens de "Só problemas", até o limite (vazio no agrupado por mod).
    pub(crate) problems: Vec<ItemView>,
    /// Quantos itens de "Só problemas" existem (com a busca).
    pub(crate) problems_total: u32,
    /// Nome do item de cada grupo que aparece em "Só problemas" (chave → nome).
    pub(crate) names: HashMap<String, String>,
}

/// De onde copiar as linhas originais: um item ou um grupo.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase", tag = "type")]
pub(crate) enum LinesRequest {
    /// Um item.
    Item {
        /// Id do item.
        id: u32,
    },
    /// Um grupo.
    Group {
        /// Chave do grupo.
        key: String,
    },
}

/// Uma sessão agrupada: o leitor do `output.log` e o agrupador.
struct Grouped {
    log: PathBuf,
    offset: u64,
    parser: LogParser,
    next_seq: u32,
    grouper: ConsoleGrouper,
    names: HashMap<String, String>,
}

impl Grouped {
    /// Lê o que o jogo escreveu desde a última leitura (só linhas completas).
    fn catch_up(&mut self) -> std::io::Result<()> {
        let mut file = match File::open(&self.log) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error),
        };
        if file.metadata()?.len() <= self.offset {
            return Ok(());
        }
        file.seek(SeekFrom::Start(self.offset))?;
        let mut buffer = Vec::with_capacity(CHUNK);
        loop {
            let read = (&mut file)
                .take(u64::try_from(CHUNK).unwrap_or(u64::MAX))
                .read_to_end(&mut buffer)?;
            let Some(end) = buffer.iter().rposition(|&byte| byte == b'\n') else {
                // Uma linha maior que o pedaço: espera o resto.
                if read == 0 || buffer.len() > 8 * CHUNK {
                    return Ok(());
                }
                continue;
            };
            let complete = buffer.drain(..=end).collect::<Vec<u8>>();
            self.offset += u64::try_from(complete.len()).unwrap_or(0);
            self.feed(&complete);
            if read == 0 {
                return Ok(());
            }
        }
    }

    fn feed(&mut self, bytes: &[u8]) {
        let mut lines: Vec<LogLine> = Vec::new();
        // Só linhas completas chegam aqui: o último `\n` fecha a última linha.
        let body = bytes.strip_suffix(b"\n").unwrap_or(bytes);
        for raw in body.split(|&byte| byte == b'\n') {
            let text = decode_line(trim_line_end(raw));
            lines.extend(self.parser.push(&text, 0));
        }
        let first = self.next_seq;
        let entries: Vec<LogEntry<'_>> = lines
            .iter()
            .enumerate()
            .map(|(index, line)| LogEntry {
                seq: first.saturating_add(u32::try_from(index).unwrap_or(u32::MAX)),
                level: line.level.map(line_level),
                logger: line.logger.as_deref(),
                time: line.time.as_deref(),
                timestamp_ms: line.timestamp_ms,
                text: &line.text,
            })
            .collect();
        self.next_seq = first.saturating_add(u32::try_from(entries.len()).unwrap_or(u32::MAX));
        self.grouper.push_batch(&entries);
    }

    fn name(&self, group: &GroupSummary) -> Option<String> {
        match group.kind {
            GroupKind::Item => group
                .item
                .as_ref()
                .and_then(|item| self.names.get(item).cloned())
                .or_else(|| group.mod_id.clone()),
            GroupKind::Mod => group.mod_id.clone(),
            GroupKind::Game | GroupKind::Unknown => None,
        }
    }

    fn view(&self, request: &GroupedRequest) -> GroupedView {
        let query = request.query.as_str();
        let mut view = GroupedView {
            lines: self.grouper.lines(),
            groups: Vec::new(),
            problems: Vec::new(),
            problems_total: 0,
            names: HashMap::new(),
        };
        match request.mode {
            GroupedMode::ByMod => {
                view.groups = self
                    .grouper
                    .groups(query)
                    .into_iter()
                    .map(|group| {
                        let items = request
                            .open
                            .iter()
                            .find(|open| open.key == group.key)
                            .map(|open| {
                                let limit = open.limit.min(MAX_LIMIT) as usize;
                                self.grouper.group_items(&group.key, query, limit).0
                            })
                            .unwrap_or_default();
                        GroupRow {
                            name: self.name(&group),
                            group,
                            items,
                        }
                    })
                    .collect();
            }
            GroupedMode::Problems => {
                let limit = request.limit.min(MAX_LIMIT) as usize;
                let (items, total) = self.grouper.problems(query, limit);
                for group in self.grouper.groups("") {
                    if items.iter().any(|item| item.group == group.key)
                        && let Some(name) = self.name(&group)
                    {
                        view.names.insert(group.key.clone(), name);
                    }
                }
                view.problems = items;
                view.problems_total = total;
            }
        }
        view
    }
}

const fn line_level(level: LogLevel) -> LineLevel {
    match level {
        LogLevel::Trace => LineLevel::Trace,
        LogLevel::Debug => LineLevel::Debug,
        LogLevel::Info => LineLevel::Info,
        LogLevel::Warn => LineLevel::Warn,
        LogLevel::Error => LineLevel::Error,
        LogLevel::Fatal => LineLevel::Fatal,
    }
}

const fn console_level(level: LineLevel) -> ConsoleLevel {
    match level {
        LineLevel::Trace => ConsoleLevel::Trace,
        LineLevel::Debug => ConsoleLevel::Debug,
        LineLevel::Info => ConsoleLevel::Info,
        LineLevel::Warn => ConsoleLevel::Warn,
        LineLevel::Error => ConsoleLevel::Error,
        LineLevel::Fatal => ConsoleLevel::Fatal,
    }
}

/// A linha original como linha do console (a interface copia com o mesmo formato).
fn console_line(line: OriginalLine) -> ConsoleLine {
    ConsoleLine {
        seq: u64::from(line.seq),
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // milissegundos inteiros
        at_ms: line.stamp.at_ms.map(|ms| ms.max(0.0) as u64),
        time: line.stamp.time,
        level: line.level.map(console_level),
        logger: line.logger,
        thread: None,
        text: line.text,
        params: std::collections::BTreeMap::new(),
        origin: ConsoleOrigin::Game,
        stream: None,
    }
}

type Shared = Arc<Mutex<Grouped>>;

/// As sessões agrupadas em memória (as mais recentes no fim).
#[derive(Default)]
pub(crate) struct ConsoleGroups {
    sessions: Mutex<Vec<((PackId, String), Shared)>>,
}

impl std::fmt::Debug for ConsoleGroups {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let count = self
            .sessions
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .len();
        f.debug_struct("ConsoleGroups")
            .field("sessions", &count)
            .finish()
    }
}

impl ConsoleGroups {
    fn get(&self, key: &(PackId, String)) -> Option<Shared> {
        let mut sessions = self.sessions.lock().unwrap_or_else(PoisonError::into_inner);
        let position = sessions.iter().position(|(k, _)| k == key)?;
        let entry = sessions.remove(position);
        let shared = Arc::clone(&entry.1);
        sessions.push(entry);
        Some(shared)
    }

    fn insert(&self, key: (PackId, String), shared: Shared) -> Shared {
        let mut sessions = self.sessions.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some((_, existing)) = sessions.iter().find(|(k, _)| *k == key) {
            return Arc::clone(existing);
        }
        sessions.push((key, Arc::clone(&shared)));
        while sessions.len() > KEEP {
            sessions.remove(0);
        }
        shared
    }
}

/// O agrupador da sessão: o guardado, ou um novo com o índice dos jars do pack.
async fn grouped(
    state: &AppState,
    pack_id: PackId,
    session_id: &str,
) -> Result<Shared, AppError> {
    let dir = session_dir(&state.paths, pack_id, session_id)?;
    let key = (pack_id, session_id.to_owned());
    if let Some(shared) = state.console_groups.get(&key) {
        return Ok(shared);
    }
    let snapshot = read_snapshot(state, pack_id, None).await?;
    let names: HashMap<String, String> = snapshot
        .items
        .iter()
        .map(|item| (item.path.clone(), item.name.clone()))
        .collect();
    let input = index_input(&snapshot);
    let game_dir = InstanceDirs::for_pack(&state.paths, pack_id).game_dir;
    let cache_dir = state.paths.cache_dir();
    let log = dir.output_log();
    let shared = blocking(move || {
        let directory = match input {
            Some(input) if game_dir.is_dir() => {
                let files: Vec<(String, String)> = input
                    .items
                    .iter()
                    .map(|item| (item.path.clone(), item.filename.clone()))
                    .collect();
                let pass = build_index(&input, &game_dir, &cache_dir);
                ModDirectory::new(pass.index, files)
            }
            _ => ModDirectory::empty(),
        };
        Ok(Arc::new(Mutex::new(Grouped {
            log,
            offset: 0,
            parser: LogParser::new(LogSource::Stdout),
            next_seq: 0,
            grouper: ConsoleGrouper::new(directory),
            names,
        })))
    })
    .await?;
    Ok(state.console_groups.insert(key, shared))
}

/// O console agrupado de uma sessão (a do jogo aberto ou uma gravada), lido até o fim do
/// `output.log` agora. Erros: `app.TEST_SESSION_NOT_FOUND`.
#[tauri::command]
#[specta::specta]
pub(crate) async fn console_grouped(
    state: State<'_, AppState>,
    pack_id: PackId,
    session_id: String,
    request: GroupedRequest,
) -> Result<GroupedView, AppError> {
    grouped_view(&state, pack_id, &session_id, request).await
}

async fn grouped_view(
    state: &AppState,
    pack_id: PackId,
    session_id: &str,
    request: GroupedRequest,
) -> Result<GroupedView, AppError> {
    let shared = grouped(state, pack_id, session_id).await?;
    blocking(move || {
        let mut grouped = shared.lock().unwrap_or_else(PoisonError::into_inner);
        grouped
            .catch_up()
            .map_err(|error| AppError::internal(format!("ler o output.log: {error}")))?;
        Ok(grouped.view(&request))
    })
    .await
}

/// As linhas originais de um item (todas as vezes que apareceu) ou de um grupo, na ordem do
/// log, até `limit`. Erros: `app.TEST_SESSION_NOT_FOUND`.
#[tauri::command]
#[specta::specta]
pub(crate) async fn console_grouped_lines(
    state: State<'_, AppState>,
    pack_id: PackId,
    session_id: String,
    target: LinesRequest,
    limit: Option<u32>,
) -> Result<Vec<ConsoleLine>, AppError> {
    let shared = grouped(&state, pack_id, &session_id).await?;
    blocking(move || {
        let mut grouped = shared.lock().unwrap_or_else(PoisonError::into_inner);
        grouped
            .catch_up()
            .map_err(|error| AppError::internal(format!("ler o output.log: {error}")))?;
        let target = match target {
            LinesRequest::Item { id } => LinesTarget::Item(id),
            LinesRequest::Group { key } => LinesTarget::Group(key),
        };
        Ok(grouped
            .grouper
            .original_lines(&target, limit.map(|limit| limit as usize))
            .into_iter()
            .map(console_line)
            .collect())
    })
    .await
}

async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, AppError> + Send + 'static,
) -> Result<T, AppError> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|error| AppError::internal(format!("console agrupado interrompido: {error}")))?
}

#[cfg(test)]
mod tests {
    use std::io::Write as _;

    use super::*;

    fn session(log: PathBuf) -> Grouped {
        Grouped {
            log,
            offset: 0,
            parser: LogParser::new(LogSource::Stdout),
            next_seq: 0,
            grouper: ConsoleGrouper::new(ModDirectory::empty()),
            names: HashMap::new(),
        }
    }

    fn by_mod(open: &[&str]) -> GroupedRequest {
        GroupedRequest {
            mode: GroupedMode::ByMod,
            open: open
                .iter()
                .map(|key| OpenGroup {
                    key: (*key).to_owned(),
                    limit: 50,
                })
                .collect(),
            limit: 50,
            query: String::new(),
        }
    }

    /// O `output.log` crescendo (jogo aberto): cada leitura pega só o que é novo e linhas
    /// incompletas esperam o fim; o stack trace que chega depois junta-se ao erro.
    #[test]
    fn le_o_log_aos_poucos() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("output.log");
        let mut file = File::create(&path).unwrap();
        let mut grouped = session(path.clone());
        grouped.catch_up().unwrap();
        assert_eq!(grouped.grouper.lines(), 0);

        write!(file, "pai vivo\npai vivo\n[12:00:01] [main/ERROR] [x/]: Falh").unwrap();
        file.flush().unwrap();
        grouped.catch_up().unwrap();
        assert_eq!(grouped.grouper.lines(), 2);
        let view = grouped.view(&by_mod(&["unknown"]));
        assert_eq!(view.groups.len(), 1);
        assert_eq!(view.groups[0].items[0].count, 2);
        assert_eq!(view.groups[0].items[0].text, "pai vivo");

        write!(file, "ou\njava.lang.RuntimeException: x\n\tat a.B.c(B.java:1)\n").unwrap();
        file.flush().unwrap();
        grouped.catch_up().unwrap();
        assert_eq!(grouped.grouper.lines(), 5);
        let view = grouped.view(&GroupedRequest {
            mode: GroupedMode::Problems,
            open: Vec::new(),
            limit: 10,
            query: String::new(),
        });
        assert_eq!(view.problems_total, 1);
        assert_eq!(view.problems[0].text, "Falhou");
        assert_eq!(view.problems[0].extra_lines, 2);
        assert!(view.groups.is_empty());

        let lines = grouped
            .grouper
            .original_lines(&LinesTarget::Item(view.problems[0].id), None);
        let line = console_line(lines[0].clone());
        assert_eq!(line.time.as_deref(), Some("12:00:01"));
        assert_eq!(line.level, Some(ConsoleLevel::Error));
        assert_eq!(line.text, "Falhou\njava.lang.RuntimeException: x\n\tat a.B.c(B.java:1)");
    }

    /// Esqueleto de um jar real da D-05, remontado (descritores, uma classe por pacote).
    fn real_jar(name: &str) -> Vec<u8> {
        use zip::write::{SimpleFileOptions, ZipWriter};
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../crates/warden-jarmeta/tests/indice/esqueletos.json");
        let all: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let skeleton = &all[name];
        let mut zip = ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let options = SimpleFileOptions::default();
        for (file, content) in skeleton["arquivos"].as_object().unwrap() {
            zip.start_file(file.as_str(), options).unwrap();
            let bytes: Vec<u8> = match content {
                serde_json::Value::String(text) => text.as_bytes().to_vec(),
                other => serde_json::from_value(other.clone()).unwrap(),
            };
            zip.write_all(&bytes).unwrap();
        }
        for package in skeleton["pacotes"].as_array().unwrap() {
            let class = format!("{}/X.class", package.as_str().unwrap().replace('.', "/"));
            zip.start_file(class, options).unwrap();
            zip.write_all(&[0xCA, 0xFE, 0xBA, 0xBE, 0, 0, 0, 65]).unwrap();
        }
        zip.finish().unwrap().into_inner()
    }

    /// O comando de ponta a ponta: o índice dos jars da instância (D-05) dá o item do pack ao
    /// logger com a classe do Sodium, e o nome do item vem do inventário.
    #[tokio::test]
    async fn agrupa_pelo_indice_dos_jars_da_instancia() {
        let dir = tempfile::tempdir().unwrap();
        let state = crate::state::tests::test_state(dir.path());
        let root = dir.path().join("pack");
        std::fs::create_dir_all(root.join("mods")).unwrap();
        std::fs::write(
            root.join("pack.toml"),
            "name = \"P\"\npack-format = \"packwiz:1.1.0\"\n\n[index]\nfile = \"index.toml\"\nhash-format = \"sha256\"\nhash = \"0\"\n\n[versions]\nminecraft = \"1.20.1\"\nfabric = \"0.16.10\"\n",
        )
        .unwrap();
        std::fs::write(
            root.join("index.toml"),
            "hash-format = \"sha256\"\n\n[[files]]\nfile = \"mods/sodium.pw.toml\"\nhash = \"1\"\nmetafile = true\n",
        )
        .unwrap();
        std::fs::write(
            root.join("mods/sodium.pw.toml"),
            "name = \"Sodium\"\nfilename = \"sodium-fabric.jar\"\nside = \"client\"\n\n[download]\nurl = \"https://x/sodium.jar\"\nhash-format = \"sha256\"\nhash = \"00\"\n",
        )
        .unwrap();
        let record = warden_project::registry::PackRecord {
            id: PackId::new(),
            name: "P".into(),
            path: root,
            last_test: None,
            extra: std::collections::BTreeMap::new(),
        };
        let pack = record.id;
        state.packs.insert(record).unwrap();
        let dirs = InstanceDirs::for_pack(&state.paths, pack);
        std::fs::create_dir_all(dirs.game_dir.join("mods")).unwrap();
        std::fs::write(
            dirs.game_dir.join("mods/sodium-fabric.jar"),
            real_jar("fabric-1.20.1-sodium"),
        )
        .unwrap();
        let session = dirs.state_dir.join("sessions/2026-10-13_12-00-00");
        std::fs::create_dir_all(&session).unwrap();
        std::fs::write(
            session.join("output.log"),
            "[12:00:01] [Render thread/WARN] [me.jellysquid.mods.sodium.client.SodiumClientMod/]: Driver 1\n\
             [12:00:02] [Render thread/WARN] [me.jellysquid.mods.sodium.client.SodiumClientMod/]: Driver 2\n\
             [12:00:03] [Render thread/INFO] [minecraft/Minecraft]: Pronto\n",
        )
        .unwrap();
        let key = "item:mods/sodium.pw.toml";
        let mut request = by_mod(&[key]);
        let view = grouped_view(&state, pack, "2026-10-13_12-00-00", request.clone())
            .await
            .unwrap();
        assert_eq!(view.lines, 3);
        let sodium = &view.groups[0];
        assert_eq!(sodium.group.key, key);
        assert_eq!(sodium.name.as_deref(), Some("Sodium"));
        assert_eq!(sodium.group.warnings, 2);
        assert_eq!(sodium.items[0].count, 2);
        assert_eq!(sodium.items[0].text, "Driver {n}");
        assert!(view.groups[1].items.is_empty(), "grupo fechado não traz itens");

        request.mode = GroupedMode::Problems;
        let view = grouped_view(&state, pack, "2026-10-13_12-00-00", request)
            .await
            .unwrap();
        assert_eq!(view.problems_total, 1);
        assert_eq!(view.names.get(key).map(String::as_str), Some("Sodium"));

        let error = grouped_view(&state, pack, "../fora", by_mod(&[]))
            .await
            .unwrap_err();
        assert_eq!(
            error.code,
            crate::error::ErrorCode::App(crate::error::AppErrorCode::TestSessionNotFound)
        );
    }

    #[test]
    fn guarda_so_as_ultimas_sessoes() {
        let groups = ConsoleGroups::default();
        let dir = tempfile::tempdir().unwrap();
        let pack = PackId::new();
        let shared = |name: &str| Arc::new(Mutex::new(session(dir.path().join(name))));
        let first = groups.insert((pack, "1".into()), shared("1"));
        for name in ["2", "3", "4"] {
            groups.insert((pack, name.into()), shared(name));
        }
        assert!(groups.get(&(pack, "1".into())).is_none());
        assert!(groups.get(&(pack, "4".into())).is_some());
        // Inserir de novo a mesma sessão devolve a que já estava.
        let again = groups.insert((pack, "4".into()), shared("4"));
        assert!(Arc::ptr_eq(&again, &groups.get(&(pack, "4".into())).unwrap()));
        drop(first);
    }
}
