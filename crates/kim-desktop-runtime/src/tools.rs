use kim_sdk::{KimSdk, OutgoingPayload, SdkError, SendMessageCommand};

/// IM tools the host used to yield back to Dart. Results stay on this side.
pub async fn execute_im_tool(
    sdk: &KimSdk,
    name: &str,
    arguments_json: &str,
    current_dest: &str,
) -> String {
    let args = serde_json::from_str::<serde_json::Value>(arguments_json)
        .unwrap_or_else(|_| serde_json::json!({}));
    let outcome = match name {
        "search_messages" => search_messages(sdk, &args, current_dest).await,
        "send_message" => send_message(sdk, &args).await,
        "read_clipboard" => Err(SdkError::InvalidArgument {
            message: "clipboard is a UI capability; call the narrow clipboard hook".into(),
        }),
        other => Err(SdkError::InvalidArgument {
            message: format!("unknown tool {other}"),
        }),
    };
    match outcome {
        Ok(body) => body,
        Err(err) => serde_json::json!({"ok": false, "error": err.to_string()}).to_string(),
    }
}

async fn search_messages(
    sdk: &KimSdk,
    args: &serde_json::Value,
    current_dest: &str,
) -> Result<String, SdkError> {
    let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
    let dest = args
        .get("dest")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or(current_dest);
    let rows = sdk
        .search_messages(query.to_string(), Some(dest.to_string()))
        .await?;
    let hits: Vec<_> = rows
        .into_iter()
        .take(20)
        .map(|row| {
            serde_json::json!({
                "dest": row.dest,
                "sender": row.sender,
                "body": row.body,
                "messageId": row.message_id,
            })
        })
        .collect();
    Ok(serde_json::json!({"ok": true, "messages": hits}).to_string())
}

async fn send_message(sdk: &KimSdk, args: &serde_json::Value) -> Result<String, SdkError> {
    let dest = args.get("dest").and_then(|v| v.as_str()).unwrap_or("");
    let text = args.get("text").and_then(|v| v.as_str()).unwrap_or("");
    if dest.is_empty() || text.trim().is_empty() {
        return Err(SdkError::InvalidArgument {
            message: "send_message requires dest and text".into(),
        });
    }
    let receipt = sdk
        .enqueue_message(SendMessageCommand {
            dest: dest.to_string(),
            kind: 0,
            payload: OutgoingPayload::Text {
                body: text.to_string(),
            },
            client_id: None,
            batch_id: None,
        })
        .await?;
    Ok(serde_json::json!({
        "ok": true,
        "clientId": receipt.client_id,
    })
    .to_string())
}
