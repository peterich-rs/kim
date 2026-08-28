use kim_protocol::pkt::{
    GroupCreateNotify, GroupCreateReq, GroupCreateResp, GroupDetail, GroupJoinReq,
    GroupMembersResp, GroupQuitReq, Status,
};
use kim_router::{Context, SessionError};
use tracing::warn;

use crate::directory::{CreateGroup, GroupDirectory};

#[derive(Debug, thiserror::Error)]
enum GroupCmdError {
    #[error("no destination")]
    NoDestination,
}

fn dest_or_body_group<'a>(header_dest: &'a str, body_group: &'a str) -> &'a str {
    if !header_dest.is_empty() {
        header_dest
    } else {
        body_group
    }
}

pub async fn do_group_create(ctx: Context, groups: &dyn GroupDirectory) {
    let req = match ctx.read_body::<GroupCreateReq>() {
        Ok(r) => r,
        Err(err) => {
            let _ = ctx.resp_with_error(Status::InvalidPacketBody, &err).await;
            return;
        }
    };
    let members = req.members.clone();
    let group_id = match groups
        .create(
            &ctx.session().app,
            &CreateGroup {
                name: req.name,
                avatar: req.avatar,
                introduction: req.introduction,
                owner: req.owner,
                members: members.clone(),
            },
        )
        .await
    {
        Ok(id) => id,
        Err(err) => {
            let _ = ctx.resp_with_error(Status::SystemException, &err).await;
            return;
        }
    };

    let locs = match ctx.get_locations(&members).await {
        Ok(v) => v,
        Err(SessionError::NotFound) => Vec::new(),
        Err(err) => {
            warn!(%err, "get_locations failed");
            let _ = ctx.resp_with_error(Status::SystemException, &err).await;
            return;
        }
    };
    if !locs.is_empty() {
        let notify = GroupCreateNotify {
            group_id: group_id.clone(),
            members,
        };
        if let Err(err) = ctx.dispatch(&notify, &locs).await {
            warn!(%err, "dispatch GroupCreateNotify failed");
            let _ = ctx.resp_with_error(Status::SystemException, &err).await;
            return;
        }
    }

    let resp = GroupCreateResp { group_id };
    let _ = ctx.resp(Status::Success, Some(&resp)).await;
}

pub async fn do_group_join(ctx: Context, groups: &dyn GroupDirectory) {
    let req = match ctx.read_body::<GroupJoinReq>() {
        Ok(r) => r,
        Err(err) => {
            let _ = ctx.resp_with_error(Status::InvalidPacketBody, &err).await;
            return;
        }
    };
    let group_id = dest_or_body_group(&ctx.header().dest, &req.group_id);
    if group_id.is_empty() {
        let _ = ctx
            .resp_with_error(Status::NoDestination, &GroupCmdError::NoDestination)
            .await;
        return;
    }
    let account = if req.account.is_empty() {
        ctx.session().account.as_str()
    } else {
        req.account.as_str()
    };
    match groups.join(&ctx.session().app, group_id, account).await {
        Ok(()) => {
            let _ = ctx.resp_bytes(Status::Success, bytes::Bytes::new()).await;
        }
        Err(err) => {
            let _ = ctx.resp_with_error(Status::SystemException, &err).await;
        }
    }
}

pub async fn do_group_quit(ctx: Context, groups: &dyn GroupDirectory) {
    let req = match ctx.read_body::<GroupQuitReq>() {
        Ok(r) => r,
        Err(err) => {
            let _ = ctx.resp_with_error(Status::InvalidPacketBody, &err).await;
            return;
        }
    };
    let group_id = dest_or_body_group(&ctx.header().dest, &req.group_id);
    if group_id.is_empty() {
        let _ = ctx
            .resp_with_error(Status::NoDestination, &GroupCmdError::NoDestination)
            .await;
        return;
    }
    let account = if req.account.is_empty() {
        ctx.session().account.as_str()
    } else {
        req.account.as_str()
    };
    match groups.quit(&ctx.session().app, group_id, account).await {
        Ok(()) => {
            let _ = ctx.resp_bytes(Status::Success, bytes::Bytes::new()).await;
        }
        Err(err) => {
            let _ = ctx.resp_with_error(Status::SystemException, &err).await;
        }
    }
}

pub async fn do_group_detail(ctx: Context, groups: &dyn GroupDirectory) {
    if ctx.header().dest.is_empty() {
        let _ = ctx
            .resp_with_error(Status::NoDestination, &GroupCmdError::NoDestination)
            .await;
        return;
    }
    match groups.detail(&ctx.session().app, &ctx.header().dest).await {
        Ok(info) => {
            let resp = GroupDetail {
                group_id: info.id,
                name: info.name,
                avatar: info.avatar,
                introduction: info.introduction,
                owner: info.owner,
                members: info.members,
            };
            let _ = ctx.resp(Status::Success, Some(&resp)).await;
        }
        Err(err) => {
            let _ = ctx.resp_with_error(Status::SystemException, &err).await;
        }
    }
}

