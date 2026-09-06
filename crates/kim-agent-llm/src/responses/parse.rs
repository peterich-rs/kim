//! Parse OpenAI Responses API SSE into internal StreamEvents.

use kim_agent_types::StreamEvent;
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ResponsesParseError {
    #[error("invalid sse: {0}")]
    Invalid(String),
    #[error("json: {0}")]
    Json(String),
}

/// Parse a single SSE event block (may contain `event:` / `data:` lines).
pub fn parse_sse_chunk(chunk: &str) -> Result<Option<StreamEvent>, ResponsesParseError> {
    let mut data_lines: Vec<&str> = Vec::new();
    for line in chunk.lines() {
        if let Some(rest) = line.strip_prefix("data:") {
            data_lines.push(rest.trim_start());
        } else if line.starts_with(':') || line.starts_with("event:") || line.is_empty() {
            continue;
        }
    }
    if data_lines.is_empty() {
        return Ok(None);
    }
    let data = data_lines.join("\n");
    if data.trim() == "[DONE]" {
        return Ok(None);
    }
    let value: Value =
        serde_json::from_str(&data).map_err(|e| ResponsesParseError::Json(e.to_string()))?;
    Ok(map_responses_event(&value))
}

/// Split a full SSE body into events and map each.
pub fn parse_sse_stream(body: &str) -> Result<Vec<StreamEvent>, ResponsesParseError> {
    let mut out = Vec::new();
    for block in split_sse_blocks(body) {
        if let Some(ev) = parse_sse_chunk(&block)? {
            out.push(ev);
        }
    }
    Ok(out)
}

fn split_sse_blocks(body: &str) -> Vec<String> {
    let normalized = body.replace("\r\n", "\n");
    normalized
        .split("\n\n")
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

fn map_responses_event(v: &Value) -> Option<StreamEvent> {
    let ty = v.get("type")?.as_str()?;
    match ty {
        "response.created" => {
            let id = v
                .pointer("/response/id")
                .and_then(|x| x.as_str())
                .or_else(|| v.get("id").and_then(|x| x.as_str()))
                .unwrap_or("")
                .to_string();
            Some(StreamEvent::ResponseStarted {
                provider_response_id: id,
            })
        }
        "response.in_progress" => None,
        "response.completed" => Some(StreamEvent::Completed),
        "response.failed" | "error" => {
            let message = v
                .pointer("/response/error/message")
                .and_then(|x| x.as_str())
                .or_else(|| v.get("message").and_then(|x| x.as_str()))
                .or_else(|| v.pointer("/error/message").and_then(|x| x.as_str()))
                .unwrap_or("provider error")
                .to_string();
            Some(StreamEvent::Failed { message })
        }
        "response.output_text.delta" => {
            let delta = v
                .get("delta")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            let output_index = v.get("output_index").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
            Some(StreamEvent::TextDelta {
                output_index,
                delta,
            })
        }
        "response.output_text.done"
        | "response.content_part.added"
        | "response.content_part.done" => None,
        "response.output_item.added" => map_output_item_added(v),
        "response.output_item.done" => map_output_item_done(v),
        "response.function_call_arguments.delta" => {
            let delta = v
                .get("delta")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            let output_index = v.get("output_index").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
            let call_id = extract_call_id(v).unwrap_or_default();
            Some(StreamEvent::ToolCallArgsDelta {
                output_index,
                call_id,
                delta,
            })
        }
        "response.function_call_arguments.done" => {
            // Prefer finished via output_item.done; still emit ToolCallFinished if args present.
            let output_index = v.get("output_index").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
            let call_id = extract_call_id(v).unwrap_or_default();
            let arguments = v
                .get("arguments")
                .and_then(|x| x.as_str())
                .unwrap_or("{}")
                .to_string();
            let name = v
                .get("name")
                .and_then(|x| x.as_str())
                .or_else(|| v.pointer("/item/name").and_then(|x| x.as_str()))
                .unwrap_or("")
                .to_string();
            if name.is_empty() {
                // Incomplete; wait for output_item.done
                None
            } else {
                Some(StreamEvent::ToolCallFinished {
                    output_index,
                    call_id,
                    name,
                    arguments,
                })
            }
        }
        // Unknown types: forward-compatible ignore
        _ => None,
    }
}

fn map_output_item_added(v: &Value) -> Option<StreamEvent> {
    let item = v.get("item")?;
    let item_type = item.get("type")?.as_str()?;
    let output_index = v.get("output_index").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
    match item_type {
        "message" => None,
        "function_call" => {
            let call_id = item
                .get("call_id")
                .and_then(|x| x.as_str())
                .or_else(|| item.get("id").and_then(|x| x.as_str()))
                .unwrap_or("")
                .to_string();
            let name = item
                .get("name")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            Some(StreamEvent::ToolCallStarted {
                output_index,
                call_id,
                name,
            })
        }
        _ => None,
    }
}

fn map_output_item_done(v: &Value) -> Option<StreamEvent> {
    let item = v.get("item")?;
    let item_type = item.get("type")?.as_str()?;
    let output_index = v.get("output_index").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
    match item_type {
        "function_call" => {
            let call_id = item
                .get("call_id")
                .and_then(|x| x.as_str())
                .or_else(|| item.get("id").and_then(|x| x.as_str()))
                .unwrap_or("")
                .to_string();
            let name = item
                .get("name")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            let arguments = item
                .get("arguments")
                .and_then(|x| x.as_str())
                .unwrap_or("{}")
                .to_string();
            Some(StreamEvent::ToolCallFinished {
                output_index,
                call_id,
                name,
                arguments,
            })
        }
        _ => None,
    }
}

fn extract_call_id(v: &Value) -> Option<String> {
    v.get("call_id")
        .and_then(|x| x.as_str())
        .or_else(|| v.pointer("/item/call_id").and_then(|x| x.as_str()))
        .map(|s| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignores_unknown_type() {
        let chunk = r#"data: {"type":"response.file_search.done","sequence_number":9}"#;
        assert_eq!(parse_sse_chunk(chunk).unwrap(), None);
    }
}
