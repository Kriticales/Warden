//! O formato comum entre o laço e os provedores (ADR-0060; ARCHITECTURE §9.5 "Provedores").
//!
//! O laço ([`crate::engine`]) só conhece estes tipos; cada tradutor (`providers/<provedor>/`)
//! converte o pedido para a sua API e a resposta de volta, e classifica os erros em [`Failure`].
//!
//! - [`Message`]: sistema, usuário (texto), assistente (texto e [`ToolCall`]) e resultados de
//!   ferramenta ([`ToolResult`], todos os de uma rodada numa mensagem). O campo [`Opaque`] guarda
//!   o que o provedor exige receber de volta igual (o `content` inteiro do Gemini, com as
//!   *thought signatures*; os blocos de raciocínio da Anthropic): o tradutor do mesmo provedor o
//!   reenvia sem tocar.
//! - [`ChatRequest`]: modelo, mensagens, ferramentas ([`ToolDecl`]), a escolha de ferramenta
//!   ([`ToolChoice`]) e, na reserva, o esquema da resposta em JSON.
//! - [`ChatResponse`]: a mensagem do assistente, o motivo de parada, o uso de tokens e o corpo
//!   exato enviado.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::AiProvider;
use crate::conversation::TokenUsage;
use crate::error::Error;

/// Quem fala numa mensagem.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Role {
    /// As instruções do Warden (o *system prompt*).
    System,
    /// O usuário (ou o Warden em nome dele, como o aviso do limite de rodadas).
    User,
    /// A IA: texto e chamadas de ferramenta.
    Assistant,
    /// Os resultados das ferramentas de uma rodada.
    Tool,
}

/// Uma chamada de ferramenta pedida pela IA.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCall {
    /// O `id` da chamada (o resultado repete o mesmo). O Gemini antigo pode não mandar.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// A ferramenta.
    pub name: String,
    /// Os argumentos, como vieram (objeto JSON).
    pub args: Value,
}

/// O resultado de uma ferramenta, como vai para a IA.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolResult {
    /// O `id` da chamada que ele responde.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// A ferramenta.
    pub name: String,
    /// O conteúdo (objeto JSON: os itens citáveis, ou `{ "erro": … }`).
    pub content: Value,
}

/// O que um provedor exige receber de volta igual, marcado com o provedor que o mandou.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Opaque {
    /// O provedor dono do valor (outro provedor o ignora).
    pub provider: AiProvider,
    /// O valor, nunca reconstruído.
    pub value: Value,
}

/// Uma mensagem da conversa no formato comum.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    /// Quem fala.
    pub role: Role,
    /// O texto (sistema, usuário e o texto visível do assistente, sem o pensamento).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub text: String,
    /// As chamadas de ferramenta (assistente).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub calls: Vec<ToolCall>,
    /// Os resultados das ferramentas (resultado de ferramenta).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub results: Vec<ToolResult>,
    /// O que o provedor exige receber de volta igual.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opaque: Option<Opaque>,
}

impl Message {
    fn new(role: Role) -> Self {
        Self {
            role,
            text: String::new(),
            calls: Vec::new(),
            results: Vec::new(),
            opaque: None,
        }
    }

    /// As instruções do Warden.
    #[must_use]
    pub fn system(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            ..Self::new(Role::System)
        }
    }

    /// Uma mensagem do usuário.
    #[must_use]
    pub fn user(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            ..Self::new(Role::User)
        }
    }

    /// Uma resposta da IA.
    #[must_use]
    pub fn assistant(text: String, calls: Vec<ToolCall>, opaque: Option<Opaque>) -> Self {
        Self {
            text,
            calls,
            opaque,
            ..Self::new(Role::Assistant)
        }
    }

    /// Os resultados das ferramentas de uma rodada.
    #[must_use]
    pub fn tool_results(results: Vec<ToolResult>) -> Self {
        Self {
            results,
            ..Self::new(Role::Tool)
        }
    }

    /// O texto, se é uma mensagem do usuário.
    #[must_use]
    pub fn user_text(&self) -> Option<&str> {
        (self.role == Role::User).then_some(self.text.as_str())
    }

    /// O valor opaco, se veio deste provedor.
    #[must_use]
    pub fn opaque_of(&self, provider: AiProvider) -> Option<&Value> {
        self.opaque
            .as_ref()
            .filter(|opaque| opaque.provider == provider)
            .map(|opaque| &opaque.value)
    }
}

/// Uma ferramenta declarada.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolDecl {
    /// Nome.
    pub name: String,
    /// Descrição para a IA.
    pub description: String,
    /// JSON Schema dos argumentos.
    pub parameters: Value,
}

/// Que ferramentas a IA pode (ou tem de) chamar nesta requisição.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolChoice {
    /// A IA decide.
    Auto,
    /// Só estes nomes, e a resposta tem de ser uma chamada (o escalonamento por etapa, com a
    /// `responder` sempre na lista). No Gemini, `ANY` com `allowedFunctionNames` (D-15b: com
    /// `VALIDATED`, o Flash respondia em texto e caía na reserva).
    Required(Vec<String>),
    /// Nenhuma ferramenta (a reserva, com [`ChatRequest::response_schema`]).
    None,
}

/// Um pedido ao provedor.
#[derive(Debug, Clone, PartialEq)]
pub struct ChatRequest {
    /// O modelo.
    pub model: String,
    /// As mensagens, a de sistema primeiro.
    pub messages: Vec<Message>,
    /// As ferramentas declaradas (fixas por conversa).
    pub tools: Vec<ToolDecl>,
    /// A escolha de ferramenta.
    pub choice: ToolChoice,
    /// O esquema da resposta em JSON (a reserva).
    pub response_schema: Option<Value>,
}

