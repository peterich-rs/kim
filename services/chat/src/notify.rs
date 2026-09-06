use kim_router::{Context, SessionError};
use prost::Message;
use tracing::warn;

/// Push `body` with `command` to every online location of `account`.
/// Skips the sender's own channel via [`Context::dispatch_cmd`].
pub async fn notify_account<B: Message>(ctx: &Context, account: &str, command: &str, body: &B) {
    match ctx.list_locations(account).await {
        Ok(locs) if !locs.is_empty() => {
            if let Err(err) = ctx.dispatch_cmd(command, body, &locs).await {
                warn!(%err, account, command, "account notify failed");
            }
        }
        Ok(_) | Err(SessionError::NotFound) => {}
        Err(err) => warn!(%err, account, command, "account notify loc failed"),
    }
}
