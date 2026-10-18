//! Comandos das propostas da IA (ARCHITECTURE §4.1 domínio `ai` e §9.5 "Propostas"; SPEC T14 ✦
//! passo 3; ROADMAP D-14). Finos: o trabalho é de `ai_tools/proposals.rs`.
//!
//! - `ai_proposal_review`: a diferença exata de uma proposta pendente, para o diálogo de
//!   **Aplicar** (nada é gravado), com o `token` do que foi mostrado.
//! - `ai_proposal_apply`: grava pelo fluxo normal o que foi mostrado (o `token` confere; senão
//!   `ai.PROPOSAL_STALE` e nada muda), com ponto de segurança antes quando muda o pack.
//! - `ai_proposal_discard`: só a decisão, na conversa; nada muda no pack.
//!
//! Erros: `ai.PROPOSAL_NOT_FOUND`, `ai.PROPOSAL_DECIDED`, `ai.PROPOSAL_STALE`,
//! `ai.CONVERSATION_BUSY` (a IA está respondendo) e os dos fluxos normais.
#![allow(clippy::needless_pass_by_value)] // argumentos pertencem ao contrato do Tauri

use tauri::{AppHandle, State};
use warden_ai::proposals::Proposal;
use warden_core::PackId;

use crate::ai_tools::proposals::{self, ProposalApplied, ProposalReview, areas_of};
use crate::commands::ai::app_notify;
use crate::commands::inventory::notify_changed;
use crate::error::AppError;
use crate::state::AppState;

/// A diferença exata de uma proposta (o diálogo de Aplicar). Nada é gravado.
#[tauri::command]
#[specta::specta]
pub(crate) async fn ai_proposal_review(
    state: State<'_, AppState>,
    pack_id: PackId,
    conversation_id: String,
    proposal_id: String,
) -> Result<ProposalReview, AppError> {
    proposals::review(&state, pack_id, &conversation_id, &proposal_id).await
}

/// Aplica a proposta mostrada no diálogo (o `token` da prévia).
#[tauri::command]
#[specta::specta]
pub(crate) async fn ai_proposal_apply(
    app: AppHandle,
    state: State<'_, AppState>,
    pack_id: PackId,
    conversation_id: String,
    proposal_id: String,
    token: String,
) -> Result<ProposalApplied, AppError> {
    let result = proposals::apply(&state, pack_id, &conversation_id, &proposal_id, &token).await;
    if let Ok(applied) = &result {
        let areas = areas_of(&applied.proposal.change);
        if !areas.is_empty() {
            notify_changed(&app, pack_id, areas);
        }
    }
    app_notify(app)(pack_id, &conversation_id);
    result
}

/// Descarta a proposta (nada muda no pack).
#[tauri::command]
#[specta::specta]
pub(crate) fn ai_proposal_discard(
    app: AppHandle,
    state: State<'_, AppState>,
    pack_id: PackId,
    conversation_id: String,
    proposal_id: String,
) -> Result<Proposal, AppError> {
    let result = proposals::discard(&state, pack_id, &conversation_id, &proposal_id);
    app_notify(app)(pack_id, &conversation_id);
    result
}
