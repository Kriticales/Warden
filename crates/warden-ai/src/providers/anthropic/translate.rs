//! O formato comum ↔ a API Messages da Anthropic (`POST v1/messages`; ADR-0060).
//!
//! - Sistema no topo (`system`); o histórico inteiro em `messages` a cada rodada, sem estado.
//! - A resposta do modelo (`content` inteiro: os blocos de raciocínio `thinking` e
//!   `redacted_thinking` com a assinatura, o texto e os `tool_use`) vai no campo opaco e volta
//!   **intacta**: a API recusa bloco de raciocínio alterado e invalida os que vêm depois de
//!   qualquer mudança no começo da conversa, inclusive em `system` e `tools`. Por isso `system`
//!   e `tools` ficam fixos por conversa e o histórico só cresce.
//! - Resultados: um bloco `tool_result` por chamada, com o `tool_use_id`, todos numa mensagem do
//!   usuário logo depois das chamadas (o texto que vem em seguida entra na mesma mensagem, depois
//!   dos resultados).
//! - Escolha de ferramenta: os modelos que a regra escolhe (Sonnet 5.5 e Opus 5.5) recusam com
//!   400 a escolha forçada (`any` ou `tool`), e trocar `tools` entre as rodadas invalidaria o
//!   raciocínio. Então toda rodada vai com `auto` e as ferramentas de sempre: no Claude não há o
//!   escalonamento por etapa do Gemini (que existe para manter poucas ferramentas ativas; as
//!   permissões do consentimento continuam conferidas pelo laço), e uma resposta em texto cai na
//!   reserva, como no Gemini.
//! - Reserva: `tool_choice: none` e `output_config.format` com o esquema da resposta final
//!   (objetos fechados, como a API pede).
//! - Cache: `cache_control` automático no topo do pedido (o prefixo fixo é reaproveitado).

use serde_json::{Map, Value, json};

use crate::conversation::TokenUsage;
use crate::providers::{
    AiProvider, ChatRequest, ChatResponse, Failure, Message, Opaque, Role, StopReason, ToolCall,
    ToolChoice, ToolDecl, ToolResult,
};

/// Teto de saída por resposta (raciocínio incluído). A API exige um valor; 16 mil cabe numa
/// requisição sem streaming e sobra para a resposta final.
pub const MAX_TOKENS: u64 = 16_000;

/// O bloco `tool_result` de um resultado, com o `tool_use_id` da chamada. O conteúdo é o JSON
/// compacto do resultado.
#[must_use]
pub fn tool_result(result: &ToolResult) -> Value {
    json!({
        "type": "tool_result",
        "tool_use_id": result.id.clone().unwrap_or_default(),
        "content": result.content.to_string(),
    })
}

/// A mensagem da API de uma mensagem comum (`None`: a de sistema, que vai em `system`).
#[must_use]
pub fn message(message: &Message) -> Option<Value> {
    if let Some(opaque) = message.opaque_of(AiProvider::Anthropic) {
        // O `content` da resposta, reenviado intacto (os blocos de raciocínio e a assinatura).
        return Some(json!({ "role": "assistant", "content": opaque }));
    }
    match message.role {
        Role::System => None,
        Role::User => Some(json!({ "role": "user",
            "content": [{ "type": "text", "text": message.text }] })),
        Role::Tool => Some(json!({ "role": "user",
            "content": message.results.iter().map(tool_result).collect::<Vec<_>>() })),
        Role::Assistant => {
            // Só numa conversa que não veio da Anthropic (no app, uma conversa fica no provedor
            // em que começou).
            let mut content = Vec::new();
            if !message.text.is_empty() {
                content.push(json!({ "type": "text", "text": message.text }));
            }
            for call in &message.calls {
                content.push(json!({
                    "type": "tool_use",
                    "id": call.id.clone().unwrap_or_default(),
                    "name": call.name,
                    "input": call.args,
                }));
            }
            Some(json!({ "role": "assistant", "content": content }))
        }
    }
}

/// As mensagens da API, com as mensagens seguidas do mesmo papel juntas numa só (os resultados
/// de ferramenta e o texto do usuário que vem depois deles).
#[must_use]
pub fn messages(all: &[Message]) -> Vec<Value> {
    let mut merged: Vec<Value> = Vec::new();
    for next in all.iter().filter_map(message) {
        if let Some(last) = merged.last_mut()
            && last["role"] == next["role"]
            && let (Some(blocks), Some(more)) =
                (last["content"].as_array_mut(), next["content"].as_array())
        {
            blocks.extend(more.iter().cloned());
            continue;
        }
        merged.push(next);
    }
    merged
}

fn declaration(tool: &ToolDecl) -> Value {
    json!({
        "name": tool.name,
        "description": tool.description,
        "input_schema": tool.parameters,
    })
}

/// O esquema com todo objeto fechado (`additionalProperties: false`), como as saídas
/// estruturadas da API pedem.
#[must_use]
pub fn closed(schema: &Value) -> Value {
    match schema {
        Value::Object(map) => {
            let mut out: Map<String, Value> = map
                .iter()
                .map(|(key, value)| (key.clone(), closed(value)))
                .collect();
            if map.get("type").and_then(Value::as_str) == Some("object") {
                out.insert("additionalProperties".into(), json!(false));
            }
            Value::Object(out)
        }
        Value::Array(list) => Value::Array(list.iter().map(closed).collect()),
        other => other.clone(),
    }
}

