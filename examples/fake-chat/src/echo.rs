use kim_protocol::pkt::Status;
use kim_router::Context;
use tracing::warn;

pub async fn do_echo(ctx: Context) {
    let body = ctx.request().body.clone();
    if let Err(err) = ctx.resp_bytes(Status::Success, body).await {
        warn!(%err, "resp failed");
    }
}
