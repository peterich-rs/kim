//! Loopback HTTP for Royal: kick a live session via the existing Kickout path.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::post;
use axum::Router;
use kim_protocol::pkt::{Flag, KickAccount, KickoutNotify};
use kim_protocol::{LogicPkt, CMD_LOGIN_SIGN_IN};
use kim_router::{Dispatcher, SessionError, SessionStorage};
use prost::Message;
use tracing::{info, warn};

#[derive(Clone)]
pub struct ChatAdmin {
    cache: Arc<dyn SessionStorage>,
    dispatcher: Arc<dyn Dispatcher>,
}

impl ChatAdmin {
    pub fn new(cache: Arc<dyn SessionStorage>, dispatcher: Arc<dyn Dispatcher>) -> Self {
        Self { cache, dispatcher }
    }

    pub async fn kick(&self, account: &str) -> Result<bool, SessionError> {
        let locs = match self.cache.list_locations(account).await {
            Ok(v) => v,
            Err(SessionError::NotFound) => return Ok(false),
            Err(err) => return Err(err),
        };
        if locs.is_empty() {
            return Ok(false);
        }
        for loc in &locs {
            let mut pkt = LogicPkt::new(CMD_LOGIN_SIGN_IN, 0, Bytes::new());
            pkt.header.flag = Flag::Push as i32;
            pkt.write_body(&KickoutNotify {
                channel_id: loc.channel_id.clone(),
            });
            if let Err(err) = self
                .dispatcher
                .push(&loc.gate_id, std::slice::from_ref(&loc.channel_id), pkt)
                .await
            {
                warn!(%err, account, "kick dispatch failed");
                return Err(SessionError::Other(err.to_string()));
            }
            self.cache.delete(account, &loc.channel_id).await?;
            info!(account, channel = %loc.channel_id, "kicked");
        }
        Ok(true)
    }
}

async fn kick_handler(
    State(admin): State<ChatAdmin>,
    body: Bytes,
) -> Result<StatusCode, (StatusCode, String)> {
    let req =
        KickAccount::decode(body.as_ref()).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    if req.account.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "empty account".into()));
    }
    match admin.kick(&req.account).await {
        Ok(_) => Ok(StatusCode::NO_CONTENT),
        Err(err) => {
            warn!(%err, "kick failed");
            Err((StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))
        }
    }
}

pub fn router(admin: ChatAdmin) -> Router {
    Router::new()
        .route("/internal/kick", post(kick_handler))
        .with_state(admin)
}

pub async fn serve(listen: SocketAddr, app: Router) -> std::io::Result<()> {
    let listener = tokio::net::TcpListener::bind(listen).await?;
    axum::serve(listener, app)
        .await
        .map_err(|e| std::io::Error::other(e.to_string()))
}
