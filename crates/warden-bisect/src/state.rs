//! O estado guardado da busca (`instances/<pack>/state/bisect.json`; ARCHITECTURE §9.7 item
//! 10) e os resultados terminados (`state/bisect-results/<id>.json`), que Problemas →
//! Travamentos liga ao travamento de origem.
//!
//! Gravado depois de **cada** rodada (num temporário ao lado, trocado de uma vez): pausar e
//! reabrir o app continua da rodada seguinte.

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{BisectError, Result};
use crate::judge::{Symptom, Target};
use crate::search::Search;
use crate::watch::{Anchor, Goal};

/// Versão do formato.
pub const SCHEMA: u32 = 1;
/// Nome do arquivo do estado atual.
pub const STATE_FILE: &str = "bisect.json";
/// Pasta dos resultados terminados.
pub const RESULTS_DIR: &str = "bisect-results";

/// O que cada rodada mediu (além do veredito, que fica na [`Search`]).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RoundInfo {
    /// Número da rodada.
    pub number: u32,
    /// Duração da rodada inteira (preparar, abrir e encerrar).
    #[specta(type = specta_typescript::Number)]
    pub duration_ms: u64,
    /// Até entrar no mundo (ou chegar ao menu), quando chegou.
    #[specta(type = Option<specta_typescript::Number>)]
    pub reached_ms: Option<u64>,
    /// O sinal da falha, quando falhou.
    pub anchor: Option<Anchor>,
    /// A assinatura da falha, quando falhou.
    pub signature: Option<String>,
    /// O log da rodada, relativo à pasta do estado (`bisect-logs/<id>/round-6.log`).
    pub log: Option<String>,
}

/// O estado inteiro de uma busca.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BisectState {
    /// Versão do formato.
    pub schema: u32,
    /// Identificador da busca (`20261018-153012`).
    pub id: String,
    /// O que o usuário descreveu.
    pub symptom: Symptom,
    /// Entrar no mundo ou chegar ao menu.
    pub goal: Goal,
    /// A assinatura do problema (vazia até a rodada 0 reproduzir).
    pub target: Target,
    /// Hash da assinatura (D-06) do travamento de onde a busca começou, se veio de um.
    pub origin_crash: Option<String>,
    /// Versão do Minecraft e loader (para o veredito e os argumentos).
    pub minecraft: String,
    /// Loader do `pack.toml` (`neoforge`, `fabric`…; `None` = vanilla).
    pub loader: Option<String>,
    /// O mundo de teste (pasta em `saves/` da instância de teste), copiado a cada rodada.
    pub world: Option<String>,
    /// O algoritmo.
    pub search: Search,
    /// O que cada rodada mediu.
    pub rounds: Vec<RoundInfo>,
    /// W: tempo até "entrou no mundo" medido na rodada 0 (base da estimativa).
    pub world_ms: Option<u64>,
    /// Intervalo entre `Starting integrated…` e `logged in` na rodada de controle.
    pub control_interval_ms: Option<u64>,
    /// Começo (milissegundos desde 1970).
    pub started_at_ms: u64,
    /// Tempo gasto em rodadas (sem as pausas).
    pub elapsed_ms: u64,
    /// Pausada pelo usuário (ou pelo fechamento do app) entre rodadas.
    pub paused: bool,
    /// Fim (milissegundos desde 1970), quando terminou.
    pub finished_at_ms: Option<u64>,
}

impl BisectState {
    /// Arquivo do estado atual na pasta `state/` da instância.
    #[must_use]
    pub fn path_in(state_dir: &Path) -> PathBuf {
        state_dir.join(STATE_FILE)
    }

    /// Lê o estado atual. `Ok(None)` se não há busca guardada.
    pub fn load(state_dir: &Path) -> Result<Option<Self>> {
        read(&Self::path_in(state_dir))
    }

    /// Grava o estado atual e, se terminou, o resultado em `bisect-results/`.
    pub fn save(&self, state_dir: &Path) -> Result<()> {
        write(&Self::path_in(state_dir), self)?;
        if self.search.outcome().is_some() {
            write(&result_path(state_dir, &self.id), self)?;
        }
        Ok(())
    }
}

/// Arquivo de um resultado terminado.
#[must_use]
pub fn result_path(state_dir: &Path, id: &str) -> PathBuf {
    state_dir.join(RESULTS_DIR).join(format!("{id}.json"))
}

/// Os resultados terminados, do mais novo para o mais antigo (ilegíveis ficam de fora).
pub fn results(state_dir: &Path) -> Result<Vec<BisectState>> {
    let dir = state_dir.join(RESULTS_DIR);
    let entries = match fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(BisectError::io(&dir, error)),
    };
    let mut found: Vec<BisectState> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .filter_map(|path| read(&path).ok().flatten())
        .collect();
    found.sort_by(|a, b| b.started_at_ms.cmp(&a.started_at_ms).then(b.id.cmp(&a.id)));
    Ok(found)
}

fn read(path: &Path) -> Result<Option<BisectState>> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(BisectError::io(path, error)),
    };
    serde_json::from_str(&text)
        .map(Some)
        .map_err(|error| BisectError::StateUnreadable {
            path: path.to_path_buf(),
            reason: error.to_string(),
        })
}

fn write(path: &Path, state: &BisectState) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| BisectError::io(parent, error))?;
    }
    let text = serde_json::to_string_pretty(state)
        .map_err(|error| BisectError::Internal(error.to_string()))?;
    let temp = path.with_extension("json.tmp");
    fs::write(&temp, text).map_err(|error| BisectError::io(&temp, error))?;
    fs::rename(&temp, path).map_err(|error| BisectError::io(path, error))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::Verdict;
    use crate::universe::{ItemSpec, Universe};

    fn state(id: &str, started: u64) -> BisectState {
        let items = vec![ItemSpec {
            path: "mods/a.pw.toml".into(),
            name: "A".into(),
            library: false,
            player_tool: false,
        }];
        BisectState {
            schema: SCHEMA,
            id: id.into(),
            symptom: Symptom::CrashOnWorld,
            goal: Goal::World,
            target: Target::default(),
            origin_crash: Some("abc".into()),
            minecraft: "1.21.1".into(),
            loader: Some("neoforge".into()),
            world: Some("mundo".into()),
            search: Search::new(Universe::new(items, &[], &[])),
            rounds: Vec::new(),
            world_ms: None,
            control_interval_ms: None,
            started_at_ms: started,
            elapsed_ms: 0,
            paused: false,
            finished_at_ms: None,
        }
    }

    #[test]
    fn grava_le_e_guarda_o_resultado_terminado() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(BisectState::load(dir.path()).unwrap(), None);
        let mut current = state("b", 2);
        current.search.record(Verdict::FailedSame);
        current.save(dir.path()).unwrap();
        assert_eq!(
            BisectState::load(dir.path()).unwrap(),
            Some(current.clone())
        );
        assert!(results(dir.path()).unwrap().is_empty());
        assert!(!dir.path().join("bisect.json.tmp").exists());

        current.search.cancel();
        current.save(dir.path()).unwrap();
        let mut older = state("a", 1);
        older.search.cancel();
        older.save(dir.path()).unwrap();
        let list = results(dir.path()).unwrap();
        assert_eq!(
            list.iter()
                .map(|state| state.id.as_str())
                .collect::<Vec<_>>(),
            ["b", "a"]
        );
    }

    #[test]
    fn estado_estragado_e_erro_proprio() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(STATE_FILE), "{ nao").unwrap();
        let error = BisectState::load(dir.path()).unwrap_err();
        assert!(matches!(error, BisectError::StateUnreadable { .. }));
    }
}
