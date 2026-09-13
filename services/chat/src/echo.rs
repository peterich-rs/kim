use kim_protocol::pkt::Status;
use kim_router::{Context, RouterError};

pub async fn do_echo(ctx: Context) -> Result<(), RouterError> {
    let body = ctx.request().body.clone();
    ctx.resp_bytes(Status::Success, body).await?;
    Ok(())
}