/// Por que o modelo parou.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StopReason {
    /// Terminou o texto.
    Stop,
    /// Pediu ferramentas.
    ToolCalls,
    /// Chegou ao limite de saída.
    Length,
    /// Outro motivo, como a API escreveu.
    Other(String),
}

/// Uma resposta que dá para usar.
#[derive(Debug, Clone, PartialEq)]
pub struct ChatResponse {
    /// A mensagem do assistente (com o campo opaco do provedor).
    pub message: Message,
    /// Por que parou.
    pub stop: StopReason,
    /// Tokens desta requisição.
    pub usage: TokenUsage,
    /// O corpo exato enviado (bytes UTF-8).
    pub sent: String,
}

/// Uma requisição que não deu uma resposta usável, já classificada pelo tradutor. O laço decide
/// as novas tentativas por esta classificação, com os números da ADR-0055.
#[derive(Debug)]
pub enum Failure {
    /// Temporário: o provedor ficou sobrecarregado (503, 500, 529…). Repetir com espera.
    Overloaded {
        /// O código HTTP.
        status: u16,
    },
    /// Temporário: sem conexão ou tempo esgotado. Repetir com espera.
    Network(warden_http::Error),
    /// Limite por minuto: esperar o tempo pedido e repetir.
    PerMinute {
        /// O tempo pedido.
        wait: Duration,
    },
    /// Cota esgotada (diária ou longa): parar, sem prometer horário.
    Quota {
        /// É o plano gratuito (só o Gemini tem).
        free_tier: bool,
    },
    /// Sinal de cobrança: parar.
    Billing {
        /// A mensagem da API, sem a chave.
        detail: String,
    },
    /// Chave recusada: parar.
    KeyInvalid {
        /// O código HTTP.
        status: u16,
    },
    /// Resposta malformada (chamada inválida, sem conteúdo): repetir até 2 vezes sem mexer no
    /// histórico.
    Malformed {
        /// O que estava errado.
        reason: String,
        /// Os tokens que a resposta gastou mesmo assim.
        usage: TokenUsage,
    },
    /// Recusa por outro motivo (4xx).
    Rejected {
        /// O código HTTP.
        status: u16,
        /// A mensagem da API, sem a chave.
        detail: String,
    },
    /// Qualquer outro erro: parar com ele.
    Fatal(Error),
}

impl Failure {
    /// Vale repetir com espera exponencial.
    #[must_use]
    pub fn is_temporary(&self) -> bool {
        matches!(self, Self::Overloaded { .. } | Self::Network(_))
    }

    /// Um erro do cliente HTTP: falta de conexão e tempo esgotado são temporários.
    #[must_use]
    pub fn from_http(error: Error) -> Self {
        match error {
            Error::Http(
                error @ (warden_http::Error::Timeout { .. } | warden_http::Error::Network { .. }),
            ) => Self::Network(error),
            other => Self::Fatal(other),
        }
    }

    /// O erro que vai para a pergunta quando não adianta (ou não dá mais para) repetir.
    #[must_use]
    pub fn into_error(self, model: &str) -> Error {
        match self {
            Self::Overloaded { status } => Error::Unavailable { status },
            Self::Network(error) => Error::Http(error),
            Self::PerMinute { .. } => Error::RateLimited { waits: 0 },
            Self::Quota { free_tier } => Error::QuotaExceeded {
                model: model.to_owned(),
                free_tier,
            },
            Self::Billing { detail } => Error::Billing { detail },
            Self::KeyInvalid { status } => Error::KeyInvalid { status },
            Self::Malformed { reason, .. } => Error::BadResponse { reason },
            Self::Rejected { status, detail } => Error::Rejected { status, detail },
            Self::Fatal(error) => error,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn mensagem_grava_so_o_que_tem() {
        let user = serde_json::to_value(Message::user("oi")).unwrap();
        assert_eq!(user, json!({ "role": "user", "text": "oi" }));
        let assistant = Message::assistant(
            String::new(),
            vec![ToolCall {
                id: Some("c1".into()),
                name: "list_mods".into(),
                args: json!({}),
            }],
            Some(Opaque {
                provider: AiProvider::Gemini,
                value: json!({ "role": "model" }),
            }),
        );
        let value = serde_json::to_value(&assistant).unwrap();
        assert_eq!(value["opaque"]["provider"], "gemini");
        assert_eq!(serde_json::from_value::<Message>(value).unwrap(), assistant);
        assert!(assistant.opaque_of(AiProvider::Gemini).is_some());
        assert!(assistant.opaque_of(AiProvider::Anthropic).is_none());
        assert_eq!(assistant.user_text(), None);
        assert_eq!(Message::user("x").user_text(), Some("x"));
    }

    #[test]
    fn classificacao_comum() {
        assert!(Failure::Overloaded { status: 503 }.is_temporary());
        let timeout = Failure::from_http(Error::Http(warden_http::Error::Timeout {
            host: "x".into(),
            url: "https://x/".into(),
            seconds: 90,
        }));
        assert!(timeout.is_temporary());
        assert!(!Failure::from_http(Error::Cancelled).is_temporary());
        assert!(
            !Failure::Malformed {
                reason: "x".into(),
                usage: TokenUsage::default()
            }
            .is_temporary()
        );
        assert!(matches!(
            Failure::Quota { free_tier: true }.into_error("m"),
            Error::QuotaExceeded {
                free_tier: true,
                ..
            }
        ));
    }
}
