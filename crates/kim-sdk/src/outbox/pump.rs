use tokio_util::sync::CancellationToken;

use crate::error::SdkError;
use crate::KimSdk;

pub(crate) async fn run_once(sdk: &KimSdk, cancel: &CancellationToken) -> Result<usize, SdkError> {
    let store = sdk.store()?;
    let session = sdk.session_snapshot()?;
    let epoch = sdk.current_epoch().0;
    let due = store.load_due(&session.account).await?;
    let proto = sdk.protocol()?;
    let mut sent = 0usize;
    for row in due {
        if cancel.is_cancelled() {
            return Ok(sent);
        }
        if sdk.current_epoch().0 != epoch {
            sdk.metrics_inc_epoch_drop();
            return Err(SdkError::StaleEpoch {
                expected: epoch,
                actual: sdk.current_epoch().0,
            });
        }
        if !store.outbox_alive(&session.account, &row.client_id).await? {
            continue;
        }
        let mut extra = row.extra.clone();
        let mut body = if row.local_path.is_empty() {
            row.body.clone()
        } else {
            row.local_path.clone()
        };
        if row.payload_type == kim_protocol::MESSAGE_TYPE_IMAGE && !body.starts_with("http") {
            let url = sdk
                .uploader()?
                .upload_image(
                    &session.token,
                    &crate::media::MediaRef {
                        path: body.clone(),
                        mime: row.mime.clone(),
                        width: row.width,
                        height: row.height,
                        byte_size: row.byte_size,
                    },
                )
                .await?;
            body = url;
            extra = image_extra(row.width, row.height);
        }
        if cancel.is_cancelled() || !store.outbox_alive(&session.account, &row.client_id).await? {
            continue;
        }
        match proto
            .send_message(
                &row.dest,
                row.kind,
                &body,
                &extra,
                row.payload_type,
                &row.client_id,
            )
            .await
        {
            Ok((message_id, _)) => {
                if cancel.is_cancelled() {
                    continue;
                }
                store
                    .mark_sent(
                        epoch,
                        session.account.clone(),
                        row.client_id.clone(),
                        message_id,
                    )
                    .await?;
                sent += 1;
                if row.payload_type == kim_protocol::MESSAGE_TYPE_TEXT {
                    sdk.agent()
                        .enqueue_turn(
                            &row.dest,
                            &body,
                            message_id,
                            crate::ids::SessionEpoch(epoch),
                        )
                        .await;
                }
            }
            Err(err) if err.retryable_send() => {
                let attempt = row.attempt.saturating_add(1);
                let delay = 1000i64.saturating_mul(1i64 << attempt.min(6));
                store
                    .mark_retry(
                        epoch,
                        session.account.clone(),
                        row.client_id,
                        attempt,
                        crate::store::now_ms().saturating_add(delay),
                    )
                    .await?;
            }
            Err(_) => {
                store
                    .mark_failed(epoch, session.account.clone(), row.client_id)
                    .await?;
            }
        }
    }
    Ok(sent)
}

fn image_extra(width: i32, height: i32) -> String {
    if width <= 0 || height <= 0 {
        String::new()
    } else {
        format!(r#"{{"w":{width},"h":{height}}}"#)
    }
}
