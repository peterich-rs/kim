use kim_agent_storage::{MemoryStorage, SqliteStorage, Storage, ValueAddr, Write};
use kim_agent_types::{
    AgentMessage, Entry, EntryBase, EntryId, OperationId, Usage, UsageId, UsageRow,
};

async fn check_backend<S: Storage>(store: &S) {
    let id = EntryId::new();
    let entry = Entry::Message {
        base: EntryBase {
            id,
            parent_id: None,
            seq: 0,
            timestamp_ms: 0,
        },
        message: AgentMessage::User {
            text: "hello".into(),
        },
    };
    let usage_id = UsageId::new();
    let op = OperationId::new();
    store
        .commit(vec![
            Write::InsertEntry(entry),
            Write::InsertUsage(UsageRow {
                id: usage_id,
                operation_id: op,
                usage: Usage::default(),
                model: Some("test".into()),
            }),
            Write::SetValue {
                address: ValueAddr::branch_tip("main"),
                value: serde_json::json!(id.to_string()),
            },
            Write::AppendList {
                address: ValueAddr::new("app.list", "a"),
                element: serde_json::json!(1),
            },
        ])
        .await
        .unwrap();

    assert!(store.get_entry(id).await.unwrap().is_some());
    assert_eq!(
        store
            .get_value(&ValueAddr::branch_tip("main"))
            .await
            .unwrap()
            .unwrap()
            .as_str()
            .unwrap(),
        id.to_string()
    );
    assert_eq!(
        store
            .get_list(&ValueAddr::new("app.list", "a"))
            .await
            .unwrap()
            .len(),
        1
    );
    assert!(store.get_usage(usage_id).await.unwrap().is_some());

    let dup = Entry::Message {
        base: EntryBase {
            id,
            parent_id: None,
            seq: 0,
            timestamp_ms: 0,
        },
        message: AgentMessage::User { text: "x".into() },
    };
    assert!(store.commit(vec![Write::InsertEntry(dup)]).await.is_err());
}

#[tokio::test]
async fn memory_conformance() {
    check_backend(&MemoryStorage::new()).await;
}

#[tokio::test]
async fn sqlite_conformance() {
    let dir = tempfile::tempdir().unwrap();
    let path = format!("sqlite://{}/c.db", dir.path().display());
    let store = SqliteStorage::open(&path).await.unwrap();
    check_backend(&store).await;
}
