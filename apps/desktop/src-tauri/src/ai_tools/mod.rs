//! As ferramentas da IA sobre os dados do Warden (ARCHITECTURE §3 e §9.5 "Ferramentas"; ROADMAP
//! D-04): a `warden-ai` declara e conduz o laço; aqui fica quem lê o pack, os achados, as
//! sessões do Testar, os logs, as configs, o histórico e o grafo ([`PackTools`], o
//! [`warden_ai::tools::ToolSource`] do app) e o texto inicial do consentimento ([`initial`]).
//!
//! **Registro** ([`registry`]): as ferramentas de leitura da D-04. Registro acréscimo-apenas
//! (ROADMAP §1): a D-14 acrescenta as propostas e as consultas externas (`get_mod_changelog`,
//! `search_mod_issues`, `get_mixin_report`, `get_bisect_result`) com
//! [`warden_ai::tools::ToolRegistry::with_extra`], no mesmo esquema (declaradas desde o começo e
//! liberadas por `allowedFunctionNames`).
//!
//! Tudo o que sai daqui passa pela redação (ARCHITECTURE §9.4): caminhos relativos à pasta do
//! pack ou com `<usuário>`, sem o nome do jogador, IPs, e-mails nem chaves.

pub(crate) mod initial;
mod read;

pub(crate) use read::read_log_text;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};

use async_trait::async_trait;
use serde_json::Value;
use warden_ai::tools::{KnownMods, ToolOutput, ToolRegistry, ToolSource};
use warden_core::PackId;
use warden_diagnostics::Redactor;

use crate::state::AppState;

/// As ferramentas declaradas em toda conversa.
pub(crate) fn registry() -> ToolRegistry {
    ToolRegistry::new()
}

/// Id de sessão das ferramentas de log para o arquivo escolhido no computador.
pub(crate) const LOCAL_FILE_SESSION: &str = "arquivo";

/// Quem lê os dados de um pack para a IA, numa rodada do laço.
pub(crate) struct PackTools<'a> {
    pub(crate) state: &'a AppState,
    pub(crate) pack_id: PackId,
    pub(crate) root: PathBuf,
    pub(crate) redactor: Redactor,
    /// O log escolhido no computador, já redigido (conversas com essa origem).
    pub(crate) local_log: Option<PathBuf>,
    /// Logs já lidos e redigidos nesta rodada, por sessão.
    logs: Mutex<HashMap<String, Arc<Vec<String>>>>,
    /// Os mods do pack (com os ids dos jars), lidos uma vez.
    mods: tokio::sync::OnceCell<read::PackMods>,
}

impl<'a> PackTools<'a> {
    pub(crate) fn new(
        state: &'a AppState,
        pack_id: PackId,
        root: PathBuf,
        local_log: Option<PathBuf>,
    ) -> Self {
        let player = state.settings.get().player_name;
        Self {
            state,
            pack_id,
            root,
            redactor: crate::commands::report::redactor(&player, &state.secrets),
            local_log,
            logs: Mutex::new(HashMap::new()),
            mods: tokio::sync::OnceCell::new(),
        }
    }

    fn cached_log(&self, session: &str) -> Option<Arc<Vec<String>>> {
        self.logs
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(session)
            .cloned()
    }

    fn store_log(&self, session: &str, lines: Vec<String>) -> Arc<Vec<String>> {
        let lines = Arc::new(lines);
        self.logs
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(session.to_owned(), Arc::clone(&lines));
        lines
    }

    async fn pack_mods(&self) -> &read::PackMods {
        self.mods.get_or_init(|| read::PackMods::load(self)).await
    }
}

#[async_trait]
impl ToolSource for PackTools<'_> {
    async fn call(&self, name: &str, args: &Value) -> Result<ToolOutput, String> {
        read::call(self, name, args).await
    }

    async fn known_mods(&self) -> KnownMods {
        self.pack_mods().await.known()
    }
}

#[cfg(test)]
mod tests;
