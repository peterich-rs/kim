use kim_protocol::pkt::{GroupCreateReq, GroupCreateResp, Status};
use kim_router::Context;

use crate::directory::{CreateGroup, GroupDirectory};

pub async fn do_group_create(ctx: Context, groups: &dyn GroupDirectory) {
    let req = match ctx.read_body::<GroupCreateReq>() {
        Ok(r) => r,
        Err(err) => {
            let _ = ctx.resp_with_error(Status::InvalidPacketBody, &err).await;
            return;
        }
    };
    match groups
        .create(
            &ctx.session().app,
            &CreateGroup {
                name: req.name,
                avatar: req.avatar,
                introduction: req.introduction,
                owner: req.owner,
                members: req.members,
            },
        )
        .await
    {
        Ok(group_id) => {
            let resp = GroupCreateResp { group_id };
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
    use kim_router::Router;
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
}
