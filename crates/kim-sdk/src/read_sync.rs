use std::time::Duration;

use tokio::sync::mpsc;
use tokio::time::{self, MissedTickBehavior};

use crate::error::SdkError;
use crate::KimSdk;

pub(crate) fn spawn(sdk: KimSdk, mut kick: mpsc::Receiver<()>) {
    let parent = sdk.child_token();
    tokio::spawn(async move {
        let mut ticker = time::interval(Duration::from_secs(2));
        ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                _ = parent.cancelled() => break,
                msg = kick.recv() => {
                    if msg.is_none() {
                        break;
                    }
                }
                _ = ticker.tick() => {}
            }
            if let Err(err) = pump_once(&sdk).await {
                tracing::debug!(error = %err, "read sync pump failed");
            }
        }
    });
}

async fn pump_once(sdk: &KimSdk) -> Result<(), SdkError> {
    let store = sdk.store()?;
    let session = match sdk.session_snapshot() {
        Ok(s) if !s.account.is_empty() => s,
        _ => return Ok(()),
    };
    let epoch = sdk.current_epoch().0;
    let due = store.due_reads(&session.account).await?;
    if due.is_empty() {
        return Ok(());
    }
    let Ok(proto) = sdk.protocol() else {
        return Ok(());
    };
    for item in due {
        if sdk.current_epoch().0 != epoch {
            return Ok(());
        }
        match proto
            .mark_read_state(&item.dest, item.kind, item.message_id)
            .await
        {
            Ok(state) => {
                let confirmed = state
                    .as_ref()
                    .map(|s| s.last_read_message_id.max(item.message_id))
                    .unwrap_or(item.message_id);
                let ((), sequence) = store
                    .confirm_read(
                        epoch,
                        session.account.clone(),
                        item.dest.clone(),
                        confirmed,
                        state,
                    )
                    .await?;
                sdk.after_command(sequence).await;
            }
            Err(SdkError::NotConnected) => return Ok(()),
            Err(err) => {
                tracing::warn!(dest = %item.dest, error = %err, "read sync failed");
                let _ = crate::store::watermarks::mark_due_failed_on_pool(
                    &store.pool,
                    &session.account,
                    &item.dest,
                    &err.to_string(),
                )
                .await;
            }
        }
    }
    Ok(())
}
