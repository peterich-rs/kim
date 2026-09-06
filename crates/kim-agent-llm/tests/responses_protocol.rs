use kim_agent_llm::{parse_sse_stream, reduce_events, LlmError, ScriptedLlm, LlmClient};
use kim_agent_types::{AssistantContent, Context, StopReason, StreamEvent};
use futures::StreamExt;
use std::path::PathBuf;

fn fixture(name: &str) -> String {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("tests/fixtures/responses");
    p.push(name);
    std::fs::read_to_string(p).expect("fixture")
}

#[test]
fn text_only_stream_settles_stop() {
    let events = parse_sse_stream(&fixture("text_only.sse")).unwrap();
    let settled = reduce_events(events);
    assert_eq!(settled.stop_reason, StopReason::Stop);
    assert_eq!(
        settled.provider_response_id.as_deref(),
        Some("resp_text_1")
    );
    match &settled.content[0] {
        AssistantContent::Text { text } => assert_eq!(text, "Hello, world"),
        _ => panic!("expected text"),
    }
}

#[test]
fn tool_call_chunked_args() {
    let events = parse_sse_stream(&fixture("tool_call.sse")).unwrap();
    let settled = reduce_events(events);
    assert_eq!(settled.stop_reason, StopReason::ToolUse);
    match &settled.content[0] {
        AssistantContent::ToolCall {
            name,
            arguments,
            parse_error,
            ..
        } => {
            assert_eq!(name, "read");
            assert!(parse_error.is_none());
            assert_eq!(arguments["path"], "/tmp/a.txt");
        }
        _ => panic!("expected tool call"),
    }
}

#[test]
fn parallel_tools_ordered_by_output_index() {
    let events = parse_sse_stream(&fixture("parallel_tools.sse")).unwrap();
    let settled = reduce_events(events);
    assert_eq!(settled.stop_reason, StopReason::ToolUse);
    assert_eq!(settled.content.len(), 2);
    match &settled.content[0] {
        AssistantContent::ToolCall { name, call_id, .. } => {
            assert_eq!(name, "read");
            assert_eq!(call_id, "call_a");
        }
        _ => panic!("expected first tool"),
    }
    match &settled.content[1] {
        AssistantContent::ToolCall { name, call_id, .. } => {
            assert_eq!(name, "bash");
            assert_eq!(call_id, "call_b");
        }
        _ => panic!("expected second tool"),
    }
}

#[test]
fn failed_response_is_error() {
    let events = parse_sse_stream(&fixture("failed.sse")).unwrap();
    let settled = reduce_events(events);
    assert_eq!(settled.stop_reason, StopReason::Error);
}

#[test]
fn invalid_tool_json_keeps_sibling() {
    let events = parse_sse_stream(&fixture("bad_tool_json.sse")).unwrap();
    let settled = reduce_events(events);
    assert_eq!(settled.stop_reason, StopReason::ToolUse);
    assert_eq!(settled.content.len(), 2);
    match &settled.content[0] {
        AssistantContent::ToolCall { parse_error, .. } => {
            assert!(parse_error.is_some());
        }
        _ => panic!("expected tool"),
    }
    match &settled.content[1] {
        AssistantContent::ToolCall {
            name, parse_error, ..
        } => {
            assert_eq!(name, "bash");
            assert!(parse_error.is_none());
        }
        _ => panic!("expected sibling tool"),
    }
}

#[tokio::test]
async fn mid_stream_cancel_aborted() {
    let llm = ScriptedLlm::single(vec![
        StreamEvent::ResponseStarted {
            provider_response_id: "r".into(),
        },
        StreamEvent::TextDelta {
            output_index: 0,
            delta: "partial".into(),
        },
        StreamEvent::TextDelta {
            output_index: 0,
            delta: " more".into(),
        },
        StreamEvent::Completed,
    ]);
    let cx = Context::new();
    let mut stream = llm.stream(Default::default(), &cx).await.unwrap();
    let mut reducer = kim_agent_llm::Reducer::new();
    // First event OK, then cancel before further polls.
    match stream.next().await {
        Some(Ok(ev)) => reducer.apply(ev),
        other => panic!("expected first event, got {other:?}"),
    }
    cx.cancel();
    let mut saw_abort = false;
    while let Some(item) = stream.next().await {
        match item {
            Ok(ev) => reducer.apply(ev),
            Err(LlmError::Aborted) => {
                reducer.mark_aborted();
                saw_abort = true;
                break;
            }
            Err(e) => panic!("{e}"),
        }
    }
    assert!(saw_abort, "expected Aborted after cancel");
    assert_eq!(reducer.settle().stop_reason, StopReason::Aborted);
}
