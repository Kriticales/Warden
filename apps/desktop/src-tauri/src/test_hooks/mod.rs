//! Ganchos do Testar (ROADMAP L-04; ARCHITECTURE §7.4 e §8.3).
//!
//! O teste chama cada gancho registrado em três momentos:
//! - [`TestHooks::before_prepare`]: antes de preparar o Minecraft (passo 1 do SPEC T13,
//!   "Verificar o pack (rápido)", da D-03);
//! - [`TestHooks::before_launch`]: com a instância pronta, logo antes de abrir o jogo (passo 4,
//!   "Verificação final", da D-03; linha de base da captura, da C-03; checagem de segurança,
//!   da 1.1);
//! - [`TestHooks::after_exit`]: depois que o jogo fechou, antes de gravar a sessão ("Por que
//!   travou", da D-03; "O que mudou durante o teste", da C-03). O que o gancho devolve entra no
//!   `session.json`.
//!
//! Erro num gancho `before_*` interrompe o teste com esse erro (o diálogo com Corrigir e Testar
//! mesmo assim é da D-03). Erro no `after_exit` não perde a sessão: vira aviso.
//!
//! **Registro acréscimo-apenas** (ROADMAP §1): cada tarefa declara o seu módulo
//! (`diagnostics.rs` da D-03, `capture.rs` da C-03, `security.rs` da 1.1, `perf.rs` da L-10) e
//! acrescenta uma linha em [`registered`]. Sem ganchos, o teste passa direto.

use std::path::Path;
use std::sync::Arc;

use serde_json::{Map, Value};
use warden_core::PackId;
use warden_instance::InstanceDirs;
use warden_launcher::process::GameExit;

use crate::error::AppError;
use crate::operations::OperationHandle;
use crate::test_session::TestMode;

/// O teste em andamento, como os ganchos o veem.
#[allow(dead_code)] // lidos pelos ganchos das tarefas donas (D-03, C-03)
pub(crate) struct HookContext<'a> {
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
}

/// Como o jogo terminou.
#[allow(dead_code)] // lidos pelos ganchos das tarefas donas (D-03, C-03)
pub(crate) struct ExitContext<'a> {
    /// Resultado, código e arquivos de travamento.
    pub(crate) exit: &'a GameExit,
    /// A pasta da sessão (`output.log`).
    pub(crate) session_dir: &'a Path,
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
    vec![]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sem_ganchos_o_teste_passa_direto() {
        assert!(registered().is_empty());
    }
}
