//! Scripted LLM for harness tests (no network).

use crate::{LlmClient, LlmError, LlmResult, ResponseRequest};
use async_trait::async_trait;
use futures::stream::{self, BoxStream};
use kim_agent_types::{Context, StreamEvent};
use std::sync::Mutex;

pub struct ScriptedLlm {
    scripts: Mutex<Vec<Vec<StreamEvent>>>,
}

impl ScriptedLlm {
    pub fn new(scripts: Vec<Vec<StreamEvent>>) -> Self {
        Self {
            scripts: Mutex::new(scripts),
        }
    }

    pub fn single(events: Vec<StreamEvent>) -> Self {
        Self::new(vec![events])
    }
}

#[async_trait]
impl LlmClient for ScriptedLlm {
    async fn stream(
        &self,
        _req: ResponseRequest,
        cx: &Context,
    ) -> LlmResult<BoxStream<'static, LlmResult<StreamEvent>>> {
        let mut guard = self.scripts.lock().expect("script lock");
        if guard.is_empty() {
            return Err(LlmError::Failed("no scripted responses left".into()));
        }
        let events = guard.remove(0);
        drop(guard);
        let cancelled = cx.abort.clone();
        // State: (remaining events, cancel token, already_emitted_abort)
        let s = stream::unfold(
            (events.into_iter(), cancelled, false),
            |(mut iter, cancelled, aborted)| async move {
                if aborted {
                    return None;
                }
                if cancelled.is_cancelled() {
                    return Some((Err(LlmError::Aborted), (iter, cancelled, true)));
                }
                match iter.next() {
                    Some(ev) => Some((Ok(ev), (iter, cancelled, false))),
                    None => None,
                }
            },
        );
        Ok(Box::pin(s))
    }
}
