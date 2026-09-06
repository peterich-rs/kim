use kim_agent_harness::{AcceptRequest, DriveStatus, Effects, Harness, HarnessEffects};
use kim_agent_llm::ScriptedLlm;
use kim_agent_storage::{MemoryStorage, SqliteStorage, Storage, ValueAddr};
use kim_agent_tools::{ScriptedTool, ToolRegistry};
use kim_agent_types::*;
use std::sync::Arc;

fn tool_then_text() -> Vec<Vec<StreamEvent>> {
    vec![
        vec![
            StreamEvent::ResponseStarted {
                provider_response_id: "r1".into(),
            },
            StreamEvent::ToolCallStarted {
                output_index: 0,
                call_id: "call_1".into(),
                name: "echo".into(),
            },
            StreamEvent::ToolCallArgsDelta {
                output_index: 0,
                call_id: "call_1".into(),
                delta: "{\"msg\":\"hi\"}".into(),
            },
            StreamEvent::ToolCallFinished {
                output_index: 0,
                call_id: "call_1".into(),
                name: "echo".into(),
                arguments: "{\"msg\":\"hi\"}".into(),
            },
            StreamEvent::Completed,
        ],
        vec![
            StreamEvent::ResponseStarted {
                provider_response_id: "r2".into(),
            },
            StreamEvent::TextDelta {
                output_index: 0,
                delta: "done with tool".into(),
            },
            StreamEvent::Completed,
        ],
    ]
}

async fn run_e2e(storage: Arc<dyn Storage>) {
    let mut tools = ToolRegistry::new();
    let echo = ScriptedTool::new(
        "echo",
        vec![ToolResult {
            output: "echo:hi".into(),
            is_error: false,
        }],
        ReplayPolicy::Never,
    );
    tools.register(echo.clone());

    let llm = Arc::new(ScriptedLlm::new(tool_then_text()));
    let effects: Arc<dyn Effects> = Arc::new(HarnessEffects {
        llm,
        tools: tools.clone(),
    });

    // project with AGENTS.md
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("AGENTS.md"), "be helpful").unwrap();
    std::fs::create_dir_all(dir.path().join(".agents/skills")).unwrap();
    std::fs::write(dir.path().join(".agents/skills/x.md"), "skill x").unwrap();

    let harness = Harness::builder(storage.clone())
        .effects(effects)
        .tools(tools)
        .project_root(dir.path())
        .build()
        .unwrap();

    let cx = Context::new();
    let adm = harness
        .accept(AcceptRequest::prompt("use echo"), &cx)
        .await
        .unwrap();
    let status = harness.drive(adm.operation_id, &cx).await.unwrap();
    match status {
        DriveStatus::Completed { result } => match result {
            OperationResult::Completed { .. } => {}
            other => panic!("unexpected {other:?}"),
        },
        other => panic!("unexpected drive status {other:?}"),
    }
    assert_eq!(echo.call_count.load(std::sync::atomic::Ordering::SeqCst), 1);

    // tree has user + assistant(tool) + tool + assistant(text)
    let entries = storage.scan_entries().await.unwrap();
    assert!(entries.len() >= 4);
}

#[tokio::test]
async fn scripted_tool_path_memory() {
    run_e2e(Arc::new(MemoryStorage::new())).await;
}

#[tokio::test]
async fn scripted_tool_path_sqlite_and_resume_idle() {
    let dir = tempfile::tempdir().unwrap();
    let path = format!("sqlite://{}/h.db", dir.path().display());
    let storage: Arc<dyn Storage> = Arc::new(SqliteStorage::open(&path).await.unwrap());
    run_e2e(storage.clone()).await;

    // After complete, resume should find no current op
    let tools = ToolRegistry::new();
    let llm = Arc::new(ScriptedLlm::new(vec![]));
    let effects: Arc<dyn Effects> = Arc::new(HarnessEffects {
        llm,
        tools: tools.clone(),
    });
    let harness = Harness::builder(storage.clone())
        .effects(effects)
        .tools(tools)
        .build()
        .unwrap();
    let report = harness.resume(&Context::new()).await.unwrap();
    assert!(report.resumed_operations.is_empty());
}

#[tokio::test]
async fn sqlite_resume_from_assistant_ready() {
    let dir = tempfile::tempdir().unwrap();
    let path = format!("sqlite://{}/r.db", dir.path().display());
    let storage: Arc<dyn Storage> = Arc::new(SqliteStorage::open(&path).await.unwrap());

    let tools = ToolRegistry::new();
    let llm = Arc::new(ScriptedLlm::single(vec![
        StreamEvent::ResponseStarted {
            provider_response_id: "r".into(),
        },
        StreamEvent::TextDelta {
            output_index: 0,
            delta: "hello".into(),
        },
        StreamEvent::Completed,
    ]));
    let effects: Arc<dyn Effects> = Arc::new(HarnessEffects {
        llm: llm.clone(),
        tools: tools.clone(),
    });
    let harness = Harness::builder(storage.clone())
        .effects(effects)
        .tools(tools.clone())
        .build()
        .unwrap();
    let cx = Context::new();
    let adm = harness
        .accept(AcceptRequest::prompt("hi"), &cx)
        .await
        .unwrap();

    // Manually park at AssistantReady by advancing Starting→Checkpoint→AssistantReady via partial drives
    // Simulate crash: write AssistantReady directly after accept Starting state
    let op = adm.operation_id;
    storage
        .commit(vec![kim_agent_storage::Write::SetValue {
            address: ValueAddr::op_state(op),
            value: serde_json::to_value(OperationState::AssistantReady {
                scope: OperationScope::new(),
            })
            .unwrap(),
        }])
        .await
        .unwrap();

    // New harness instance (process restart)
    let effects2: Arc<dyn Effects> = Arc::new(HarnessEffects {
        llm,
        tools: tools.clone(),
    });
    let harness2 = Harness::builder(storage.clone())
        .effects(effects2)
        .tools(tools)
        .build()
        .unwrap();
    let report = harness2.resume(&cx).await.unwrap();
    assert_eq!(report.resumed_operations, vec![op]);
    let lane = storage
        .get_value(&ValueAddr::lane_state("main"))
        .await
        .unwrap()
        .unwrap();
    let lane: LaneState = serde_json::from_value(lane).unwrap();
    assert!(lane.current_operation_id.is_none());
}
