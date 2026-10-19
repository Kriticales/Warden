//! O formato comum ↔ o `generateContent` do Gemini (ARCHITECTURE §9.5 "Laço").
//!
//! O corpo que sai é **o mesmo, byte a byte**, que o laço da D-04 montava para a mesma conversa
//! (`tests/corpo_identico.rs`, com os corpos gravados antes da D-15): as mesmas chaves, na mesma
//! ordem de inserção, o `content` de cada resposta do modelo reenviado intacto pelo campo opaco
//! (as *thought signatures*), todas as `functionResponse` de uma rodada numa mensagem, com o
//! `id` da chamada, temperatura 1.0 e `NONE` com `responseFormat` na reserva. A única diferença
//! desde a D-15b: as rodadas de ferramenta usam `ANY` com `allowedFunctionNames` (antes,
//! `VALIDATED`), que obriga uma chamada; com a `responder` sempre na lista, a resposta final sai
//! por ela, e não em texto (o Flash respondia em texto com `VALIDATED`).

use serde_json::{Value, json};

use crate::conversation::TokenUsage;
use crate::providers::{
    AiProvider, ChatRequest, ChatResponse, Failure, Message, Opaque, Role, StopReason, ToolCall,
    ToolChoice, ToolDecl, ToolResult,
};

/// Temperatura recomendada pelo Google para o Gemini 3.
const TEMPERATURE: f64 = 1.0;

/// As partes de um `content`.
fn parts(content: &Value) -> &[Value] {
    content
        .get("parts")
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}

/// O texto das partes que não são pensamento.
pub(super) fn visible_text(content: &Value) -> String {
    parts(content)
        .iter()
        .filter(|part| {
            !part
                .get("thought")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        })
        .filter_map(|part| part.get("text").and_then(Value::as_str))
        .collect()
}

/// As chamadas (`functionCall`) de um `content` do modelo.
fn calls_of(content: &Value) -> Vec<ToolCall> {
    parts(content)
        .iter()
        .filter_map(|part| part.get("functionCall"))
        .map(|call| ToolCall {
            id: call
                .get("id")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
            name: call
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned(),
            args: call.get("args").cloned().unwrap_or_else(|| json!({})),
        })
        .collect()
}

/// A parte `functionResponse` de um resultado, com o `id` da chamada.
#[must_use]
pub fn function_response(result: &ToolResult) -> Value {
    let mut inner = json!({
        "name": result.name,
        "response": result.content,
    });
    if let Some(id) = &result.id {
        inner["id"] = json!(id);
    }
    json!({ "functionResponse": inner })
}

/// O `content` de uma mensagem (`None`: a de sistema, que vai em `systemInstruction`).
#[must_use]
pub fn content(message: &Message) -> Option<Value> {
    if let Some(opaque) = message.opaque_of(AiProvider::Gemini) {
        // Reenviado intacto, nunca reconstruído (CA-T14-14).
        return Some(opaque.clone());
    }
    match message.role {
        Role::System => None,
        Role::User => Some(json!({ "role": "user", "parts": [{ "text": message.text }] })),
        Role::Tool => {
            let responses: Vec<Value> = message.results.iter().map(function_response).collect();
            Some(json!({ "role": "user", "parts": responses }))
        }
        Role::Assistant => {
            // Só numa conversa que não veio do Gemini (uma conversa fica no provedor em que
            // começou, então isto não acontece no app).
            let mut parts = Vec::new();
            if !message.text.is_empty() {
                parts.push(json!({ "text": message.text }));
            }
            for call in &message.calls {
                let mut inner = json!({ "name": call.name, "args": call.args });
                if let Some(id) = &call.id {
                    inner["id"] = json!(id);
                }
                parts.push(json!({ "functionCall": inner }));
            }
            Some(json!({ "role": "model", "parts": parts }))
        }
    }
}

fn declaration(tool: &ToolDecl) -> Value {
    json!({
        "name": tool.name,
        "description": tool.description,
        "parametersJsonSchema": tool.parameters,
    })
}

