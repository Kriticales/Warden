//! O formato comum ↔ a Responses API da OpenAI (`POST v1/responses`; ADR-0060).
//!
//! - **Sem estado:** `store: false` (nada guardado na OpenAI) e o histórico inteiro em `input` a
//!   cada rodada. Os itens que o modelo devolve (`reasoning` com o `encrypted_content`, `message`
//!   e `function_call`) vão inteiros no campo opaco e voltam **intactos** em `input`, como a
//!   documentação exige para os modelos de raciocínio com ferramentas.
//! - Ferramentas: `tools` fixo por conversa (`type: function`); entre as rodadas muda só o
//!   `tool_choice`: `allowed_tools` com `mode: required` e os nomes liberados, `auto` ou `none`.
//! - Resultados: um `function_call_output` por chamada, com o `call_id` dela, todos juntos depois
//!   das chamadas da rodada.
//! - Reserva: `tool_choice: none` e `text.format` com o esquema da resposta final.

use serde_json::{Value, json};

use crate::conversation::TokenUsage;
use crate::providers::{
    AiProvider, ChatRequest, ChatResponse, Failure, Message, Opaque, Role, StopReason, ToolCall,
    ToolChoice, ToolDecl, ToolResult,
};

/// O nome do esquema da reserva (`text.format.name`).
const ANSWER_FORMAT: &str = "resposta_final";

/// O item `function_call_output` de um resultado, com o `call_id` da chamada. O `output` é o
/// conteúdo em JSON compacto (a API pede texto).
#[must_use]
pub fn function_call_output(result: &ToolResult) -> Value {
    json!({
        "type": "function_call_output",
        "call_id": result.id.clone().unwrap_or_default(),
        "output": result.content.to_string(),
    })
}

/// Os itens de `input` de uma mensagem (a de sistema vai em `instructions`).
#[must_use]
pub fn items(message: &Message) -> Vec<Value> {
    if let Some(opaque) = message.opaque_of(AiProvider::Openai) {
        // Reenviados intactos, nunca reconstruídos (o raciocínio criptografado).
        return opaque.as_array().cloned().unwrap_or_default();
    }
    match message.role {
        Role::System => Vec::new(),
        Role::User => vec![json!({ "type": "message", "role": "user", "content": message.text })],
        Role::Tool => message.results.iter().map(function_call_output).collect(),
        Role::Assistant => {
            // Só numa conversa que não veio da OpenAI (no app, uma conversa fica no provedor em
            // que começou).
            let mut items = Vec::new();
            if !message.text.is_empty() {
                items.push(json!({ "type": "message", "role": "assistant",
                    "content": [{ "type": "output_text", "text": message.text }] }));
            }
            for call in &message.calls {
                items.push(json!({
                    "type": "function_call",
                    "call_id": call.id.clone().unwrap_or_default(),
                    "name": call.name,
                    "arguments": call.args.to_string(),
                }));
            }
            items
        }
    }
}

fn declaration(tool: &ToolDecl) -> Value {
    json!({
        "type": "function",
        "name": tool.name,
        "description": tool.description,
        "parameters": tool.parameters,
        "strict": false,
    })
}

/// O corpo de um `POST v1/responses`.
#[must_use]
pub fn body(request: &ChatRequest) -> Value {
    let instructions: Vec<&str> = request
        .messages
        .iter()
        .filter(|message| message.role == Role::System)
        .map(|message| message.text.as_str())
        .collect();
    let input: Vec<Value> = request.messages.iter().flat_map(items).collect();
    let mut body = json!({
        "model": request.model,
        "input": input,
        "store": false,
        "include": ["reasoning.encrypted_content"],
    });
    if !instructions.is_empty() {
        body["instructions"] = json!(instructions.join("\n\n"));
    }
    if !request.tools.is_empty() {
        body["tools"] = json!(request.tools.iter().map(declaration).collect::<Vec<_>>());
        body["parallel_tool_calls"] = json!(true);
        body["tool_choice"] = match &request.choice {
            ToolChoice::Required(names) => json!({
                "type": "allowed_tools",
                "mode": "required",
                "tools": names.iter().map(|name| json!({ "type": "function", "name": name })).collect::<Vec<_>>(),
            }),
            ToolChoice::Auto => json!("auto"),
            ToolChoice::None => json!("none"),
        };
    }
    if let Some(schema) = &request.response_schema {
        body["text"] = json!({ "format": {
            "type": "json_schema",
            "name": ANSWER_FORMAT,
            "schema": schema,
            // O esquema tem campos opcionais (`modId`), que o modo estrito não aceita.
            "strict": false,
        }});
    }
    body
}

