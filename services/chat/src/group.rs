use kim_protocol::pkt::{
    GroupCreateNotify, GroupCreateReq, GroupCreateResp, GroupDetail, GroupJoinReq,
    GroupMembersResp, GroupQuitReq, Status,
};
use kim_router::{Context, SessionError};
use tracing::warn;

use crate::directory::{CreateGroup, GroupDirectory, GroupError, GroupInfo};

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

fn is_member(info: &GroupInfo, account: &str) -> bool {
    info.members.iter().any(|m| m == account)
}

fn group_lookup_status(err: &GroupError) -> Status {
    match err {
        GroupError::NotFound => Status::NotGroupMember,
        GroupError::Id(_) | GroupError::Backend(_) => Status::SystemException,
    }
}

async fn load_group(
    groups: &dyn GroupDirectory,
    app: &str,
    group_id: &str,
) -> Result<GroupInfo, GroupError> {
    groups.detail(app, group_id).await
}

pub async fn do_group_create(ctx: Context, groups: &dyn GroupDirectory) {
    let req = match ctx.read_body::<GroupCreateReq>() {
        Ok(r) => r,
        Err(err) => {
            let _ = ctx.resp_with_error(Status::InvalidPacketBody, &err).await;
            return;
        }
    };
    let owner = ctx.session().account.clone();
    if !req.owner.is_empty() && req.owner != owner {
        warn!(requested = %req.owner, session = %owner, "ignoring create owner");
    }
    if req.members.iter().any(|m| m != &owner) {
        warn!(session = %owner, "ignoring create members");
    }
    let members = vec![owner.clone()];
    let group_id = match groups
        .create(
            &ctx.session().app,
            &CreateGroup {
                name: req.name,
                avatar: req.avatar,
                introduction: req.introduction,
                owner,
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
    let session_account = ctx.session().account.as_str();
    if !req.account.is_empty() && req.account != session_account {
        let _ = ctx
            .resp_bytes(Status::Unauthorized, bytes::Bytes::new())
            .await;
        return;
    }
    match load_group(groups, &ctx.session().app, group_id).await {
        Ok(info) if is_member(&info, session_account) => {
            let _ = ctx.resp_bytes(Status::Success, bytes::Bytes::new()).await;
        }
        Ok(_) => {
            let _ = ctx
                .resp_bytes(Status::Unauthorized, bytes::Bytes::new())
                .await;
        }
        Err(err) => {
            let _ = ctx.resp_with_error(group_lookup_status(&err), &err).await;
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
    let session_account = ctx.session().account.as_str();
    if !req.account.is_empty() && req.account != session_account {
        let _ = ctx
            .resp_bytes(Status::Unauthorized, bytes::Bytes::new())
            .await;
        return;
    }
    match load_group(groups, &ctx.session().app, group_id).await {
        Ok(info) if is_member(&info, session_account) => {
            match groups
                .quit(&ctx.session().app, group_id, session_account)
                .await
            {
                Ok(()) => {
                    let _ = ctx.resp_bytes(Status::Success, bytes::Bytes::new()).await;
                }
                Err(err) => {
                    let _ = ctx.resp_with_error(group_lookup_status(&err), &err).await;
                }
            }
        }
        Ok(_) => {
            let _ = ctx
                .resp_bytes(Status::NotGroupMember, bytes::Bytes::new())
                .await;
        }
        Err(err) => {
            let _ = ctx.resp_with_error(group_lookup_status(&err), &err).await;
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
    match load_group(groups, &ctx.session().app, &ctx.header().dest).await {
        Ok(info) if is_member(&info, &ctx.session().account) => {
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
        Ok(_) => {
            let _ = ctx
                .resp_bytes(Status::NotGroupMember, bytes::Bytes::new())
                .await;
        }
        Err(err) => {
            let _ = ctx.resp_with_error(group_lookup_status(&err), &err).await;
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
    match load_group(groups, &ctx.session().app, &ctx.header().dest).await {
        Ok(info) if is_member(&info, &ctx.session().account) => {
            let resp = GroupMembersResp {
                members: info.members,
            };
            let _ = ctx.resp(Status::Success, Some(&resp)).await;
        }
        Ok(_) => {
            let _ = ctx
                .resp_bytes(Status::NotGroupMember, bytes::Bytes::new())
                .await;
        }
        Err(err) => {
            let _ = ctx.resp_with_error(group_lookup_status(&err), &err).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use async_trait::async_trait;
    use bytes::Bytes;
    use kim_protocol::pkt::{
        Flag, GroupCreateReq, GroupCreateResp, GroupJoinReq, GroupQuitReq, Session, Status,
    };
    use kim_protocol::{
        LogicPkt, CMD_GROUP_CREATE, CMD_GROUP_DETAIL, CMD_GROUP_JOIN, CMD_GROUP_MEMBERS,
        CMD_GROUP_QUIT, META_DEST_SERVER,
    };
    use kim_router::test_support::RecordingDispatcher;
    use kim_router::{Router, SessionStorage};
    use kim_session::MemorySessionStore;

    use super::{do_group_create, do_group_detail, do_group_join, do_group_members, do_group_quit};
    use crate::directory::{
        CreateGroup, GroupDirectory, GroupError, GroupInfo, MemoryGroupDirectory,
    };
    use crate::idgen::{IdGenerator, SequenceIdGen};

    fn alice() -> Session {
        Session {
            channel_id: "ch-alice".into(),
            gate_id: "wg-1".into(),
            account: "alice".into(),
            app: "kim".into(),
            ..Session::default()
        }
    }

    fn session(account: &str, app: &str) -> Session {
        Session {
            channel_id: format!("ch-{account}"),
            gate_id: "wg-1".into(),
            account: account.into(),
            app: app.into(),
            ..Session::default()
        }
    }

    fn pkt(cmd: &str, dest: &str, body: Bytes, channel: &str) -> LogicPkt {
        let mut pkt = LogicPkt::new(cmd, 1, body);
        pkt.header.channel_id = channel.into();
        pkt.set_meta(META_DEST_SERVER, "wg-1");
        if !dest.is_empty() {
            pkt.set_dest(dest);
        }
        pkt
    }

    fn create_req_pkt(req: &GroupCreateReq) -> LogicPkt {
        let mut p = pkt(CMD_GROUP_CREATE, "", Bytes::new(), "ch-alice");
        p.write_body(req);
        p
    }

    fn memory_groups() -> Arc<MemoryGroupDirectory> {
        let idgen: Arc<dyn IdGenerator> = Arc::new(SequenceIdGen::default());
        Arc::new(MemoryGroupDirectory::new(idgen))
    }

    async fn serve(
        cmd: &'static str,
        groups: Arc<dyn GroupDirectory>,
        dispatcher: Arc<RecordingDispatcher>,
        logic: LogicPkt,
        session: Session,
    ) {
        let mut router = Router::new();
        router.handle(cmd, move |ctx| {
            let groups = groups.clone();
            async move {
                match cmd {
                    CMD_GROUP_CREATE => do_group_create(ctx, groups.as_ref()).await,
                    CMD_GROUP_JOIN => do_group_join(ctx, groups.as_ref()).await,
                    CMD_GROUP_QUIT => do_group_quit(ctx, groups.as_ref()).await,
                    CMD_GROUP_DETAIL => do_group_detail(ctx, groups.as_ref()).await,
                    CMD_GROUP_MEMBERS => do_group_members(ctx, groups.as_ref()).await,
                    _ => unreachable!(),
                }
            }
        });
        router
            .serve(
                logic,
                dispatcher,
                Arc::new(MemorySessionStore::new()),
                session,
            )
            .await
            .unwrap();
    }

    fn status_of(dispatcher: &RecordingDispatcher) -> i32 {
        let got = dispatcher.recorded();
        let resps: Vec<_> = got
            .iter()
            .filter(|p| p.pkt.header.flag == Flag::Response as i32)
            .collect();
        assert_eq!(resps.len(), 1);
        resps[0].pkt.header.status
    }

    async fn create_alice_group(groups: Arc<MemoryGroupDirectory>) -> String {
        let dispatcher = Arc::new(RecordingDispatcher::default());
        serve(
            CMD_GROUP_CREATE,
            groups.clone(),
            dispatcher.clone(),
            create_req_pkt(&GroupCreateReq {
                name: "g".into(),
                avatar: String::new(),
                introduction: String::new(),
                owner: "eve".into(),
                members: vec!["eve".into(), "bob".into()],
            }),
            alice(),
        )
        .await;
        let got = dispatcher.recorded();
        let resp: GroupCreateResp = got
            .iter()
            .find(|p| p.pkt.header.flag == Flag::Response as i32)
            .unwrap()
            .pkt
            .read_body()
            .unwrap();
        resp.group_id
    }

    #[tokio::test]
    async fn create_forces_session_owner_and_drops_extra_members() {
        let groups = memory_groups();
        let dispatcher = Arc::new(RecordingDispatcher::default());
        serve(
            CMD_GROUP_CREATE,
            groups.clone(),
            dispatcher.clone(),
            create_req_pkt(&GroupCreateReq {
                name: "group1".into(),
                avatar: "av".into(),
                introduction: "intro".into(),
                owner: "eve".into(),
                members: vec!["eve".into(), "bob".into()],
            }),
            alice(),
        )
        .await;
        assert_eq!(status_of(&dispatcher), Status::Success as i32);
        let resp: GroupCreateResp = dispatcher.recorded()[0].pkt.read_body().unwrap();
        let members = groups.members("kim", &resp.group_id).await.unwrap();
        assert_eq!(members, vec!["alice".to_string()]);
        let detail = groups.detail("kim", &resp.group_id).await.unwrap();
        assert_eq!(detail.owner, "alice");
    }

    #[tokio::test]
    async fn bad_body_is_invalid_packet_body() {
        let groups = memory_groups();
        let dispatcher = Arc::new(RecordingDispatcher::default());
        serve(
            CMD_GROUP_CREATE,
            groups,
            dispatcher.clone(),
            pkt(
                CMD_GROUP_CREATE,
                "",
                Bytes::from_static(&[0xff, 0x00, 0xab]),
                "ch-alice",
            ),
            alice(),
        )
        .await;
        assert_eq!(status_of(&dispatcher), Status::InvalidPacketBody as i32);
    }

    #[tokio::test]
    async fn create_notifies_creator_other_devices_only() {
        let groups = memory_groups();
        let cache = Arc::new(MemorySessionStore::new());
        cache
            .add(&Session {
                channel_id: "ch-alice-web".into(),
                gate_id: "wg-1".into(),
                account: "alice".into(),
                app: "kim".into(),
                ..Session::default()
            })
            .await
            .unwrap();
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
                alice(),
            )
            .await
            .unwrap();
        let pushes: Vec<_> = dispatcher
            .recorded()
            .into_iter()
            .filter(|p| p.pkt.header.flag == Flag::Push as i32)
            .collect();
        assert_eq!(pushes.len(), 1);
        assert_eq!(pushes[0].channels, vec!["ch-alice-web".to_string()]);
        let n: kim_protocol::pkt::GroupCreateNotify = pushes[0].pkt.read_body().unwrap();
        assert_eq!(n.members, vec!["alice".to_string()]);
    }

    #[tokio::test]
    async fn join_rejects_self_serve_and_proxy() {
        let groups = memory_groups();
        let gid = create_alice_group(groups.clone()).await;
        let join = |account: &str| {
            let mut p = pkt(CMD_GROUP_JOIN, &gid, Bytes::new(), "ch-bob");
            p.write_body(&GroupJoinReq {
                account: account.into(),
                group_id: gid.clone(),
            });
            p
        };

        let dispatcher = Arc::new(RecordingDispatcher::default());
        serve(
            CMD_GROUP_JOIN,
            groups.clone(),
            dispatcher.clone(),
            join(""),
            session("bob", "kim"),
        )
        .await;
        assert_eq!(status_of(&dispatcher), Status::Unauthorized as i32);

        let dispatcher = Arc::new(RecordingDispatcher::default());
        serve(
            CMD_GROUP_JOIN,
            groups.clone(),
            dispatcher.clone(),
            join("bob"),
            alice(),
        )
        .await;
        assert_eq!(status_of(&dispatcher), Status::Unauthorized as i32);

        let dispatcher = Arc::new(RecordingDispatcher::default());
        serve(
            CMD_GROUP_JOIN,
            groups.clone(),
            dispatcher.clone(),
            join(""),
            alice(),
        )
        .await;
        assert_eq!(status_of(&dispatcher), Status::Success as i32);
        assert_eq!(
            groups.members("kim", &gid).await.unwrap(),
            vec!["alice".to_string()]
        );
    }

    #[tokio::test]
    async fn quit_unknown_or_non_member_is_not_group_member() {
        let groups = memory_groups();
        let gid = create_alice_group(groups.clone()).await;
        let quit = |account: &str, dest: &str, channel: &str| {
            let mut p = pkt(CMD_GROUP_QUIT, dest, Bytes::new(), channel);
            p.write_body(&GroupQuitReq {
                account: account.into(),
                group_id: dest.into(),
            });
            p
        };

        let dispatcher = Arc::new(RecordingDispatcher::default());
        serve(
            CMD_GROUP_QUIT,
            groups.clone(),
            dispatcher.clone(),
            quit("", "nope", "ch-alice"),
            alice(),
        )
        .await;
        assert_eq!(status_of(&dispatcher), Status::NotGroupMember as i32);

        let dispatcher = Arc::new(RecordingDispatcher::default());
        serve(
            CMD_GROUP_QUIT,
            groups.clone(),
            dispatcher.clone(),
            quit("", &gid, "ch-bob"),
            session("bob", "kim"),
        )
        .await;
        assert_eq!(status_of(&dispatcher), Status::NotGroupMember as i32);

        let dispatcher = Arc::new(RecordingDispatcher::default());
        serve(
            CMD_GROUP_QUIT,
            groups.clone(),
            dispatcher.clone(),
            quit("bob", &gid, "ch-alice"),
            alice(),
        )
        .await;
        assert_eq!(status_of(&dispatcher), Status::Unauthorized as i32);

        let dispatcher = Arc::new(RecordingDispatcher::default());
        serve(
            CMD_GROUP_QUIT,
            groups.clone(),
            dispatcher.clone(),
            quit("", &gid, "ch-alice"),
            alice(),
        )
        .await;
        assert_eq!(status_of(&dispatcher), Status::Success as i32);
        match groups.members("kim", &gid).await {
            Ok(m) => assert!(!m.contains(&"alice".to_string())),
            Err(GroupError::NotFound) => {}
            other => panic!("unexpected {other:?}"),
        }
    }

    #[tokio::test]
    async fn detail_non_member_is_not_group_member() {
        let groups = memory_groups();
        let gid = create_alice_group(groups.clone()).await;
        let dispatcher = Arc::new(RecordingDispatcher::default());
        serve(
            CMD_GROUP_DETAIL,
            groups.clone(),
            dispatcher.clone(),
            pkt(CMD_GROUP_DETAIL, &gid, Bytes::new(), "ch-bob"),
            session("bob", "kim"),
        )
        .await;
        assert_eq!(status_of(&dispatcher), Status::NotGroupMember as i32);

        let dispatcher = Arc::new(RecordingDispatcher::default());
        serve(
            CMD_GROUP_DETAIL,
            groups,
            dispatcher.clone(),
            pkt(CMD_GROUP_DETAIL, &gid, Bytes::new(), "ch-alice"),
            alice(),
        )
        .await;
        assert_eq!(status_of(&dispatcher), Status::Success as i32);
    }

    #[tokio::test]
    async fn detail_backend_is_system_exception() {
        struct FailDetail;
        #[async_trait]
        impl GroupDirectory for FailDetail {
            async fn create(&self, _app: &str, _req: &CreateGroup) -> Result<String, GroupError> {
                Err(GroupError::Backend("unused".into()))
            }
            async fn members(
                &self,
                _app: &str,
                _group_id: &str,
            ) -> Result<Vec<String>, GroupError> {
                Err(GroupError::Backend("sql down".into()))
            }
            async fn join(
                &self,
                _app: &str,
                _group_id: &str,
                _account: &str,
            ) -> Result<(), GroupError> {
                Err(GroupError::Backend("sql down".into()))
            }
            async fn quit(
                &self,
                _app: &str,
                _group_id: &str,
                _account: &str,
            ) -> Result<(), GroupError> {
                Err(GroupError::Backend("sql down".into()))
            }
            async fn detail(&self, _app: &str, _group_id: &str) -> Result<GroupInfo, GroupError> {
                Err(GroupError::Backend("sql down".into()))
            }
        }

        let dispatcher = Arc::new(RecordingDispatcher::default());
        serve(
            CMD_GROUP_DETAIL,
            Arc::new(FailDetail),
            dispatcher.clone(),
            pkt(CMD_GROUP_DETAIL, "g1", Bytes::new(), "ch-alice"),
            alice(),
        )
        .await;
        assert_eq!(status_of(&dispatcher), Status::SystemException as i32);
    }

    #[tokio::test]
    async fn kim_session_cannot_detail_gray_group() {
        let groups = memory_groups();
        let gray = groups
            .create(
                "kim-gray",
                &CreateGroup {
                    name: "g".into(),
                    avatar: String::new(),
                    introduction: String::new(),
                    owner: "alice".into(),
                    members: vec!["alice".into()],
                },
            )
            .await
            .unwrap();
        let dispatcher = Arc::new(RecordingDispatcher::default());
        serve(
            CMD_GROUP_DETAIL,
            groups,
            dispatcher.clone(),
            pkt(CMD_GROUP_DETAIL, &gray, Bytes::new(), "ch-alice"),
            alice(),
        )
        .await;
        assert_eq!(status_of(&dispatcher), Status::NotGroupMember as i32);
    }

    #[tokio::test]
    async fn members_non_member_has_no_body() {
        let groups = memory_groups();
        let gid = create_alice_group(groups.clone()).await;
        let dispatcher = Arc::new(RecordingDispatcher::default());
        serve(
            CMD_GROUP_MEMBERS,
            groups,
            dispatcher.clone(),
            pkt(CMD_GROUP_MEMBERS, &gid, Bytes::new(), "ch-bob"),
            session("bob", "kim"),
        )
        .await;
        assert_eq!(status_of(&dispatcher), Status::NotGroupMember as i32);
        assert!(dispatcher.recorded()[0].pkt.body.is_empty());
    }
}
