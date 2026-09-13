use kim_protocol::pkt::{Flag, Session};
use kim_protocol::{AccountId, ChannelId, GatewayId, LogicPkt};
use kim_router::{Context, Dispatcher, Location, RouterError, SessionError, SessionStorage};
use prost::Message;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::warn;

/// Push `body` with `command` to every online location of `account`.
/// Skips the sender's own channel via [`Context::dispatch_cmd`].
pub async fn notify_account<B: Message>(ctx: &Context, account: &str, command: &str, body: &B) {
    match ctx.list_locations(&AccountId::from_trusted(account)).await {
        Ok(locs) if !locs.is_empty() => {
            if let Err(err) = ctx.dispatch_cmd(command, body, &locs).await {
                warn!(%err, account, command, "account notify failed");
            }
        }
        Ok(_) | Err(SessionError::NotFound) => {}
        Err(err) => warn!(%err, account, command, "account notify loc failed"),
    }
}

/// Push to explicit locations without a request Context (system / debounce).
/// `skip_channel` empty → push to all `recvs`.
pub async fn notify_locations<B: Message>(
    dispatcher: &dyn Dispatcher,
    skip_channel: &str,
    command: &str,
    body: &B,
    recvs: &[Location],
) -> Result<(), RouterError> {
    if recvs.is_empty() {
        return Ok(());
    }
    let mut packet = LogicPkt::new(command, 0, bytes::Bytes::new());
    packet.header.flag = Flag::Push as i32;
    packet.write_body(body);

    let mut group: HashMap<GatewayId, Vec<ChannelId>> = HashMap::new();
    let mut order: Vec<GatewayId> = Vec::new();
    for recv in recvs {
        if !skip_channel.is_empty() && recv.channel_id.as_str() == skip_channel {
            continue;
        }
        match group.entry(recv.gate_id.clone()) {
            Entry::Vacant(v) => {
                order.push(v.key().clone());
                v.insert(vec![recv.channel_id.clone()]);
            }
            Entry::Occupied(mut o) => {
                o.get_mut().push(recv.channel_id.clone());
            }
        }
    }

    let mut first_err = None;
    for gw in order {
        let Some(ids) = group.get(&gw) else {
            continue;
        };
        if let Err(err) = dispatcher.push(&gw, ids, packet.clone()).await {
            warn!(%err, gateway = %gw, "notify_locations push failed");
            if first_err.is_none() {
                first_err = Some(err);
            }
        }
    }
    match first_err {
        Some(err) => Err(err),
        None => Ok(()),
    }
}

/// Synthetic context for system pushes (empty channel_id → nothing skipped).
#[allow(dead_code)]
pub fn system_context(
    dispatcher: Arc<dyn Dispatcher>,
    storage: Arc<dyn SessionStorage>,
) -> Context {
    let request = LogicPkt::new("chat.presence", 0, bytes::Bytes::new());
    let session = Session::default();
    Context::new(request, session, dispatcher, storage)
}