/// O corpo de um `generateContent`.
#[must_use]
pub fn body(request: &ChatRequest) -> Value {
    let system: Vec<Value> = request
        .messages
        .iter()
        .filter(|message| message.role == Role::System)
        .map(|message| json!({ "text": message.text }))
        .collect();
    let contents: Vec<Value> = request.messages.iter().filter_map(content).collect();
    let declarations: Vec<Value> = request.tools.iter().map(declaration).collect();
    let mut generation = json!({ "temperature": TEMPERATURE });
    if let Some(schema) = &request.response_schema {
        generation["responseFormat"] = json!({
            "text": { "mimeType": "APPLICATION_JSON", "schema": schema }
        });
    }
    let function_calling = match &request.choice {
        ToolChoice::Required(names) => json!({
            "mode": "ANY",
            "allowedFunctionNames": names,
        }),
        ToolChoice::Auto => json!({ "mode": "AUTO" }),
        ToolChoice::None => json!({ "mode": "NONE" }),
    };
    let mut body = json!({
        "systemInstruction": { "parts": system },
        "contents": contents,
        "tools": [{ "functionDeclarations": declarations }],
        "toolConfig": { "functionCallingConfig": function_calling },
        "generationConfig": generation,
    });
    if let Some(object) = body.as_object_mut() {
        if system.is_empty() {
            object.remove("systemInstruction");
        }
        if request.tools.is_empty() {
            object.remove("tools");
            object.remove("toolConfig");
        }
    }
    body
}

/// O corpo de um `countTokens`: o pedido inteiro (sistema, ferramentas e conversa) em
/// `generateContentRequest`.
#[must_use]
pub fn count_tokens_body(request: &ChatRequest) -> Value {
    let mut inner = body(request);
    inner["model"] = json!(format!("models/{}", request.model));
    json!({ "generateContentRequest": inner })
}

/// Lê o `usageMetadata` de uma resposta.
#[must_use]
pub fn usage_of(metadata: &Value) -> TokenUsage {
    let number = |key: &str| metadata.get(key).and_then(Value::as_u64).unwrap_or(0);
    TokenUsage {
        prompt: number("promptTokenCount") + number("toolUsePromptTokenCount"),
        cached: number("cachedContentTokenCount"),
        output: number("candidatesTokenCount"),
        thoughts: number("thoughtsTokenCount"),
    }
}

/// Uma resposta 200 já lida como JSON. `MALFORMED_FUNCTION_CALL`, `UNEXPECTED_TOOL_CALL`,
/// `MALFORMED_RESPONSE` e a resposta sem `content` viram [`Failure::Malformed`] (o laço repete
/// sem mexer no histórico, S-R5-4 §13).
pub fn response(reply: &Value, sent: String) -> Result<ChatResponse, Failure> {
    let usage = reply.get("usageMetadata").map(usage_of).unwrap_or_default();
    let candidate = reply
        .pointer("/candidates/0")
        .cloned()
        .unwrap_or(Value::Null);
    let finish = candidate
        .get("finishReason")
        .and_then(Value::as_str)
        .unwrap_or("");
    let bad_finish = matches!(
        finish,
        "MALFORMED_FUNCTION_CALL" | "UNEXPECTED_TOOL_CALL" | "MALFORMED_RESPONSE"
    );
    let Some(content) = candidate.get("content").cloned().filter(|_| !bad_finish) else {
        return Err(Failure::Malformed {
            reason: if finish.is_empty() {
                "resposta sem conteúdo".into()
            } else {
                finish.to_owned()
            },
            usage,
        });
    };
    let calls = calls_of(&content);
    let stop = match finish {
        "STOP" if !calls.is_empty() => StopReason::ToolCalls,
        "STOP" | "" => StopReason::Stop,
        "MAX_TOKENS" => StopReason::Length,
        other => StopReason::Other(other.to_owned()),
    };
    let message = Message::assistant(
        visible_text(&content),
        calls,
        Some(Opaque {
            provider: AiProvider::Gemini,
            value: content,
        }),
    );
    Ok(ChatResponse {
        message,
        stop,
        usage,
        sent,
    })
}

