//! Ordem de preferência dos provedores do Sensor Sculk (ADR-0070, que complementa a ADR-0060;
//! Configurações → Sensor Sculk).
//!
//! Em vez de um provedor só, a pessoa ordena os três. A conversa nova usa o **primeiro da ordem
//! com a chave funcionando**: com chave no cofre e sem recusa no último **Testar**
//! ([`crate::key_checks`]). Os outros com chave ficam de reserva; sem chave ou com a chave
//! recusada, o provedor é pulado. Sem nenhum utilizável, vale o primeiro da ordem (e a conversa
//! pede a chave dele, como antes). Uma conversa já começada continua no provedor dela.
//!
//! A ordem fica em `ai/settings.json` (`aiOrder`). O arquivo antigo, com um provedor só
//! (`aiProvider`), vale como esse provedor primeiro e os outros na ordem da tela; `aiProvider`
//! continua sendo gravado com o primeiro da ordem, para uma versão anterior do Warden ler.
//!
//! - `ai_order_set`: grava a ordem nova (setas, arrastar ou Alt+↑/↓ na tela);
//! - `ai_models_of`: os modelos que a chave de um provedor alcança, para a tabela informativa
//!   da página (o modo de cada pergunta é escolhido no Sensor Sculk, não aqui).

use tauri::State;
use warden_ai::providers::{AiProvider, ModelChoices};

use super::ai::{AiSettingsView, load_settings, models_for, save_settings, secret_of, settings_view};
use crate::error::AppError;
use crate::state::AppState;

/// A ordem completa: a gravada (sem repetições), com os que faltam no fim na ordem da tela.
/// Sem ordem gravada, `legacy` (o `aiProvider` antigo) vem primeiro.
pub(crate) fn normalized(saved: &[AiProvider], legacy: AiProvider) -> Vec<AiProvider> {
    let mut order = Vec::with_capacity(AiProvider::ALL.len());
    let start: &[AiProvider] = if saved.is_empty() {
        std::slice::from_ref(&legacy)
    } else {
        saved
    };
    for provider in start.iter().chain(AiProvider::ALL.iter()) {
        if !order.contains(provider) {
            order.push(*provider);
        }
    }
    order
}

/// A ordem com `provider` em primeiro (a escolha das boas-vindas).
pub(crate) fn moved_to_front(order: &[AiProvider], provider: AiProvider) -> Vec<AiProvider> {
    let front: Vec<AiProvider> = std::iter::once(provider)
        .chain(order.iter().copied())
        .collect();
    normalized(&front, provider)
}

/// Se o provedor pode abrir uma conversa nova: chave no cofre e não recusada no último teste.
pub(crate) fn usable(state: &AppState, provider: AiProvider) -> bool {
    let kind = secret_of(provider);
    state
        .secrets
        .status()
        .is_ok_and(|status| status.is_configured(kind))
        && !state.key_checks.refused(kind)
}

/// O provedor das conversas novas: o primeiro utilizável da ordem; sem nenhum, o primeiro.
pub(crate) fn new_conversation_provider(
    order: &[AiProvider],
    usable: impl Fn(AiProvider) -> bool,
) -> AiProvider {
    order
        .iter()
        .copied()
        .find(|provider| usable(*provider))
        .or_else(|| order.first().copied())
        .unwrap_or_default()
}

pub(crate) fn order_set_impl(
    state: &AppState,
    order: &[AiProvider],
) -> Result<AiSettingsView, AppError> {
    let mut settings = load_settings(state);
    let order = normalized(order, settings.ai_provider);
    settings.ai_provider = order.first().copied().unwrap_or_default();
    settings.ai_order = order;
    save_settings(state, &settings)?;
    tracing::info!(order = ?settings.ai_order, "ordem dos provedores do Sculk gravada");
    settings_view(state)
}

/// Grava a ordem de preferência dos provedores. Um provedor que falte vai para o fim.
#[tauri::command]
#[specta::specta]
#[allow(clippy::needless_pass_by_value)] // contrato do Tauri
pub(crate) fn ai_order_set(
    state: State<'_, AppState>,
    order: Vec<AiProvider>,
) -> Result<AiSettingsView, AppError> {
    order_set_impl(&state, &order)
}

