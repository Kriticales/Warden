//! Os provedores de IA (ADR-0060; ARCHITECTURE §9.5 "Provedores").
//!
//! - [`AiProvider`]: o id gravado nas Configurações e em cada conversa.
//! - [`Provider`]: o que o laço ([`crate::engine`]) pede a um provedor, sempre no formato comum
//!   de [`chat`]. Cada tradutor fica na sua pasta: [`gemini`] (D-04, migrado na D-15),
//!   [`openai`] (D-16) e [`anthropic`] (D-17).
//! - [`connect`]: o cliente do provedor com a chave do usuário.
//! - [`ModelTier`] e [`ModelChoices`]: "Mais econômico" e "Análise mais cuidadosa", escolhidos
//!   por cada provedor pela lista de modelos da conta (nenhum nome fixo no código, ADR-0055), e
//!   o modelo mais leve para quando a cota diária do escolhido acaba (só o Gemini tem, ADR-0060).

pub mod anthropic;
pub mod chat;
pub mod gemini;
pub mod openai;

use std::time::Duration;

use async_trait::async_trait;
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use warden_core::CancellationToken;
use warden_http::HttpClient;

pub use chat::{
    ChatRequest, ChatResponse, Failure, Message, Opaque, Role, StopReason, ToolCall, ToolChoice,
    ToolDecl, ToolResult,
};

use crate::error::Result;

/// Um provedor de IA.
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    Serialize,
    Deserialize,
    specta::Type,
)]
#[serde(rename_all = "camelCase")]
pub enum AiProvider {
    /// Google Gemini (o padrão: tem plano gratuito).
    #[default]
    Gemini,
    /// OpenAI.
    Openai,
    /// Anthropic (o nome exibido é "Claude").
    Anthropic,
}

impl AiProvider {
    /// Todos, na ordem da tela de Configurações.
    pub const ALL: [Self; 3] = [Self::Gemini, Self::Openai, Self::Anthropic];

    /// O id gravado (`gemini`, `openai`, `anthropic`).
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Gemini => "gemini",
            Self::Openai => "openai",
            Self::Anthropic => "anthropic",
        }
    }

    /// O nome próprio exibido (glossário, QUALITY §8), para os parâmetros das frases de erro.
    #[must_use]
    pub const fn display_name(self) -> &'static str {
        match self {
            Self::Gemini => "Gemini",
            Self::Openai => "OpenAI",
            Self::Anthropic => "Claude",
        }
    }

    /// O provedor tem plano gratuito pela API (o aviso de plano gratuito só aparece nele).
    #[must_use]
    pub const fn has_free_tier(self) -> bool {
        matches!(self, Self::Gemini)
    }

    /// O nível de um nome de modelo deste provedor (`None`: não é um dos que o Warden escolhe).
    #[must_use]
    pub fn tier_of(self, model: &str) -> Option<ModelTier> {
        match self {
            Self::Gemini => gemini::models::tier_of(model),
            Self::Openai => openai::models::tier_of(model),
            Self::Anthropic => anthropic::models::tier_of(model),
        }
    }
}

/// A escolha em Configurações, por provedor.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "camelCase")]
pub enum ModelTier {
    /// Padrão: o modelo estável mais econômico do provedor. No Gemini, os dois níveis usam o
    /// modelo mais forte do plano gratuito, o Flash mais novo (decisão do dono, ADR-0060).
    #[default]
    Economical,
    /// "Análise mais cuidadosa": o modelo estável mais capaz da faixa barata (no Gemini, o
    /// Flash mais novo, com poucas conversas por dia no plano gratuito).
    Careful,
}

/// Os modelos resolvidos para uma chave.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ModelChoices {
    /// O modelo do nível "Mais econômico" (`None`: nenhum disponível).
    pub economical: Option<String>,
    /// O modelo do nível "Análise mais cuidadosa".
    pub careful: Option<String>,
    /// O modelo mais leve, que o laço usa sozinho numa pergunta quando a cota diária do modelo
    /// da conversa acaba (`None`: o provedor não troca; no Gemini, o Flash-Lite mais novo).
    #[serde(default)]
    #[specta(optional)]
    pub lighter: Option<String>,
}

impl ModelChoices {
    /// O modelo do nível escolhido; sem ele, o do outro nível (melhor que nenhum).
    #[must_use]
    pub fn pick(&self, tier: ModelTier) -> Option<(&str, ModelTier)> {
        let (first, second) = match tier {
            ModelTier::Economical => (
                (self.economical.as_deref(), ModelTier::Economical),
                (self.careful.as_deref(), ModelTier::Careful),
            ),
            ModelTier::Careful => (
                (self.careful.as_deref(), ModelTier::Careful),
                (self.economical.as_deref(), ModelTier::Economical),
            ),
        };
        first
            .0
            .map(|model| (model, first.1))
            .or_else(|| second.0.map(|model| (model, second.1)))
    }
}