pub async fn do_group_members(ctx: Context, groups: &dyn GroupDirectory) {
    if ctx.header().dest.is_empty() {
        let _ = ctx
            .resp_with_error(Status::NoDestination, &GroupCmdError::NoDestination)
            .await;
        return;
    }
    match groups.members(&ctx.session().app, &ctx.header().dest).await {
        Ok(members) => {
            let resp = GroupMembersResp { members };
            let _ = ctx.resp(Status::Success, Some(&resp)).await;
        }
        Err(err) => {
            let _ = ctx.resp_with_error(Status::SystemException, &err).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use bytes::Bytes;
    use kim_protocol::pkt::{Flag, GroupCreateReq, GroupCreateResp, Session, Status};
    use kim_protocol::{LogicPkt, CMD_GROUP_CREATE, META_DEST_SERVER};
    use kim_router::test_support::RecordingDispatcher;
    use kim_router::{Router, SessionStorage};
    use kim_session::MemorySessionStore;

    use super::do_group_create;
    use crate::directory::{GroupDirectory, MemoryGroupDirectory};
    use crate::idgen::{IdGenerator, SequenceIdGen};

    fn sender_session() -> Session {
        Session {
            channel_id: "ch-alice".into(),
            gate_id: "wg-1".into(),
            account: "alice".into(),
            app: "kim".into(),
            ..Session::default()
        }
    }

    fn create_pkt(body: Bytes) -> LogicPkt {
        let mut pkt = LogicPkt::new(CMD_GROUP_CREATE, 1, body);
        pkt.header.channel_id = "ch-alice".into();
        pkt.set_meta(META_DEST_SERVER, "wg-1");
        pkt
    }

    fn create_req_pkt(req: &GroupCreateReq) -> LogicPkt {
        let mut pkt = create_pkt(Bytes::new());
        pkt.write_body(req);
        pkt
    }

    fn memory_groups() -> Arc<MemoryGroupDirectory> {
        let idgen: Arc<dyn IdGenerator> = Arc::new(SequenceIdGen::default());
        Arc::new(MemoryGroupDirectory::new(idgen))
    }

    async fn serve_group_create(
        groups: Arc<dyn GroupDirectory>,
        dispatcher: Arc<RecordingDispatcher>,
        pkt: LogicPkt,
        session: Session,
    ) {
        let mut router = Router::new();
        router.handle(CMD_GROUP_CREATE, move |ctx| {
            let groups = groups.clone();
            async move { do_group_create(ctx, groups.as_ref()).await }
        });
        router
            .serve(
                pkt,
                dispatcher,
                Arc::new(MemorySessionStore::new()),
                session,
            )
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn valid_req_succeeds_and_members_include_owner_and_members() {
        let groups = memory_groups();
        let dispatcher = Arc::new(RecordingDispatcher::default());
        serve_group_create(
            groups.clone(),
            dispatcher.clone(),
            create_req_pkt(&GroupCreateReq {
                name: "group1".into(),
                avatar: "av".into(),
                introduction: "intro".into(),
                owner: "alice".into(),
                members: vec!["bob".into(), "carol".into()],
            }),
            sender_session(),
        )
        .await;

        let got = dispatcher.recorded();
        let resps: Vec<_> = got
            .iter()
            .filter(|p| p.pkt.header.flag == Flag::Response as i32)
            .collect();
        assert_eq!(resps.len(), 1);
        assert_eq!(resps[0].pkt.header.status, Status::Success as i32);
        let resp: GroupCreateResp = resps[0].pkt.read_body().unwrap();
        assert!(!resp.group_id.is_empty());
        let members = groups.members("kim", &resp.group_id).await.unwrap();
        assert!(members.contains(&"alice".to_string()));
        assert!(members.contains(&"bob".to_string()));
        assert!(members.contains(&"carol".to_string()));
    }

    #[tokio::test]
    async fn bad_body_is_invalid_packet_body() {
        let groups = memory_groups();
        let dispatcher = Arc::new(RecordingDispatcher::default());
        serve_group_create(
            groups.clone(),
            dispatcher.clone(),
            create_pkt(Bytes::from_static(&[0xff, 0x00, 0xab])),
            sender_session(),
        )
        .await;

        let got = dispatcher.recorded();
        let resps: Vec<_> = got
            .iter()
            .filter(|p| p.pkt.header.flag == Flag::Response as i32)
            .collect();
        assert_eq!(resps.len(), 1);
        assert_eq!(resps[0].pkt.header.status, Status::InvalidPacketBody as i32);
    }

    #[tokio::test]
    async fn create_notifies_online_members_except_sender() {
        let groups = memory_groups();
        let cache = Arc::new(MemorySessionStore::new());
        cache
            .add(&Session {
                channel_id: "ch-bob".into(),
                gate_id: "wg-1".into(),
                account: "bob".into(),
                app: "kim".into(),
                ..Session::default()
            })
            .await
            .unwrap();
        let dispatcher = Arc::new(RecordingDispatcher::default());
        let mut router = Router::new();
        router.handle(CMD_GROUP_CREATE, {
            let groups = groups.clone();
            move |ctx| {
                let groups = groups.clone();
                async move { do_group_create(ctx, groups.as_ref()).await }
            }
        });
        router
            .serve(
                create_req_pkt(&GroupCreateReq {
                    name: "g".into(),
                    owner: "alice".into(),
                    members: vec!["alice".into(), "bob".into()],
                    avatar: String::new(),
                    introduction: String::new(),
                }),
                dispatcher.clone(),
                cache,
                sender_session(),
            )
            .await
            .unwrap();
        let got = dispatcher.recorded();
        let pushes: Vec<_> = got
            .iter()
            .filter(|p| p.pkt.header.flag == Flag::Push as i32)
            .collect();
        assert_eq!(pushes.len(), 1);
        assert_eq!(pushes[0].channels, vec!["ch-bob".to_string()]);
        let n: kim_protocol::pkt::GroupCreateNotify = pushes[0].pkt.read_body().unwrap();
        assert!(!n.group_id.is_empty());
        assert!(n.members.contains(&"bob".to_string()));
    }
}
