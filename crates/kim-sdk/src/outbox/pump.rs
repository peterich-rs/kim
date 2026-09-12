use crate::error::SdkError;
use crate::KimSdk;

pub(crate) async fn run_once(sdk: &KimSdk) -> Result<usize, SdkError> {
    let store = sdk.store()?;
    let session = sdk.session_snapshot()?;
    let epoch = sdk.current_epoch().0;
    let due = store.load_due(&session.account).await?;
    let proto = sdk.protocol()?;
    let mut sent = 0usize;
    for row in due {
        if sdk.current_epoch().0 != epoch {
            return Err(SdkError::StaleEpoch {
                expected: epoch,
                actual: sdk.current_epoch().0,
            });
        }
        let extra = row.extra.clone();
        let mut body = row.body.clone();
        if row.payload_type == kim_protocol::MESSAGE_TYPE_IMAGE && !body.starts_with("http") {
            let url = sdk
                .uploader()?
                .upload_image(
                    &session.token,
                    &crate::media::MediaRef {
                        path: body.clone(),
                        mime: extra.clone(),
                        width: 0,
                        height: 0,
                        byte_size: 0,
                    },
                )
                .await?;
            body = url;
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
                store
                    .mark_failed(epoch, session.account.clone(), row.client_id)
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