/// O que o laço pede a um provedor. Sem novas tentativas aqui: cada requisição sai uma vez e
/// volta classificada ([`Failure`]); quem repete, espera ou para é o laço.
#[async_trait]
pub trait Provider: Send + Sync {
    /// Qual provedor.
    fn id(&self) -> AiProvider;

    /// O tempo-limite de cada requisição.
    fn timeout(&self) -> Duration;

    /// Uma rodada: manda o pedido e devolve a resposta no formato comum.
    async fn generate(
        &self,
        request: &ChatRequest,
        cancel: &CancellationToken,
    ) -> std::result::Result<ChatResponse, Failure>;

    /// O tamanho do pedido em tokens, contado pelo provedor (`None`: o provedor não conta).
    async fn count_tokens(
        &self,
        _request: &ChatRequest,
        _cancel: &CancellationToken,
    ) -> std::result::Result<Option<u64>, Failure> {
        Ok(None)
    }

    /// Os modelos que a chave alcança, pela regra do provedor. Chave recusada vira
    /// [`crate::Error::KeyInvalid`].
    async fn models(&self) -> Result<ModelChoices>;

    /// O resultado de uma ferramenta exatamente como ele vai no pedido (o bloco "Enviado à
    /// IA", CA-T14-03).
    fn tool_result_text(&self, result: &ToolResult) -> String;
}

/// O cliente do provedor com o endereço oficial (ou o do servidor simulado, em build de debug).
pub fn connect(
    provider: AiProvider,
    http: HttpClient,
    key: SecretString,
) -> Result<Box<dyn Provider>> {
    match provider {
        AiProvider::Gemini => Ok(Box::new(gemini::GeminiClient::new(http, key)?)),
        AiProvider::Openai => Ok(Box::new(openai::OpenAiClient::new(http, key)?)),
        AiProvider::Anthropic => Ok(Box::new(anthropic::AnthropicClient::new(http, key)?)),
    }
}

/// O cliente do provedor com outro endereço (testes).
pub fn connect_with_base_url(
    provider: AiProvider,
    http: HttpClient,
    base: &str,
    key: SecretString,
) -> Result<Box<dyn Provider>> {
    match provider {
        AiProvider::Gemini => Ok(Box::new(gemini::GeminiClient::with_base_url(
            http, base, key,
        )?)),
        AiProvider::Openai => Ok(Box::new(openai::OpenAiClient::with_base_url(
            http, base, key,
        )?)),
        AiProvider::Anthropic => Ok(Box::new(anthropic::AnthropicClient::with_base_url(
            http, base, key,
        )?)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_estaveis_e_gemini_padrao() {
        assert_eq!(
            serde_json::to_value(AiProvider::ALL).unwrap(),
            serde_json::json!(["gemini", "openai", "anthropic"])
        );
        for provider in AiProvider::ALL {
            assert_eq!(serde_json::to_value(provider).unwrap(), provider.id());
        }
        assert_eq!(AiProvider::default(), AiProvider::Gemini);
        assert!(AiProvider::Gemini.has_free_tier());
        assert!(!AiProvider::Openai.has_free_tier() && !AiProvider::Anthropic.has_free_tier());
        assert_eq!(AiProvider::Anthropic.display_name(), "Claude");
    }

    #[test]
    fn nivel_pelo_provedor() {
        assert_eq!(
            AiProvider::Gemini.tier_of("gemini-3.5-flash-lite"),
            Some(ModelTier::Economical)
        );
        assert_eq!(AiProvider::Openai.tier_of("gemini-3.5-flash-lite"), None);
        assert_eq!(
            AiProvider::Openai.tier_of("gpt-6-sol"),
            Some(ModelTier::Careful)
        );
        assert_eq!(
            AiProvider::Anthropic.tier_of("claude-sonnet-5-5"),
            Some(ModelTier::Economical)
        );
        assert_eq!(AiProvider::Anthropic.tier_of("gpt-6-sol"), None);
    }

    #[test]
    fn escolha_cai_no_outro_nivel_quando_falta() {
        let only_flash = ModelChoices {
            economical: None,
            careful: Some("gemini-3.8-flash".into()),
            lighter: None,
        };
        assert_eq!(
            only_flash.pick(ModelTier::Economical),
            Some(("gemini-3.8-flash", ModelTier::Careful))
        );
        assert_eq!(ModelChoices::default().pick(ModelTier::Careful), None);
    }

    #[test]
    fn cada_provedor_conecta_com_o_seu_tradutor() {
        let http = HttpClient::new(warden_http::HttpConfig::for_version("9.9.9")).unwrap();
        for provider in AiProvider::ALL {
            let client = connect(provider, http.clone(), SecretString::from("x")).unwrap();
            assert_eq!(client.id(), provider);
            let client = connect_with_base_url(
                provider,
                http.clone(),
                "http://127.0.0.1:9/",
                SecretString::from("x"),
            )
            .unwrap();
            assert_eq!(client.id(), provider);
        }
    }
}
