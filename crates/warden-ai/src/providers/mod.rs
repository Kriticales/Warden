//! Os provedores de IA (ADR-0060; ARCHITECTURE §9.5 "Provedores").
//!
//! - [`AiProvider`]: o id gravado nas Configurações e em cada conversa.
//! - [`Provider`]: o que o laço ([`crate::engine`]) pede a um provedor, sempre no formato comum
//!   de [`chat`]. Cada tradutor fica na sua pasta: [`gemini`] (D-04, migrado na D-15),
//!   [`openai`] (D-16) e [`anthropic`] (D-17).
//! - [`connect`]: o cliente do provedor com a chave do usuário, sobre um [`HttpClient`] feito
//!   com [`http_config`].
//! - [`SearchMode`] e [`ModelChoices`]: "Procurar rápido" e "Procurar a fundo", escolhidos a
//!   cada pergunta (ADR-0064), com os modelos de cada provedor tirados da lista de modelos da
//!   conta (nenhum nome fixo no código, ADR-0055), e o modelo mais leve para quando a cota
//!   diária do escolhido acaba (só o Gemini tem, ADR-0060).

pub mod anthropic;
pub mod chat;
pub mod gemini;
pub mod openai;

use std::time::Duration;

use async_trait::async_trait;
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use warden_core::CancellationToken;
use warden_http::{HttpClient, HttpConfig};

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

    /// O modo de um nome de modelo deste provedor (`None`: não é um dos que o Warden escolhe).
    #[must_use]
    pub fn mode_of(self, model: &str) -> Option<SearchMode> {
        match self {
            Self::Gemini => gemini::models::mode_of(model),
            Self::Openai => openai::models::mode_of(model),
            Self::Anthropic => anthropic::models::mode_of(model),
        }
    }
}

/// Como o Sculk procura numa pergunta (ADR-0064): escolhido a cada pergunta no Claude e na
/// OpenAI; no Gemini não há escolha (os dois usam o Flash estável mais novo, com a troca para o
/// Flash-Lite no limite diário, ADR-0060).
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "camelCase")]
pub enum SearchMode {
    /// "Procurar rápido": o modelo estável mais econômico do provedor e menos consultas.
    #[default]
    Quick,
    /// "Procurar a fundo": o modelo estável mais forte da faixa barata e mais consultas.
    Deep,
}

impl SearchMode {
    /// Rodadas de consulta por pergunta neste modo (ARCHITECTURE §9.5 "Laço": 8 a fundo).
    #[must_use]
    pub const fn max_rounds(self) -> u32 {
        match self {
            Self::Quick => 4,
            Self::Deep => 8,
        }
    }
}

/// Os modelos resolvidos para uma chave.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ModelChoices {
    /// O modelo de "Procurar rápido" (`None`: nenhum disponível).
    pub economical: Option<String>,
    /// O modelo de "Procurar a fundo".
    pub careful: Option<String>,
    /// O modelo mais leve, que o laço usa sozinho numa pergunta quando a cota diária do modelo
    /// da conversa acaba (`None`: o provedor não troca; no Gemini, o Flash-Lite mais novo).
    #[serde(default)]
    #[specta(optional)]
    pub lighter: Option<String>,
}

impl ModelChoices {
    /// O modelo do modo escolhido; sem ele, o do outro modo (melhor que nenhum).
    #[must_use]
    pub fn pick(&self, mode: SearchMode) -> Option<(&str, SearchMode)> {
        let (first, second) = match mode {
            SearchMode::Quick => (
                (self.economical.as_deref(), SearchMode::Quick),
                (self.careful.as_deref(), SearchMode::Deep),
            ),
            SearchMode::Deep => (
                (self.careful.as_deref(), SearchMode::Deep),
                (self.economical.as_deref(), SearchMode::Quick),
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

/// O maior tempo-limite por requisição entre os provedores.
pub const LONGEST_REQUEST_TIMEOUT: Duration = {
    let mut longest = gemini::REQUEST_TIMEOUT;
    if openai::REQUEST_TIMEOUT.as_secs() > longest.as_secs() {
        longest = openai::REQUEST_TIMEOUT;
    }
    if anthropic::REQUEST_TIMEOUT.as_secs() > longest.as_secs() {
        longest = anthropic::REQUEST_TIMEOUT;
    }
    longest
};

/// A configuração do [`HttpClient`] dos provedores, a partir da do app (D-15b).
///
/// Os provedores respondem sem *streaming*: nada chega até a resposta inteira ficar pronta, e um
/// modelo com raciocínio pode levar mais de um minuto. O `read_timeout` do app (30 s sem receber
/// dados) cortava essas respostas antes do tempo-limite do provedor (o Noturno 39215806251 viu o
/// Flash parar 5 vezes em 30 s com "90 s" no erro). Aqui ele sobe para o maior tempo-limite dos
/// provedores; quem limita cada requisição continua sendo o tempo-limite do provedor
/// ([`Provider::timeout`]).
#[must_use]
pub fn http_config(mut config: HttpConfig) -> HttpConfig {
    config.read_timeout = config.read_timeout.max(LONGEST_REQUEST_TIMEOUT);
    config
}

/// O cliente do provedor com o endereço oficial (ou o do servidor simulado, em build de debug).
/// O `http` deve vir de [`http_config`].
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
    fn leitura_espera_o_tempo_limite_de_cada_provedor() {
        let app = HttpConfig::for_version("9.9.9");
        let config = http_config(app.clone());
        for timeout in [
            gemini::REQUEST_TIMEOUT,
            openai::REQUEST_TIMEOUT,
            anthropic::REQUEST_TIMEOUT,
        ] {
            assert!(config.read_timeout >= timeout, "{timeout:?}");
        }
        // O resto da configuração do app fica igual.
        assert_eq!(config.connect_timeout, app.connect_timeout);
        assert_eq!(config.request_timeout, app.request_timeout);
        assert_eq!(config.user_agent, app.user_agent);
    }

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
    fn modo_estavel_e_limita_as_consultas() {
        assert_eq!(serde_json::to_value(SearchMode::Deep).unwrap(), "deep");
        assert_eq!(serde_json::to_value(SearchMode::Quick).unwrap(), "quick");
        assert!(SearchMode::Quick.max_rounds() < SearchMode::Deep.max_rounds());
    }

    #[test]
    fn modo_pelo_provedor() {
        assert_eq!(
            AiProvider::Gemini.mode_of("gemini-3.5-flash-lite"),
            Some(SearchMode::Quick)
        );
        assert_eq!(AiProvider::Openai.mode_of("gemini-3.5-flash-lite"), None);
        assert_eq!(
            AiProvider::Openai.mode_of("gpt-6-sol"),
            Some(SearchMode::Deep)
        );
        assert_eq!(
            AiProvider::Anthropic.mode_of("claude-sonnet-5-5"),
            Some(SearchMode::Quick)
        );
        assert_eq!(AiProvider::Anthropic.mode_of("gpt-6-sol"), None);
    }

    #[test]
    fn escolha_cai_no_outro_nivel_quando_falta() {
        let only_flash = ModelChoices {
            economical: None,
            careful: Some("gemini-3.8-flash".into()),
            lighter: None,
        };
        assert_eq!(
            only_flash.pick(SearchMode::Quick),
            Some(("gemini-3.8-flash", SearchMode::Deep))
        );
        assert_eq!(ModelChoices::default().pick(SearchMode::Deep), None);
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
