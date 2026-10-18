//! Ganchos do Testar (ROADMAP L-04; ARCHITECTURE §7.4 e §8.3).
//!
//! O teste chama cada gancho registrado nestes momentos:
//! - [`TestHooks::before_prepare`]: antes de preparar o Minecraft (passo 1 do SPEC T13,
//!   "Verificar o pack (rápido)", da D-03);
//! - [`TestHooks::before_launch`]: com a instância pronta, logo antes de abrir o jogo (passo 4,
//!   "Verificação final", da D-03; linha de base da captura, da C-03; checagem de segurança,
//!   da 1.1);
//! - [`TestHooks::on_game_started`] e [`TestHooks::on_game_line`]: com o jogo aberto, logo
//!   depois de abrir o processo e a cada linha da saída (desempenho, da L-10);
//! - [`TestHooks::after_exit`]: depois que o jogo fechou, antes de gravar a sessão ("Por que
//!   travou", da D-03; "O que mudou durante o teste", da C-03). O que o gancho devolve entra no
//!   `session.json`.
//!
//! Erro num gancho `before_*` interrompe o teste com esse erro (o diálogo com Corrigir e Testar
//! mesmo assim é da D-03: com "Testar mesmo assim", o pedido chega com
//! [`HookContext::ignore_pack_errors`]). Erro no `after_exit` não perde a sessão: vira aviso.
//!
//! **Registro acréscimo-apenas** (ROADMAP §1): cada tarefa declara o seu módulo
//! (`diagnostics.rs` da D-03, `capture.rs` da C-03, `security.rs` da 1.1, `perf.rs` da L-10,
//! `crash_history.rs` da D-06) e acrescenta uma linha em [`registered`]. [`registered`] é chamado a cada teste: cada gancho
//! pode guardar o estado do teste em andamento.

mod capture;
mod crash_history;
mod diagnostics;
mod perf;

pub(crate) use perf::session_perf;

use std::path::Path;
use std::sync::Arc;
use std::time::SystemTime;

use serde_json::{Map, Value};
use warden_core::{AppPaths, PackId};
use warden_instance::InstanceDirs;
use warden_launcher::GameSpec;
use warden_launcher::log::LogLine;
use warden_launcher::process::{GameExit, GameHandle};
use warden_launcher::session::SessionRecord;

use crate::error::AppError;
use crate::events::OperationEvent;
use crate::operations::OperationHandle;
use crate::state::AppState;
use crate::test_session::TestMode;

/// O teste em andamento, como os ganchos o veem.
#[allow(dead_code)] // lidos pelos ganchos das tarefas donas (D-03, C-03)
pub(crate) struct HookContext<'a> {
    /// O estado do app (serviços, pastas, cofre).
    pub(crate) state: &'a AppState,
    /// As pastas do app (dados locais, cache).
    pub(crate) paths: &'a AppPaths,
    /// O pack.
    pub(crate) pack_id: PackId,
    /// A pasta do pack.
    pub(crate) pack_root: &'a Path,
    /// A instância de teste (`gameDir` e `state`).
    pub(crate) dirs: &'a InstanceDirs,
    /// O modo do teste.
    pub(crate) mode: TestMode,
    /// A operação do teste: etapas, progresso, avisos e cancelamento.
    pub(crate) operation: &'a OperationHandle,
    /// "Testar mesmo assim": os erros do pré-teste não interrompem o teste (D-03).
    pub(crate) ignore_pack_errors: bool,
}