/// Converte o histórico do formato 1 das conversas (o `contents` da API, D-04) para o formato
/// comum, sem perda: a resposta do modelo vai inteira no campo opaco, e qualquer mensagem que
/// não voltaria **byte a byte** igual também guarda o original no campo opaco.
#[must_use]
pub fn messages_from_contents(contents: Vec<Value>) -> Vec<Message> {
    contents
        .into_iter()
        .map(|content| {
            let role = content.get("role").and_then(Value::as_str).unwrap_or("");
            let all = parts(&content);
            let message = if role == "model" {
                Message::assistant(visible_text(&content), calls_of(&content), None)
            } else if !all.is_empty()
                && all
                    .iter()
                    .all(|part| part.get("functionResponse").is_some())
            {
                Message::tool_results(
                    all.iter()
                        .filter_map(|part| part.get("functionResponse"))
                        .map(|inner| ToolResult {
                            id: inner
                                .get("id")
                                .and_then(Value::as_str)
                                .map(ToOwned::to_owned),
                            name: inner
                                .get("name")
                                .and_then(Value::as_str)
                                .unwrap_or("")
                                .to_owned(),
                            content: inner.get("response").cloned().unwrap_or(Value::Null),
                        })
                        .collect(),
                )
            } else {
                Message::user(
                    all.iter()
                        .find_map(|part| part.get("text").and_then(Value::as_str))
                        .unwrap_or(""),
                )
            };
            let same = role != "model"
                && self::content(&message).is_some_and(|rebuilt| {
                    serde_json::to_string(&rebuilt).ok() == serde_json::to_string(&content).ok()
                });
            if same {
                message
            } else {
                Message {
                    opaque: Some(Opaque {
                        provider: AiProvider::Gemini,
                        value: content,
                    }),
                    ..message
                }
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resposta_sem_conteudo_ou_malformada_vira_nova_tentativa() {
        let malformed = json!({ "candidates": [{ "finishReason": "MALFORMED_FUNCTION_CALL",
            "content": { "role": "model", "parts": [] } }], "usageMetadata": { "promptTokenCount": 10 } });
        match response(&malformed, String::new()) {
            Err(Failure::Malformed { reason, usage }) => {
                assert_eq!(reason, "MALFORMED_FUNCTION_CALL");
                assert_eq!(usage.prompt, 10);
            }
            other => panic!("{other:?}"),
        }
        let empty = json!({ "candidates": [{}] });
        assert!(matches!(
            response(&empty, String::new()),
            Err(Failure::Malformed { reason, .. }) if reason == "resposta sem conteúdo"
        ));
    }

    #[test]
    fn resposta_com_chamadas_guarda_o_content_intacto() {
        let content = json!({ "role": "model", "parts": [
            { "text": "pensando", "thought": true },
            { "functionCall": { "name": "list_mods", "args": { "filter": "x" }, "id": "c1" }, "thoughtSignature": "c2ln" },
            { "functionCall": { "name": "get_findings" } }
        ]});
        let reply = json!({ "candidates": [{ "content": content, "finishReason": "STOP" }] });
        let parsed = response(&reply, "{}".into()).unwrap();
        assert_eq!(parsed.stop, StopReason::ToolCalls);
        assert_eq!(parsed.message.text, "");
        assert_eq!(parsed.message.calls.len(), 2);
        assert_eq!(parsed.message.calls[0].id.as_deref(), Some("c1"));
        assert_eq!(parsed.message.calls[1].args, json!({}));
        assert_eq!(
            super::content(&parsed.message),
            Some(content),
            "o content volta como veio"
        );
    }

    #[test]
    fn conversao_do_formato_1_sem_perda() {
        let contents = vec![
            json!({ "role": "user", "parts": [{ "text": "Trava ao entrar no mundo." }] }),
            json!({ "role": "model", "parts": [{ "functionCall": { "name": "get_findings", "args": {}, "id": "a" }, "thoughtSignature": "s" }] }),
            json!({ "role": "user", "parts": [{ "functionResponse": { "name": "get_findings", "response": { "itens": [] }, "id": "a" } }] }),
            // Uma forma que o Warden não escreve: fica guardada no campo opaco.
            json!({ "role": "user", "parts": [{ "text": "a" }, { "text": "b" }] }),
        ];
        let messages = messages_from_contents(contents.clone());
        assert_eq!(messages[0], Message::user("Trava ao entrar no mundo."));
        assert_eq!(messages[1].calls[0].name, "get_findings");
        assert!(messages[1].opaque.is_some());
        assert_eq!(messages[2].role, Role::Tool);
        assert!(messages[2].opaque.is_none());
        assert!(messages[3].opaque.is_some());
        let back: Vec<Value> = messages.iter().filter_map(content).collect();
        assert_eq!(back, contents);
    }
}