/// Os modelos que a chave de `provider` alcança (rede só na primeira vez nesta execução).
#[tauri::command]
#[specta::specta]
pub(crate) async fn ai_models_of(
    state: State<'_, AppState>,
    provider: AiProvider,
) -> Result<ModelChoices, AppError> {
    models_for(&state, provider, false).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use AiProvider::{Anthropic, Gemini, Openai};

    #[test]
    fn ordem_antiga_vira_o_provedor_escolhido_primeiro() {
        assert_eq!(normalized(&[], Gemini), [Gemini, Openai, Anthropic]);
        assert_eq!(normalized(&[], Anthropic), [Anthropic, Gemini, Openai]);
        assert_eq!(normalized(&[], Openai), [Openai, Gemini, Anthropic]);
    }

    #[test]
    fn ordem_gravada_sem_repeticao_e_completa() {
        assert_eq!(
            normalized(&[Anthropic, Gemini, Openai], Gemini),
            [Anthropic, Gemini, Openai]
        );
        assert_eq!(
            normalized(&[Openai, Openai, Anthropic], Gemini),
            [Openai, Anthropic, Gemini]
        );
        assert_eq!(normalized(&[Anthropic], Openai), [Anthropic, Gemini, Openai]);
    }

    #[test]
    fn mover_para_o_comeco() {
        assert_eq!(
            moved_to_front(&[Anthropic, Gemini, Openai], Openai),
            [Openai, Anthropic, Gemini]
        );
        assert_eq!(
            moved_to_front(&[Anthropic, Gemini, Openai], Anthropic),
            [Anthropic, Gemini, Openai]
        );
    }

    /// ADR-0070: o primeiro com chave funcionando; recusada ou sem chave é pulado; sem nenhum,
    /// o primeiro da ordem.
    #[test]
    fn conversa_nova_usa_o_primeiro_utilizavel() {
        let order = [Anthropic, Gemini, Openai];
        assert_eq!(new_conversation_provider(&order, |_| true), Anthropic);
        assert_eq!(
            new_conversation_provider(&order, |provider| provider == Openai),
            Openai
        );
        assert_eq!(
            new_conversation_provider(&order, |provider| provider != Anthropic),
            Gemini
        );
        assert_eq!(new_conversation_provider(&order, |_| false), Anthropic);
        assert_eq!(new_conversation_provider(&[], |_| false), Gemini);
    }

    /// ADR-0070 de ponta a ponta: o arquivo antigo (`aiProvider`) vale como esse provedor em
    /// primeiro; a ordem nova é gravada; o primeiro com chave funcionando abre as conversas
    /// novas, e a chave recusada no último teste é pulada até ser testada de novo.
    #[test]
    fn ordem_gravada_e_provedor_das_conversas_novas() {
        use crate::key_checks::KeyCheckOutcome;
        use warden_secrets::{SecretKind, SecretString};

        let dir = tempfile::tempdir().unwrap();
        let state = crate::state::tests::test_state(dir.path());
        let file = state.paths.data_dir().join("ai").join("settings.json");
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, r#"{ "aiProvider": "anthropic", "tier": "careful" }"#).unwrap();
        let view = settings_view(&state).unwrap();
        assert_eq!(view.order, [Anthropic, Gemini, Openai]);
        // Ninguém tem chave: vale o primeiro (a conversa pede a chave dele).
        assert_eq!(view.provider, Anthropic);

        for kind in [SecretKind::Anthropic, SecretKind::Openai] {
            state
                .secrets
                .set(kind, &SecretString::from("chave-de-teste-0123456789"))
                .unwrap();
        }
        let view = order_set_impl(&state, &[Openai, Anthropic, Gemini]).unwrap();
        assert_eq!(view.order, [Openai, Anthropic, Gemini]);
        assert_eq!(view.provider, Openai);

        state
            .key_checks
            .record(SecretKind::Openai, KeyCheckOutcome::Refused)
            .unwrap();
        let view = settings_view(&state).unwrap();
        assert_eq!(view.provider, Anthropic, "a chave recusada é pulada");
        assert!(view.providers.iter().any(|item| item.provider == Openai && item.key_refused));

        state
            .key_checks
            .record(SecretKind::Openai, KeyCheckOutcome::Working)
            .unwrap();
        assert_eq!(settings_view(&state).unwrap().provider, Openai);

        let saved: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
        assert_eq!(saved["aiOrder"], serde_json::json!(["openai", "anthropic", "gemini"]));
        assert_eq!(saved["aiProvider"], "openai", "uma versão anterior lê o primeiro");
        assert_eq!(saved["tier"], "careful");
    }
}