/// O jogo que acabou de abrir.
#[allow(dead_code)] // lidos pelos ganchos das tarefas donas
pub(crate) struct GameContext<'a> {
    /// O pid do processo principal.
    pub(crate) pid: u32,
    /// Quando o processo começou.
    pub(crate) started_at: SystemTime,
    /// Alça do processo (parar, processos vivos, pico de memória do Job Object).
    pub(crate) game: &'a GameHandle,
    /// O Minecraft e o loader do teste.
    pub(crate) spec: &'a GameSpec,
    /// A versão do pack testada (`version` do `pack.toml`).
    pub(crate) pack_version: Option<&'a str>,
    /// A pasta da sessão, já criada (o `session.json` só existe no fim).
    pub(crate) session_dir: &'a Path,
    /// O canal do teste, para eventos ao vivo (`PerfSample`).
    pub(crate) events: &'a Arc<dyn Fn(OperationEvent) + Send + Sync>,
    /// A memória do teste (`-Xmx`), em MB.
    pub(crate) memory_mb: u32,
    /// A memória automática do Warden para o tamanho do pack, em MB (a sugestão dos avisos de
    /// memória, L-11).
    pub(crate) pack_suggestion_mb: u32,
    /// Os argumentos da JVM do teste, como foram para a linha de comando.
    pub(crate) jvm_args: &'a [String],
    /// As variáveis de ambiente que o Warden acrescentou ao processo do jogo.
    pub(crate) game_env: &'a [(String, String)],
}

/// Como o jogo terminou.
#[allow(dead_code)] // lidos pelos ganchos das tarefas donas (D-03, C-03)
pub(crate) struct ExitContext<'a> {
    /// Resultado, código e arquivos de travamento.
    pub(crate) exit: &'a GameExit,
    /// A pasta da sessão (`output.log`).
    pub(crate) session_dir: &'a Path,
    /// O registro da sessão como vai ser gravado (com os campos do Testar), antes dos campos
    /// dos ganchos.
    pub(crate) record: &'a SessionRecord,
}

/// Um conjunto de ganchos. Os métodos têm implementação vazia: cada tarefa implementa só os
/// momentos de que precisa.
#[async_trait::async_trait]
pub(crate) trait TestHooks: Send + Sync {
    /// Nome para os registros.
    fn name(&self) -> &'static str;

    /// Antes de preparar o Minecraft.
    async fn before_prepare(&self, _context: &HookContext<'_>) -> Result<(), AppError> {
        Ok(())
    }

    /// Com a instância pronta, logo antes de abrir o jogo.
    async fn before_launch(&self, _context: &HookContext<'_>) -> Result<(), AppError> {
        Ok(())
    }

    /// O jogo acabou de abrir. Não pode demorar: o console só começa depois dele (trabalho
    /// contínuo vai para uma tarefa própria, que termina no `after_exit`).
    async fn on_game_started(&self, _context: &HookContext<'_>, _game: &GameContext<'_>) {}

    /// Uma linha da saída do jogo, na ordem. Chamado na tarefa que lê o jogo: só trabalho
    /// curto, sem bloquear.
    fn on_game_line(&self, _line: &LogLine) {}

    /// Depois que o jogo fechou. Os campos devolvidos entram no `session.json`.
    async fn after_exit(
        &self,
        _context: &HookContext<'_>,
        _exit: &ExitContext<'_>,
    ) -> Result<Map<String, Value>, AppError> {
        Ok(Map::new())
    }
}

/// Os ganchos registrados, na ordem em que rodam. Uma linha por tarefa.
pub(crate) fn registered() -> Vec<Arc<dyn TestHooks>> {
    vec![
        // D-03: pré-teste (rápido) e verificação final; "Por que travou" no fim.
        Arc::new(diagnostics::DiagnosticsHooks),
        // C-03: linha de base antes de abrir o jogo e o que mudou durante o teste.
        Arc::new(capture::CaptureHooks),
        // L-10: RAM do processo e tempos até carregar e entrar no mundo.
        Arc::new(perf::PerfHook::default()),
        // D-06: assinatura do travamento, para a lista de Travamentos.
        Arc::new(crash_history::CrashHistoryHook),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ganchos_registrados_em_ordem() {
        let names: Vec<&str> = registered().iter().map(|hook| hook.name()).collect();
        assert_eq!(names, ["diagnostics", "capture", "perf", "crash_history"]);
    }

    #[test]
    fn cada_teste_recebe_ganchos_novos() {
        // Cada chamada cria instâncias novas: o estado de um teste não passa para o próximo.
        let first = registered();
        let second = registered();
        let perf = first.iter().position(|hook| hook.name() == "perf").unwrap();
        assert!(!Arc::ptr_eq(&first[perf], &second[perf]));
    }
}
