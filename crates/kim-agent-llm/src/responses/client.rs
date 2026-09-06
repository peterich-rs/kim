//! Live OpenAI Responses API client (SSE streaming).

use crate::responses::parse::parse_sse_chunk;
use crate::{LlmClient, LlmError, LlmResult, ResponseInputItem, ResponseRequest};
use async_trait::async_trait;
use futures::stream::{self, BoxStream, StreamExt};
use kim_agent_types::{Context, StreamEvent, ToolChoice};
use reqwest::Client;
use serde_json::{json, Value};
use std::time::Duration;

/// HTTP Responses client. Config is owned per instance so provider changes
/// rebuild the client without touching SQLite session trees.
#[derive(Clone)]
pub struct OpenAiResponsesClient {
    http: Client,
    base_url: String,
    api_key: String,
}

impl OpenAiResponsesClient {
    pub fn new(base_url: impl Into<String>, api_key: impl Into<String>) -> Self {
        let http = Client::builder()
            .timeout(Duration::from_secs(120))
            .build()
            .unwrap_or_else(|_| Client::new());
        Self {
            http,
            base_url: base_url.into().trim_end_matches('/').to_string(),
            api_key: api_key.into(),
        }
    }

    fn endpoint(&self) -> String {
        if self.base_url.ends_with("/responses") {
            self.base_url.clone()
        } else {
            format!("{}/responses", self.base_url)
        }
    }

    fn body(req: &ResponseRequest) -> Value {
        let mut input = Vec::new();
        for item in &req.input {
            match item {
                ResponseInputItem::Message { role, content } => {
                    let parts: Vec<Value> = content
                        .iter()
                        .map(|p| match p {
                            crate::ContentPart::InputText { text } => {
                                json!({"type": "input_text", "text": text})
                            }
                            crate::ContentPart::OutputText { text } => {
                                json!({"type": "output_text", "text": text})
                            }
                        })
                        .collect();
                    input.push(json!({"type": "message", "role": role, "content": parts}));
                }
                ResponseInputItem::FunctionCall {
                    call_id,
                    name,
                    arguments,
                } => {
                    input.push(json!({
                        "type": "function_call",
                        "call_id": call_id,
                        "name": name,
                        "arguments": arguments,
                    }));
                }
                ResponseInputItem::FunctionCallOutput { call_id, output } => {
                    input.push(json!({
                        "type": "function_call_output",
                        "call_id": call_id,
                        "output": output,
                    }));
                }
            }
        }

        let tools: Vec<Value> = req
            .tools
            .iter()
            .map(|t| {
                json!({
                    "type": "function",
                    "name": t.name,
                    "description": t.description,
                    "parameters": t.parameters,
                })
            })
            .collect();

        let tool_choice = match &req.tool_choice {
            ToolChoice::Auto => Value::String("auto".into()),
            ToolChoice::None => Value::String("none".into()),
            ToolChoice::Required => Value::String("required".into()),
            ToolChoice::Specific(name) => json!({"type":"function","name": name}),
        };

        let mut body = json!({
            "model": req.model.id,
            "input": input,
            "stream": req.stream,
            "parallel_tool_calls": req.parallel_tool_calls,
            "tool_choice": tool_choice,
        });
        if !tools.is_empty() {
            body["tools"] = Value::Array(tools);
        }
        if let Some(prev) = &req.previous_response_id {
            body["previous_response_id"] = Value::String(prev.clone());
        }
        if let Some(max) = req.max_output_tokens {
            body["max_output_tokens"] = json!(max);
        }
        if let Some(instr) = &req.instructions {
            body["instructions"] = Value::String(instr.clone());
        }
        body
    }
}

#[async_trait]
impl LlmClient for OpenAiResponsesClient {
    async fn stream(
        &self,
        req: ResponseRequest,
        cx: &Context,
    ) -> LlmResult<BoxStream<'static, LlmResult<StreamEvent>>> {
        if self.api_key.trim().is_empty() {
            return Err(LlmError::Failed("api_key missing for Live mode".into()));
        }
        let cancelled = cx.abort.clone();
        let response = self
            .http
            .post(self.endpoint())
            .bearer_auth(&self.api_key)
            .header("Content-Type", "application/json")
            .header("Accept", "text/event-stream")
            .json(&Self::body(&req))
            .send()
            .await
            .map_err(|e| LlmError::Failed(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(LlmError::Failed(format!("HTTP {status}: {text}")));
        }

        let byte_stream = response.bytes_stream();
        let s = stream::unfold(
            (byte_stream, String::new(), cancelled, false),
            |(mut bytes, mut buf, cancelled, done)| async move {
                if done {
                    return None;
                }
                if cancelled.is_cancelled() {
                    return Some((Err(LlmError::Aborted), (bytes, buf, cancelled, true)));
                }
                loop {
                    if let Some(idx) = buf.find("\n\n") {
                        let chunk = buf[..idx].to_string();
                        buf = buf[idx + 2..].to_string();
                        match parse_sse_chunk(&chunk) {
                            Ok(Some(ev)) => {
                                return Some((Ok(ev), (bytes, buf, cancelled, false)));
                            }
                            Ok(None) => continue,
                            Err(e) => {
                                return Some((
                                    Err(LlmError::Parse(e)),
                                    (bytes, buf, cancelled, false),
                                ));
                            }
                        }
                    }
                    match bytes.next().await {
                        Some(Ok(chunk)) => {
                            buf.push_str(&String::from_utf8_lossy(&chunk));
                            if cancelled.is_cancelled() {
                                return Some((
                                    Err(LlmError::Aborted),
                                    (bytes, buf, cancelled, true),
                                ));
                            }
                        }
                        Some(Err(e)) => {
                            return Some((
                                Err(LlmError::Failed(e.to_string())),
                                (bytes, buf, cancelled, true),
                            ));
                        }
                        None => {
                            if !buf.trim().is_empty() {
                                match parse_sse_chunk(&buf) {
                                    Ok(Some(ev)) => {
                                        return Some((
                                            Ok(ev),
                                            (bytes, String::new(), cancelled, true),
                                        ));
                                    }
                                    Ok(None) => {}
                                    Err(e) => {
                                        return Some((
                                            Err(LlmError::Parse(e)),
                                            (bytes, String::new(), cancelled, true),
                                        ));
                                    }
                                }
                            }
                            return None;
                        }
                    }
                }
            },
        );
        Ok(Box::pin(s))
    }
}
