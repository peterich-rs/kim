//! Reduce StreamEvents into SettledAssistantMessage.

use crate::LlmError;
use futures::StreamExt;
use kim_agent_types::{AssistantContent, SettledAssistantMessage, StopReason, StreamEvent, Usage};
use std::collections::BTreeMap;

#[derive(Debug, Default)]
struct TextBuf {
    text: String,
}

#[derive(Debug, Default)]
struct ToolBuf {
    call_id: String,
    name: String,
    args: String,
    finished: bool,
}

#[derive(Debug, Default)]
pub struct Reducer {
    provider_response_id: Option<String>,
    texts: BTreeMap<u32, TextBuf>,
    tools: BTreeMap<u32, ToolBuf>,
    usage: Usage,
    failed: Option<String>,
    completed: bool,
    aborted: bool,
    length: bool,
}

impl Reducer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn mark_aborted(&mut self) {
        self.aborted = true;
    }

    pub fn mark_length(&mut self) {
        self.length = true;
    }

    pub fn apply(&mut self, event: StreamEvent) {
        match event {
            StreamEvent::ResponseStarted {
                provider_response_id,
            } => {
                self.provider_response_id = Some(provider_response_id);
            }
            StreamEvent::TextDelta {
                output_index,
                delta,
            } => {
                self.texts
                    .entry(output_index)
                    .or_default()
                    .text
                    .push_str(&delta);
            }
            StreamEvent::ToolCallStarted {
                output_index,
                call_id,
                name,
            } => {
                let b = self.tools.entry(output_index).or_default();
                b.call_id = call_id;
                b.name = name;
            }
            StreamEvent::ToolCallArgsDelta {
                output_index,
                call_id,
                delta,
            } => {
                let b = self.tools.entry(output_index).or_default();
                if b.call_id.is_empty() {
                    b.call_id = call_id;
                }
                b.args.push_str(&delta);
            }
            StreamEvent::ToolCallFinished {
                output_index,
                call_id,
                name,
                arguments,
            } => {
                let b = self.tools.entry(output_index).or_default();
                if !call_id.is_empty() {
                    b.call_id = call_id;
                }
                if !name.is_empty() {
                    b.name = name;
                }
                if !arguments.is_empty() {
                    // Prefer complete arguments from finished event when present.
                    if b.args.is_empty() || arguments.len() >= b.args.len() {
                        b.args = arguments;
                    }
                }
                b.finished = true;
            }
            StreamEvent::UsageHint(u) => self.usage = u,
            StreamEvent::Completed => self.completed = true,
            StreamEvent::Failed { message } => self.failed = Some(message),
        }
    }

    pub fn settle(&self) -> SettledAssistantMessage {
        // Classification order (§10.6): Aborted → Error → Length → ToolUse → Stop
        let stop_reason = if self.aborted {
            StopReason::Aborted
        } else if self.failed.is_some() {
            StopReason::Error
        } else if self.length {
            StopReason::Length
        } else if self
            .tools
            .values()
            .any(|t| t.finished || !t.name.is_empty())
        {
            StopReason::ToolUse
        } else {
            StopReason::Stop
        };

        let mut indexes: Vec<u32> = self
            .texts
            .keys()
            .copied()
            .chain(self.tools.keys().copied())
            .collect();
        indexes.sort_unstable();
        indexes.dedup();

        let mut content = Vec::new();
        let mut source_index = 0usize;
        for idx in indexes {
            if let Some(t) = self.texts.get(&idx) {
                if !t.text.is_empty() {
                    content.push(AssistantContent::Text {
                        text: t.text.clone(),
                    });
                    source_index += 1;
                }
            }
            if let Some(tool) = self.tools.get(&idx) {
                if !tool.name.is_empty() || tool.finished {
                    let (arguments, parse_error) =
                        match serde_json::from_str::<serde_json::Value>(if tool.args.is_empty() {
                            "{}"
                        } else {
                            &tool.args
                        }) {
                            Ok(v) => (v, None),
                            Err(e) => (
                                serde_json::json!({}),
                                Some(format!("invalid tool arguments json: {e}")),
                            ),
                        };
                    content.push(AssistantContent::ToolCall {
                        call_id: tool.call_id.clone(),
                        name: tool.name.clone(),
                        arguments,
                        source_index,
                        parse_error,
                    });
                    source_index += 1;
                }
            }
        }

        SettledAssistantMessage {
            content,
            stop_reason,
            provider_response_id: self.provider_response_id.clone(),
        }
    }

    pub fn usage(&self) -> &Usage {
        &self.usage
    }

    pub fn failed_message(&self) -> Option<&str> {
        self.failed.as_deref()
    }
}

pub fn reduce_events(events: impl IntoIterator<Item = StreamEvent>) -> SettledAssistantMessage {
    let mut r = Reducer::new();
    for e in events {
        r.apply(e);
    }
    r.settle()
}

pub async fn reduce_stream<S>(mut stream: S) -> Result<(SettledAssistantMessage, Usage), LlmError>
where
    S: futures::Stream<Item = Result<StreamEvent, LlmError>> + Unpin,
{
    let mut r = Reducer::new();
    while let Some(item) = stream.next().await {
        match item {
            Ok(ev) => r.apply(ev),
            Err(LlmError::Aborted) => {
                r.mark_aborted();
                break;
            }
            Err(e) => return Err(e),
        }
    }
    Ok((r.settle(), r.usage().clone()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use kim_agent_types::StreamEvent;

    #[test]
    fn text_only_stop() {
        let settled = reduce_events([
            StreamEvent::ResponseStarted {
                provider_response_id: "r1".into(),
            },
            StreamEvent::TextDelta {
                output_index: 0,
                delta: "hi".into(),
            },
            StreamEvent::Completed,
        ]);
        assert_eq!(settled.stop_reason, StopReason::Stop);
        assert_eq!(settled.content.len(), 1);
    }
}