/// O que vale para o `messages` e para o `count_tokens`: modelo, sistema, mensagens e
/// ferramentas.
fn prompt(request: &ChatRequest) -> Value {
    let system: Vec<Value> = request
        .messages
        .iter()
        .filter(|message| message.role == Role::System)
        .map(|message| json!({ "type": "text", "text": message.text }))
        .collect();
    let mut body = json!({
        "model": request.model,
        "messages": messages(&request.messages),
    });
    if !system.is_empty() {
        body["system"] = json!(system);
    }
    if !request.tools.is_empty() {
        body["tools"] = json!(request.tools.iter().map(declaration).collect::<Vec<_>>());
    }
    body
}

/// O corpo de um `POST v1/messages`.
#[must_use]
pub fn body(request: &ChatRequest) -> Value {
    let mut body = prompt(request);
    body["max_tokens"] = json!(MAX_TOKENS);
    body["cache_control"] = json!({ "type": "ephemeral" });
    if !request.tools.is_empty() {
        body["tool_choice"] = match request.choice {
            ToolChoice::Required(_) | ToolChoice::Auto => json!({ "type": "auto" }),
            ToolChoice::None => json!({ "type": "none" }),
        };
    }
    if let Some(schema) = &request.response_schema {
        body["output_config"] =
            json!({ "format": { "type": "json_schema", "schema": closed(schema) } });
    }
    body
}

/// O corpo de um `POST v1/messages/count_tokens`: o pedido inteiro (sistema, ferramentas e
/// conversa).
#[must_use]
pub fn count_tokens_body(request: &ChatRequest) -> Value {
    prompt(request)
}

/// Lê o `usage` de uma resposta. A entrada é a soma da parte nova com a lida e a gravada no
/// cache; a API não separa o raciocínio da saída.
#[must_use]
pub fn usage_of(usage: &Value) -> TokenUsage {
    let number = |key: &str| usage.get(key).and_then(Value::as_u64).unwrap_or(0);
    let cached = number("cache_read_input_tokens");
    TokenUsage {
        prompt: number("input_tokens") + cached + number("cache_creation_input_tokens"),
        cached,
        output: number("output_tokens"),
        thoughts: 0,
    }
}

fn malformed(reason: impl Into<String>, usage: TokenUsage) -> Failure {
    Failure::Malformed {
        reason: reason.into(),
        usage,
    }
}