/// Lê o `usage` de uma resposta. O raciocínio vem dentro de `output_tokens`: aqui ele sai da
/// saída e vai para `thoughts`, como no Gemini.
#[must_use]
pub fn usage_of(usage: &Value) -> TokenUsage {
    let number = |path: &str| usage.pointer(path).and_then(Value::as_u64).unwrap_or(0);
    let reasoning = number("/output_tokens_details/reasoning_tokens");
    TokenUsage {
        prompt: number("/input_tokens"),
        cached: number("/input_tokens_details/cached_tokens"),
        output: number("/output_tokens").saturating_sub(reasoning),
        thoughts: reasoning,
    }
}

fn malformed(reason: impl Into<String>, usage: TokenUsage) -> Failure {
    Failure::Malformed {
        reason: reason.into(),
        usage,
    }
}

/// Uma resposta 200 já lida como JSON. Sem itens, com uma chamada de argumentos que não são um
/// objeto JSON ou `status: failed` vira [`Failure::Malformed`] (o laço repete sem mexer no
/// histórico).
pub fn response(reply: &Value, sent: String) -> Result<ChatResponse, Failure> {
    let usage = reply.get("usage").map(usage_of).unwrap_or_default();
    let status = reply.get("status").and_then(Value::as_str).unwrap_or("");
    if status == "failed" {
        let reason = reply
            .pointer("/error/message")
            .and_then(Value::as_str)
            .unwrap_or("status failed");
        return Err(malformed(reason, usage));
    }
    let output = reply
        .get("output")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if output.is_empty() {
        return Err(malformed("resposta sem conteúdo", usage));
    }
    let mut text = String::new();
    let mut calls = Vec::new();
    for item in &output {
        match item.get("type").and_then(Value::as_str) {
            Some("message") => {
                let parts = item
                    .get("content")
                    .and_then(Value::as_array)
                    .map_or(&[][..], Vec::as_slice);
                for part in parts {
                    if part.get("type").and_then(Value::as_str) == Some("output_text")
                        && let Some(piece) = part.get("text").and_then(Value::as_str)
                    {
                        text.push_str(piece);
                    }
                }
            }
            Some("function_call") => {
                let raw = item.get("arguments").and_then(Value::as_str).unwrap_or("");
                let args = if raw.trim().is_empty() {
                    json!({})
                } else {
                    match serde_json::from_str::<Value>(raw) {
                        Ok(args) if args.is_object() => args,
                        _ => return Err(malformed("chamada com argumentos inválidos", usage)),
                    }
                };
                calls.push(ToolCall {
                    id: item
                        .get("call_id")
                        .and_then(Value::as_str)
                        .map(ToOwned::to_owned),
                    name: item
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_owned(),
                    args,
                });
            }
            _ => {}
        }
    }
    let incomplete = reply
        .pointer("/incomplete_details/reason")
        .and_then(Value::as_str);
    let stop = match (status, incomplete) {
        ("incomplete", Some("max_output_tokens")) => StopReason::Length,
        ("incomplete", Some(other)) => StopReason::Other(other.to_owned()),
        _ if !calls.is_empty() => StopReason::ToolCalls,
        _ => StopReason::Stop,
    };
    let message = Message::assistant(
        text,
        calls,
        Some(Opaque {
            provider: AiProvider::Openai,
            value: Value::Array(output),
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
            model: "gpt-6-luna".into(),
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
    fn corpo_sem_estado_com_a_escolha_entre_nomes() {
        let body = body(&request(
            ToolChoice::Required(vec!["list_mods".into(), "responder".into()]),
            None,
        ));
        assert_eq!(body["store"], false);
        assert_eq!(body["include"], json!(["reasoning.encrypted_content"]));
        assert_eq!(body["instructions"], "instruções");
        assert_eq!(
            body["input"],
            json!([{ "type": "message", "role": "user", "content": "Por que travou?" }])
        );
        assert_eq!(body["tool_choice"]["type"], "allowed_tools");
        assert_eq!(body["tool_choice"]["mode"], "required");
        assert_eq!(body["tool_choice"]["tools"][1]["name"], "responder");
        assert_eq!(body["tools"][0]["parameters"]["type"], "object");
        assert!(body.get("text").is_none());

        let reserve = super::body(&request(
            ToolChoice::None,
            Some(json!({ "type": "object" })),
        ));
        assert_eq!(reserve["tool_choice"], "none");
        assert_eq!(reserve["text"]["format"]["type"], "json_schema");
        assert_eq!(
            reserve["text"]["format"]["schema"],
            json!({ "type": "object" })
        );
    }

    #[test]
    fn resposta_guarda_os_itens_intactos_e_le_o_uso() {
        let output = json!([
            { "type": "reasoning", "id": "rs_1", "summary": [], "encrypted_content": "Z2VoZWlt" },
            { "type": "function_call", "id": "fc_1", "call_id": "call_a", "name": "list_mods",
              "arguments": "{\"filtro\":\"create\"}", "status": "completed" },
            { "type": "function_call", "id": "fc_2", "call_id": "call_b", "name": "get_findings",
              "arguments": "", "status": "completed" }
        ]);
        let reply = json!({ "status": "completed", "output": output, "usage": {
            "input_tokens": 3000, "input_tokens_details": { "cached_tokens": 2048 },
            "output_tokens": 140, "output_tokens_details": { "reasoning_tokens": 100 } } });
        let parsed = response(&reply, "{}".into()).unwrap();
        assert_eq!(parsed.stop, StopReason::ToolCalls);
        assert_eq!(parsed.message.calls[0].args, json!({ "filtro": "create" }));
        assert_eq!(parsed.message.calls[1].args, json!({}));
        assert_eq!(parsed.message.calls[1].id.as_deref(), Some("call_b"));
        assert_eq!(
            parsed.usage,
            TokenUsage {
                prompt: 3000,
                cached: 2048,
                output: 40,
                thoughts: 100
            }
        );
        assert_eq!(items(&parsed.message), output.as_array().unwrap().clone());
    }

    #[test]
    fn resposta_com_problema_vira_nova_tentativa() {
        let bad = json!({ "status": "completed", "output": [
            { "type": "function_call", "call_id": "c", "name": "list_mods", "arguments": "{\"filtro\":" }
        ], "usage": { "input_tokens": 10 } });
        match response(&bad, String::new()) {
            Err(Failure::Malformed { usage, .. }) => assert_eq!(usage.prompt, 10),
            other => panic!("{other:?}"),
        }
        assert!(matches!(
            response(&json!({ "status": "completed", "output": [] }), String::new()),
            Err(Failure::Malformed { reason, .. }) if reason == "resposta sem conteúdo"
        ));
        assert!(matches!(
            response(
                &json!({ "status": "failed", "error": { "message": "x" } }),
                String::new()
            ),
            Err(Failure::Malformed { .. })
        ));
        let cut = json!({ "status": "incomplete", "incomplete_details": { "reason": "max_output_tokens" },
            "output": [{ "type": "message", "role": "assistant", "content": [{ "type": "output_text", "text": "{\"resumo\"" }] }] });
        let parsed = response(&cut, String::new()).unwrap();
        assert_eq!(parsed.stop, StopReason::Length);
        assert_eq!(parsed.message.text, "{\"resumo\"");
    }

    #[test]
    fn resultados_com_o_call_id() {
        let message = Message::tool_results(vec![ToolResult {
            id: Some("call_a".into()),
            name: "list_mods".into(),
            content: json!({ "itens": [] }),
        }]);
        assert_eq!(
            items(&message),
            vec![
                json!({ "type": "function_call_output", "call_id": "call_a", "output": "{\"itens\":[]}" })
            ]
        );
    }
}
