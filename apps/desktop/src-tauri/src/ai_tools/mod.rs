//! As ferramentas da IA sobre os dados do Warden (ARCHITECTURE §3 e §9.5 "Ferramentas"; ROADMAP
//! D-04): a `warden-ai` declara e conduz o laço; aqui fica quem lê o pack, os achados, as
//! sessões do Testar, os logs, as configs, o histórico e o grafo ([`PackTools`], o
//! [`warden_ai::tools::ToolSource`] do app) e o texto inicial do consentimento ([`initial`]).
//!
//! **Registro** ([`registry`]): as ferramentas de leitura da D-04. Registro acréscimo-apenas
//! (ROADMAP §1): a D-14 acrescenta as propostas e as consultas externas (`get_mod_changelog`,
//! `search_mod_issues`, `get_mixin_report`, `get_bisect_result`) com
//! [`warden_ai::tools::ToolRegistry::with_extra`], no mesmo esquema (declaradas desde o começo e
//! liberadas por `allowedFunctionNames`): [`external`], [`xray`], [`bisect`] e [`proposals`]
//! (que também prepara, aplica e descarta as propostas pelos fluxos normais).
//!
//! Tudo o que sai daqui passa pela redação (ARCHITECTURE §9.4): caminhos relativos à pasta do
//! pack ou com `<usuário>`, sem o nome do jogador, IPs, e-mails nem chaves.

mod bisect;
pub(crate) mod external;
pub(crate) mod initial;
pub(crate) mod proposals;
mod read;
mod xray;

use read::failed;
pub(crate) use read::read_log_text;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};

use async_trait::async_trait;
use serde_json::Value;
use warden_ai::external::{
    GET_BISECT_RESULT, GET_MIXIN_REPORT, GET_MOD_CHANGELOG, SEARCH_MOD_ISSUES,
};
use warden_ai::proposals::{ProposalKind, ProposalSummary};
use warden_ai::tools::{KnownMods, ToolOutput, ToolRegistry, ToolSource};
use warden_core::PackId;
use warden_diagnostics::Redactor;

use crate::state::AppState;

/// As ferramentas declaradas em toda conversa.
pub(crate) fn registry() -> ToolRegistry {
    ToolRegistry::new()
        // D-14: consultas externas, raio-x, busca do culpado e propostas.
        .with_extra(warden_ai::external::tools())
        .with_extra(warden_ai::proposals::tools())
}

/// Um argumento de texto (vazio se faltar).
fn text_arg(args: &Value, key: &str) -> String {
    args.get(key)
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_owned()
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
        let output = match name {
            SEARCH_MOD_ISSUES => external::search_mod_issues(self, args).await,
            GET_MOD_CHANGELOG => external::mod_changelog(self, args).await,
            GET_MIXIN_REPORT => xray::mixin_report(self, args).await,
            GET_BISECT_RESULT => bisect::bisect_result(self),
            _ => return read::call(self, name, args).await,
        };
        // Nada sai sem passar pela redação (inclusive as mensagens de erro).
        match output {
            Ok(mut output) => {
                for item in &mut output.items {
                    item.text = self.redactor.redact(&item.text).text;
                }
                Ok(output)
            }
            Err(message) => Err(self.redactor.redact(&message).text),
        }
    }

    async fn known_mods(&self) -> KnownMods {
        self.pack_mods().await.known()
    }

    async fn check_proposal(&self, change: &ProposalKind) -> Result<ProposalSummary, String> {
        proposals::check(self, change)
            .await
            .map_err(|message| self.redactor.redact(&message).text)
    }
}

#[cfg(test)]
mod tests;