/// Uma resposta 200 já lida como JSON. Sem conteúdo ou com uma chamada sem objeto de entrada
/// vira [`Failure::Malformed`] (o laço repete sem mexer no histórico).
pub fn response(reply: &Value, sent: String) -> Result<ChatResponse, Failure> {
    let usage = reply.get("usage").map(usage_of).unwrap_or_default();
    let content = reply
        .get("content")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let stop_reason = reply
        .get("stop_reason")
        .and_then(Value::as_str)
        .unwrap_or("");
    if content.is_empty() {
        return Err(malformed(
            if stop_reason.is_empty() {
                "resposta sem conteúdo"
            } else {
                stop_reason
            },
            usage,
        ));
    }
    let mut text = String::new();
    let mut calls = Vec::new();
    for block in &content {
        match block.get("type").and_then(Value::as_str) {
            Some("text") => text.push_str(block.get("text").and_then(Value::as_str).unwrap_or("")),
            Some("tool_use") => {
                let name = block.get("name").and_then(Value::as_str).unwrap_or("");
                let Some(input) = block.get("input").filter(|input| input.is_object()) else {
                    return Err(malformed("chamada sem objeto de entrada", usage));
                };
                calls.push(ToolCall {
                    id: block
                        .get("id")
                        .and_then(Value::as_str)
                        .map(ToOwned::to_owned),
                    name: name.to_owned(),
                    args: input.clone(),
                });
            }
            _ => {}
        }
    }
    let stop = match stop_reason {
        "tool_use" | "end_turn" if !calls.is_empty() => StopReason::ToolCalls,
        "end_turn" | "stop_sequence" | "" => StopReason::Stop,
        "max_tokens" => StopReason::Length,
        other => StopReason::Other(other.to_owned()),
    };
    let message = Message::assistant(
        text,
        calls,
        Some(Opaque {
            provider: AiProvider::Anthropic,
            value: Value::Array(content),
        }),
    );
    Ok(ChatResponse {
        message,
        stop,
        usage,
        sent,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(choice: ToolChoice, schema: Option<Value>) -> ChatRequest {
        ChatRequest {
            model: "claude-sonnet-5-5".into(),
            messages: vec![
                Message::system("instruções"),
                Message::user("Por que travou?"),
            ],
            tools: vec![ToolDecl {
                name: "list_mods".into(),
                description: "Lista os mods.".into(),
                parameters: json!({ "type": "object", "properties": {} }),
            }],
            choice,
            response_schema: schema,
        }
    }

    #[test]
    fn corpo_com_sistema_no_topo_e_escolha_automatica() {
        let body = body(&request(
            ToolChoice::Required(vec!["list_mods".into(), "responder".into()]),
            None,
        ));
        assert_eq!(
            body["system"],
            json!([{ "type": "text", "text": "instruções" }])
        );
        assert_eq!(body["messages"][0]["role"], "user");
        assert_eq!(body["tool_choice"], json!({ "type": "auto" }));
        assert_eq!(body["tools"][0]["input_schema"]["type"], "object");
        assert_eq!(body["max_tokens"], MAX_TOKENS);
        assert_eq!(body["cache_control"]["type"], "ephemeral");
        assert!(body.get("output_config").is_none());
        assert!(
            body.get("thinking").is_none(),
            "o raciocínio fica no padrão do modelo"
        );

        let schema = json!({ "type": "object", "properties": { "a": { "type": "array",
            "items": { "type": "object", "properties": {} } } } });
        let reserve = super::body(&request(ToolChoice::None, Some(schema)));
        assert_eq!(reserve["tool_choice"], json!({ "type": "none" }));
        let format = &reserve["output_config"]["format"];
        assert_eq!(format["type"], "json_schema");
        assert_eq!(format["schema"]["additionalProperties"], false);
        assert_eq!(
            format["schema"]["properties"]["a"]["items"]["additionalProperties"],
            false
        );
        assert_eq!(
            reserve["tools"], body["tools"],
            "as ferramentas não mudam na reserva"
        );
        assert!(
            count_tokens_body(&request(ToolChoice::None, None))
                .get("max_tokens")
                .is_none()
        );
    }

    #[test]
    fn resultados_e_texto_seguinte_numa_mensagem_so() {
        let messages = super::messages(&[
            Message::user("1"),
            Message::assistant(
                String::new(),
                vec![ToolCall {
                    id: Some("toolu_1".into()),
                    name: "list_mods".into(),
                    args: json!({}),
                }],
                None,
            ),
            Message::tool_results(vec![ToolResult {
                id: Some("toolu_1".into()),
                name: "list_mods".into(),
                content: json!({ "itens": [] }),
            }]),
            Message::user("2"),
        ]);
        assert_eq!(messages.len(), 3);
        assert_eq!(
            messages[2]["content"],
            json!([
                { "type": "tool_result", "tool_use_id": "toolu_1", "content": "{\"itens\":[]}" },
                { "type": "text", "text": "2" }
            ])
        );
    }

    #[test]
    fn resposta_guarda_o_raciocinio_intacto() {
        let content = json!([
            { "type": "thinking", "thinking": "", "signature": "RXN0ZSDDqSBvIHJhY2lvY8OtbmlvLg==" },
            { "type": "text", "text": "Vou olhar o crash report." },
            { "type": "tool_use", "id": "toolu_1", "name": "get_crash_report", "input": { "session_id": "s3" } },
            { "type": "redacted_thinking", "data": "b3BhY28=" }
        ]);
        let reply = json!({ "content": content, "stop_reason": "tool_use", "usage": {
            "input_tokens": 50, "cache_read_input_tokens": 3000, "cache_creation_input_tokens": 200, "output_tokens": 90 } });
        let parsed = response(&reply, String::new()).unwrap();
        assert_eq!(parsed.stop, StopReason::ToolCalls);
        assert_eq!(parsed.message.text, "Vou olhar o crash report.");
        assert_eq!(parsed.message.calls[0].id.as_deref(), Some("toolu_1"));
        assert_eq!(
            parsed.usage,
            TokenUsage {
                prompt: 3250,
                cached: 3000,
                output: 90,
                thoughts: 0
            }
        );
        assert_eq!(
            message(&parsed.message),
            Some(json!({ "role": "assistant", "content": content })),
            "o content volta como veio"
        );
    }

    #[test]
    fn sem_conteudo_ou_sem_entrada_vira_nova_tentativa() {
        // Fora da primeira etapa do Gemini: no Claude, todas as ferramentas valem desde o começo.
        let early = json!({ "content": [{ "type": "tool_use", "id": "t", "name": "read_config",
            "input": { "path": "x" } }], "stop_reason": "tool_use", "usage": { "input_tokens": 7 } });
        assert_eq!(
            response(&early, String::new()).unwrap().message.calls[0].name,
            "read_config"
        );
        match response(
            &json!({ "content": [], "stop_reason": "end_turn", "usage": { "input_tokens": 7 } }),
            String::new(),
        ) {
            Err(Failure::Malformed { reason, usage }) => {
                assert_eq!(reason, "end_turn");
                assert_eq!(usage.prompt, 7);
            }
            other => panic!("{other:?}"),
        }
        assert!(matches!(
            response(&json!({}), String::new()),
            Err(Failure::Malformed { reason, .. }) if reason == "resposta sem conteúdo"
        ));
        let no_input =
            json!({ "content": [{ "type": "tool_use", "id": "t", "name": "list_mods" }] });
        assert!(matches!(
            response(&no_input, String::new()),
            Err(Failure::Malformed { .. })
        ));
    }
}
