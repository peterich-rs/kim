use std::collections::VecDeque;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;

use async_trait::async_trait;
use bytes::Bytes;
use kim_core::{
    Acceptor, ChannelHandle, Conn, Error as CoreError, Frame, MessageListener, OpCode, Server,
    StateListener,
};
use kim_protocol::pkt::{
    Flag, HistoryItem as ProtoHistory, HistoryReq, HistoryResp, InboxItem as ProtoInbox, InboxReq,
    InboxResp, KickoutNotify, LoginReq, LoginResp, Message as PktMessage, MessageAckReq,
    MessageContentReq, MessageContentResp, MessageIndex as ProtoIndex, MessageIndexReq,
    MessageIndexResp, MessagePush, MessageReq, MessageResp, Status, UserListResp, UserProfile,
};
use kim_protocol::{
    generate, marshal, read, BasicPkt, LogicPkt, Packet, CMD_CHAT_GROUP_TALK, CMD_CHAT_TALK_ACK,
    CMD_CHAT_USER_TALK, CMD_FRIEND_LIST, CMD_FRIEND_REQUEST, CMD_HISTORY, CMD_INBOX_LIST,
    CMD_LOGIN_SIGN_IN, CMD_OFFLINE_CONTENT, CMD_OFFLINE_INDEX, CODE_PING, DEMO_DEFAULT_SECRET,
    INBOX_KIND_GROUP, INBOX_KIND_USER, MESSAGE_TYPE_IMAGE, MESSAGE_TYPE_TEXT,
};
use kim_ws::WsServer;

use crate::client::KimClient;
use crate::config::{ClientConfig, DEFAULT_DEVICE, DEFAULT_LOCAL_URL, DEFAULT_PROD_URL};
use crate::events::{Event, OutgoingContent};
use crate::login::login_on_conn;
use crate::session::MemorySession;
use crate::supervisor::{LinkState, SessionEvent, SessionSupervisor};
use crate::sync::{ConfirmGate, SyncEngine};
use crate::token::account_from_token;
use crate::wire::{
    decode_event, encode_ack, encode_ack_batch, encode_dest_cmd, encode_history, encode_inbox_list,
    encode_offline_content, encode_offline_index, encode_outgoing, encode_ping, encode_user_image,
    encode_user_talk, is_kickout,
};
use crate::ClientError;

struct MockConn {
    incoming: VecDeque<Frame>,
    outgoing: Vec<Frame>,
}

impl MockConn {
    fn with_incoming(frames: Vec<Frame>) -> Self {
        Self {
            incoming: frames.into(),
            outgoing: Vec::new(),
        }
    }
}

#[async_trait]
impl Conn for MockConn {
    async fn read_frame(&mut self) -> Result<Frame, CoreError> {
        self.incoming.pop_front().ok_or(CoreError::Closed)
    }

    async fn write_frame(&mut self, opcode: OpCode, payload: Bytes) -> Result<(), CoreError> {
        self.outgoing.push(Frame { opcode, payload });
        Ok(())
    }

    async fn flush(&mut self) -> Result<(), CoreError> {
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<(), CoreError> {
        Ok(())
    }
}

fn mint(acc: &str) -> String {
    generate(DEMO_DEFAULT_SECRET, acc, "kim", 4_000_000_000).unwrap()
}

fn login_resp(channel_id: &str, status: Status) -> Frame {
    let mut pkt = LogicPkt::new(CMD_LOGIN_SIGN_IN, 1, Bytes::new());
    pkt.header.flag = Flag::Response as i32;
    pkt.header.status = status as i32;
    pkt.write_body(&LoginResp {
        channel_id: channel_id.into(),
    });
    Frame::binary(marshal(&Packet::Logic(pkt)))
}

#[tokio::test]
async fn login_first_frame_is_signin_jwt_not_url() {
    let token = mint("alice");
    let mut conn = MockConn::with_incoming(vec![login_resp("wg-1_alice_1", Status::Success)]);
    let session = login_on_conn(&mut conn, &token, Duration::from_secs(1))
        .await
        .unwrap();
    assert_eq!(session.account, "alice");
    assert_eq!(session.channel_id, "wg-1_alice_1");
    assert_eq!(conn.outgoing.len(), 1);
    assert_eq!(conn.outgoing[0].opcode, OpCode::Binary);
    match read(&conn.outgoing[0].payload).unwrap() {
        Packet::Logic(p) => {
            assert_eq!(p.header.command, CMD_LOGIN_SIGN_IN);
            assert_eq!(p.header.sequence, 1);
            let req: LoginReq = p.read_body().unwrap();
            assert_eq!(req.token, token);
            assert_eq!(req.device, DEFAULT_DEVICE);
        }
        _ => panic!("expected logic login.signin"),
    }
}

#[tokio::test]
async fn login_rejects_unauthorized_and_plain_alice_id() {
    let token = mint("alice");
    let mut bad = MockConn::with_incoming(vec![login_resp("wg-1_alice_1", Status::Unauthorized)]);
    let err = login_on_conn(&mut bad, &token, Duration::from_secs(1))
        .await
        .unwrap_err();
    assert!(matches!(err, ClientError::Unauthorized));

    let mut alice = MockConn::with_incoming(vec![login_resp("alice", Status::Success)]);
    assert!(login_on_conn(&mut alice, &token, Duration::from_secs(1))
        .await
        .is_err());
}

#[tokio::test]
async fn talk_and_ping_over_conn() {
    let token = mint("alice");
    let ping_pong = Frame::binary(marshal(&Packet::Basic(BasicPkt::pong())));
    let mut talk_resp = LogicPkt::new(CMD_CHAT_USER_TALK, 2, Bytes::new());
    talk_resp.header.flag = Flag::Response as i32;
    talk_resp.header.status = Status::Success as i32;
    talk_resp.write_body(&MessageResp {
        message_id: 10001,
        send_time: 2000,
    });
    let talk_frame = Frame::binary(marshal(&Packet::Logic(talk_resp)));
    let conn = MockConn::with_incoming(vec![
        login_resp("wg-1_alice_1", Status::Success),
        ping_pong,
        talk_frame,
    ]);
    let mut login_conn = MockConn::with_incoming(vec![login_resp("wg-1_alice_1", Status::Success)]);
    let session = login_on_conn(&mut login_conn, &token, Duration::from_secs(1))
        .await
        .unwrap();
    let client = KimClient::with_conn(ClientConfig::local(token.clone()), Box::new(conn));
    client.force_session(session);
    client.ping().await.unwrap();
    let result = client.talk_to_user("bob", "hello").await.unwrap();
    assert_eq!(result.message_id, 10001);
    assert_eq!(result.send_time, 2000);
    assert_eq!(result.sequence, 2);
}

#[test]
fn talk_packet_sets_dest_and_client_id() {
    let bytes = encode_user_talk(2, "bob", "hello world", "cid-1");
    match read(&bytes).unwrap() {
        Packet::Logic(p) => {
            assert_eq!(p.header.command, CMD_CHAT_USER_TALK);
            assert_eq!(p.header.dest, "bob");
            assert_eq!(p.header.sequence, 2);
            let req: MessageReq = p.read_body().unwrap();
            assert_eq!(req.body, "hello world");
            assert_eq!(req.r#type, MESSAGE_TYPE_TEXT);
            assert_eq!(req.client_id, "cid-1");
        }
        _ => panic!("expected logic"),
    }
}

#[test]
fn encode_ack_keeps_single_id_field() {
    match read(&encode_ack(3, 11)).unwrap() {
        Packet::Logic(p) => {
            assert_eq!(p.header.command, CMD_CHAT_TALK_ACK);
            let req: MessageAckReq = p.read_body().unwrap();
            assert_eq!(req.message_id, 11);
            assert!(req.message_ids.is_empty());
        }
        _ => panic!("expected logic"),
    }
}

#[test]
fn encode_ack_batch_fills_message_ids() {
    match read(&encode_ack_batch(4, &[10, 11])).unwrap() {
        Packet::Logic(p) => {
            let req: MessageAckReq = p.read_body().unwrap();
            assert_eq!(req.message_id, 0);
            assert_eq!(req.message_ids, vec![10, 11]);
        }
        _ => panic!("expected logic"),
    }
}

#[test]
fn image_packet_uses_type_2_and_url_body() {
    let bytes = encode_user_image(
        2,
        "bob",
        "https://media.kim.ainexc.com/a.jpg",
        "w=1",
        "cid-2",
    );
    match read(&bytes).unwrap() {
        Packet::Logic(p) => {
            let req: MessageReq = p.read_body().unwrap();
            assert_eq!(req.r#type, MESSAGE_TYPE_IMAGE);
            assert_eq!(req.body, "https://media.kim.ainexc.com/a.jpg");
            assert_eq!(req.extra, "w=1");
            assert_eq!(req.client_id, "cid-2");
        }
        _ => panic!("expected logic"),
    }
}

#[test]
fn dest_cmd_sets_header_dest() {
    let bytes = encode_dest_cmd(CMD_FRIEND_REQUEST, 3, "bob");
    match read(&bytes).unwrap() {
        Packet::Logic(p) => {
            assert_eq!(p.header.command, CMD_FRIEND_REQUEST);
            assert_eq!(p.header.dest, "bob");
            assert_eq!(p.header.sequence, 3);
        }
        _ => panic!("expected logic"),
    }
}

fn logged_in(token: String, incoming: Vec<Frame>) -> KimClient {
    let client = KimClient::with_conn(
        ClientConfig::local(token.clone()),
        Box::new(MockConn::with_incoming(incoming)),
    );
    client.force_session(MemorySession {
        channel_id: "wg-1_alice_1".into(),
        account: "alice".into(),
        token,
    });
    client
}

#[tokio::test]
async fn friend_list_returns_profiles() {
    let mut pkt = LogicPkt::new(CMD_FRIEND_LIST, 2, Bytes::new());
    pkt.header.flag = Flag::Response as i32;
    pkt.write_body(&UserListResp {
        users: vec![UserProfile {
            account: "bob".into(),
            nickname: "Bobby".into(),
            avatar: String::new(),
            bio: String::new(),
        }],
    });
    let client = logged_in(
        mint("alice"),
        vec![Frame::binary(marshal(&Packet::Logic(pkt)))],
    );
    let users = client.friend_list().await.unwrap();
    assert_eq!(users.len(), 1);
    assert_eq!(users[0].account, "bob");
    assert_eq!(users[0].nickname, "Bobby");
    assert_eq!(users[0].avatar, "");
}

#[tokio::test]
async fn profile_and_update_round_trip() {
    use kim_protocol::{CMD_USER_PROFILE, CMD_USER_UPDATE};

    let mut get = LogicPkt::new(CMD_USER_PROFILE, 2, Bytes::new());
    get.header.flag = Flag::Response as i32;
    get.write_body(&UserProfile {
        account: "alice".into(),
        nickname: "Ali".into(),
        avatar: "https://media.kim.ainexc.com/alice/a.jpg".into(),
        bio: String::new(),
    });
    let mut upd = LogicPkt::new(CMD_USER_UPDATE, 3, Bytes::new());
    upd.header.flag = Flag::Response as i32;
    upd.write_body(&UserProfile {
        account: "alice".into(),
        nickname: "Ali".into(),
        avatar: "https://media.kim.ainexc.com/alice/b.jpg".into(),
        bio: String::new(),
    });
    let client = logged_in(
        mint("alice"),
        vec![
            Frame::binary(marshal(&Packet::Logic(get))),
            Frame::binary(marshal(&Packet::Logic(upd))),
        ],
    );
    let me = client.profile("").await.unwrap();
    assert_eq!(me.avatar, "https://media.kim.ainexc.com/alice/a.jpg");
    let next = client
        .update_profile("Ali", "https://media.kim.ainexc.com/alice/b.jpg", "")
        .await
        .unwrap();
    assert_eq!(next.avatar, "https://media.kim.ainexc.com/alice/b.jpg");
}

#[tokio::test]
async fn talk_not_friends_is_status_109() {
    let mut pkt = LogicPkt::new(CMD_CHAT_USER_TALK, 2, Bytes::new());
    pkt.header.flag = Flag::Response as i32;
    pkt.header.status = Status::NotFriends as i32;
    let client = logged_in(
        mint("alice"),
        vec![Frame::binary(marshal(&Packet::Logic(pkt)))],
    );
    let err = client.talk_to_user("bob", "hello").await.unwrap_err();
    assert!(matches!(err, ClientError::Status(109)));
}

#[test]
fn ping_is_basic_8_bytes() {
    assert_eq!(encode_ping().len(), 8);
}

#[test]
fn kickout_is_signin_push_not_login_resp() {
    let mut resp = LogicPkt::new(CMD_LOGIN_SIGN_IN, 1, Bytes::new());
    resp.header.flag = Flag::Response as i32;
    resp.write_body(&LoginResp {
        channel_id: "wg-1_alice_1".into(),
    });
    assert!(is_kickout(&resp).is_none());

    let mut kick = LogicPkt::new(CMD_LOGIN_SIGN_IN, 0, Bytes::new());
    kick.header.flag = Flag::Push as i32;
    kick.write_body(&KickoutNotify {
        channel_id: "wg-1_alice_1".into(),
    });
    assert_eq!(is_kickout(&kick).unwrap().channel_id, "wg-1_alice_1");
}

#[test]
fn decode_talk_push() {
    let mut pkt = LogicPkt::new(CMD_CHAT_USER_TALK, 0, Bytes::new());
    pkt.header.flag = Flag::Push as i32;
    pkt.write_body(&MessagePush {
        message_id: 42,
        r#type: MESSAGE_TYPE_TEXT,
        body: "hi".into(),
        extra: String::new(),
        sender: "bob".into(),
        send_time: 9,
    });
    match decode_event(&Frame::binary(marshal(&Packet::Logic(pkt)))).unwrap() {
        Event::Talk(t) => {
            assert_eq!(t.sender, "bob");
            assert_eq!(t.dest, "bob");
            assert_eq!(t.body, "hi");
            assert_eq!(t.message_id, 42);
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn decode_group_talk_push_uses_header_dest() {
    use kim_protocol::CMD_CHAT_GROUP_TALK;
    let mut pkt = LogicPkt::new(CMD_CHAT_GROUP_TALK, 0, Bytes::new());
    pkt.header.flag = Flag::Push as i32;
    pkt.header.dest = "g1".into();
    pkt.write_body(&MessagePush {
        message_id: 7,
        r#type: MESSAGE_TYPE_TEXT,
        body: "hi all".into(),
        extra: String::new(),
        sender: "bob".into(),
        send_time: 1,
    });
    match decode_event(&Frame::binary(marshal(&Packet::Logic(pkt)))).unwrap() {
        Event::Talk(t) => {
            assert_eq!(t.dest, "g1");
            assert_eq!(t.sender, "bob");
            assert_eq!(t.body, "hi all");
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn default_urls_and_session() {
    assert_eq!(DEFAULT_LOCAL_URL, "ws://127.0.0.1:8001/");
    assert_eq!(DEFAULT_PROD_URL, "wss://kim.ainexc.com/");
    assert_eq!(account_from_token(&mint("carol")).unwrap(), "carol");
    assert!(!MemorySession::default().is_logged_in());
}

struct FakeGw {
    seq: StdMutex<u64>,
}

#[async_trait]
impl Acceptor for FakeGw {
    async fn accept(&self, conn: &mut dyn Conn, timeout: Duration) -> Result<String, CoreError> {
        let frame = tokio::time::timeout(timeout, conn.read_frame())
            .await
            .map_err(|_| CoreError::HandshakeTimeout(timeout))??;
        let pkt = match read(&frame.payload) {
            Ok(Packet::Logic(p)) => p,
            _ => return Err(CoreError::Handshake("expected login.signin".into())),
        };
        if pkt.header.command != CMD_LOGIN_SIGN_IN {
            return Err(CoreError::Handshake("invalid command".into()));
        }
        let req: LoginReq = pkt
            .read_body()
            .map_err(|e| CoreError::Handshake(e.to_string()))?;
        let acc =
            account_from_token(&req.token).map_err(|e| CoreError::Handshake(e.to_string()))?;
        let n = {
            let mut g = self.seq.lock().unwrap_or_else(|e| e.into_inner());
            *g += 1;
            *g
        };
        let id = format!("wg-test_{acc}_{n}");
        let mut resp = LogicPkt::new(CMD_LOGIN_SIGN_IN, pkt.header.sequence, Bytes::new());
        resp.header.flag = Flag::Response as i32;
        resp.header.status = Status::Success as i32;
        resp.write_body(&LoginResp {
            channel_id: id.clone(),
        });
        conn.write_frame(OpCode::Binary, marshal(&Packet::Logic(resp)))
            .await?;
        Ok(id)
    }
}

#[async_trait]
impl MessageListener for FakeGw {
    async fn receive(&self, handle: &dyn ChannelHandle, payload: Bytes) {
        match read(&payload) {
            Ok(Packet::Basic(p)) if p.code == CODE_PING => {
                let _ = handle.push(marshal(&Packet::Basic(BasicPkt::pong()))).await;
            }
            Ok(Packet::Logic(p)) if p.header.command == CMD_CHAT_USER_TALK => {
                let mut resp = LogicPkt::new_from(&p.header);
                resp.header.flag = Flag::Response as i32;
                resp.header.status = Status::Success as i32;
                resp.write_body(&MessageResp {
                    message_id: 10001,
                    send_time: 2_000,
                });
                let _ = handle.push(marshal(&Packet::Logic(resp))).await;
                let mut push = LogicPkt::new(CMD_CHAT_USER_TALK, 0, Bytes::new());
                push.header.flag = Flag::Push as i32;
                push.write_body(&MessagePush {
                    message_id: 20001,
                    r#type: MESSAGE_TYPE_TEXT,
                    body: "from-bob".into(),
                    extra: String::new(),
                    sender: "bob".into(),
                    send_time: 3_000,
                });
                let _ = handle.push(marshal(&Packet::Logic(push))).await;
            }
            _ => {}
        }
    }
}

#[async_trait]
impl StateListener for FakeGw {
    async fn disconnect(&self, _channel_id: &str) -> Result<(), CoreError> {
        Ok(())
    }
}

#[tokio::test]
async fn loopback_ws_login_ping_talk() {
    let handler = Arc::new(FakeGw {
        seq: StdMutex::new(0),
    });
    let mut server = WsServer::bind("127.0.0.1:0").await.unwrap();
    server.set_drain_wait(Duration::from_millis(50));
    server.set_acceptor(handler.clone());
    server.set_message_listener(handler.clone());
    server.set_state_listener(handler);
    let addr = server.local_addr();
    let server = Arc::new(server);
    let running = server.clone();
    tokio::spawn(async move {
        running.start().await.unwrap();
    });
    tokio::time::sleep(Duration::from_millis(50)).await;

    let token = mint("alice");
    let url = format!("ws://{addr}/");
    let client = Arc::new(KimClient::new(ClientConfig::new(url, token)));
    client.connect().await.unwrap();
    let session = client.login().await.unwrap();
    assert!(session.channel_id.starts_with("wg-test_alice_"));
    assert_eq!(session.account, "alice");
    client.ping().await.unwrap();

    let waiting = client.clone();
    let recv_task = tokio::spawn(async move { waiting.recv().await });
    tokio::task::yield_now().await;

    let talk = client.talk_to_user("bob", "hello").await.unwrap();
    assert_eq!(talk.message_id, 10001);

    let event = tokio::time::timeout(Duration::from_secs(2), recv_task)
        .await
        .expect("recv timed out")
        .expect("recv task")
        .expect("recv event");
    match event {
        Event::Talk(t) => {
            assert_eq!(t.sender, "bob");
            assert_eq!(t.dest, "bob");
            assert_eq!(t.body, "from-bob");
            assert_eq!(t.message_id, 20001);
        }
        other => panic!("{other:?}"),
    }

    client.disconnect().await.unwrap();
    let _ = server.shutdown().await;
}

struct SharedConn {
    incoming: Arc<StdMutex<VecDeque<Frame>>>,
    outgoing: Arc<StdMutex<Vec<Frame>>>,
}

#[async_trait]
impl Conn for SharedConn {
    async fn read_frame(&mut self) -> Result<Frame, CoreError> {
        self.incoming
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .pop_front()
            .ok_or(CoreError::Closed)
    }

    async fn write_frame(&mut self, opcode: OpCode, payload: Bytes) -> Result<(), CoreError> {
        self.outgoing
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(Frame { opcode, payload });
        Ok(())
    }

    async fn flush(&mut self) -> Result<(), CoreError> {
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<(), CoreError> {
        Ok(())
    }
}

fn resp_logic(command: &str, seq: u32, write: impl FnOnce(&mut LogicPkt)) -> Frame {
    let mut pkt = LogicPkt::new(command, seq, Bytes::new());
    pkt.header.flag = Flag::Response as i32;
    pkt.header.status = Status::Success as i32;
    write(&mut pkt);
    Frame::binary(marshal(&Packet::Logic(pkt)))
}

fn logged_in_shared(incoming: Vec<Frame>) -> (KimClient, Arc<StdMutex<Vec<Frame>>>) {
    let outgoing = Arc::new(StdMutex::new(Vec::new()));
    let conn = SharedConn {
        incoming: Arc::new(StdMutex::new(incoming.into())),
        outgoing: outgoing.clone(),
    };
    let token = mint("alice");
    let client = KimClient::with_conn(ClientConfig::local(token.clone()), Box::new(conn));
    client.force_session(MemorySession {
        channel_id: "wg-1_alice_1".into(),
        account: "alice".into(),
        token,
    });
    (client, outgoing)
}

#[test]
fn encode_inbox_list_writes_limit() {
    match read(&encode_inbox_list(4, 200)).unwrap() {
        Packet::Logic(p) => {
            assert_eq!(p.header.command, CMD_INBOX_LIST);
            assert_eq!(p.header.sequence, 4);
            assert!(p.header.dest.is_empty());
            let req: InboxReq = p.read_body().unwrap();
            assert_eq!(req.limit, 200);
        }
        _ => panic!("expected logic"),
    }
}

#[test]
fn encode_history_sets_dest_and_kind() {
    match read(&encode_history(5, "bob", INBOX_KIND_USER, 10, 50)).unwrap() {
        Packet::Logic(p) => {
            assert_eq!(p.header.command, CMD_HISTORY);
            assert_eq!(p.header.dest, "bob");
            let req: HistoryReq = p.read_body().unwrap();
            assert_eq!(req.before_id, 10);
            assert_eq!(req.limit, 50);
            assert_eq!(req.kind, INBOX_KIND_USER);
        }
        _ => panic!("expected logic"),
    }
}

#[test]
fn encode_offline_index_resume_true() {
    match read(&encode_offline_index(6)).unwrap() {
        Packet::Logic(p) => {
            assert_eq!(p.header.command, CMD_OFFLINE_INDEX);
            let req: MessageIndexReq = p.read_body().unwrap();
            assert_eq!(req.message_id, 0);
            assert!(req.resume);
        }
        _ => panic!("expected logic"),
    }
}

#[test]
fn encode_offline_content_leaves_account_app_empty() {
    match read(&encode_offline_content(7, &[1, 2])).unwrap() {
        Packet::Logic(p) => {
            assert_eq!(p.header.command, CMD_OFFLINE_CONTENT);
            let req: MessageContentReq = p.read_body().unwrap();
            assert_eq!(req.message_ids, vec![1, 2]);
            assert!(req.account.is_empty());
            assert!(req.app.is_empty());
        }
        _ => panic!("expected logic"),
    }
}

#[test]
fn encode_outgoing_group_uses_group_talk() {
    let bytes = encode_outgoing(
        8,
        "g1",
        INBOX_KIND_GROUP,
        &OutgoingContent::Text("hi".into()),
        "cid-stable",
    );
    match read(&bytes).unwrap() {
        Packet::Logic(p) => {
            assert_eq!(p.header.command, CMD_CHAT_GROUP_TALK);
            assert_eq!(p.header.dest, "g1");
            let req: MessageReq = p.read_body().unwrap();
            assert_eq!(req.body, "hi");
            assert_eq!(req.client_id, "cid-stable");
        }
        _ => panic!("expected logic"),
    }
}

#[tokio::test]
async fn inbox_history_offline_round_trip() {
    let inbox = resp_logic(CMD_INBOX_LIST, 2, |p| {
        p.write_body(&InboxResp {
            items: vec![ProtoInbox {
                dest: "bob".into(),
                kind: INBOX_KIND_USER,
                title: "Bobby".into(),
                avatar: String::new(),
                last_body: "yo".into(),
                last_sender: "bob".into(),
                last_message_id: 9,
                last_send_time: 1,
                unread: 2,
            }],
        });
    });
    let mut hist = resp_logic(CMD_HISTORY, 3, |p| {
        p.write_body(&HistoryResp {
            messages: vec![ProtoHistory {
                message_id: 9,
                r#type: MESSAGE_TYPE_TEXT,
                body: "yo".into(),
                extra: String::new(),
                sender: "bob".into(),
                send_time: 1,
                direction: 0,
            }],
        });
    });
    if let Packet::Logic(mut p) = read(&hist.payload).unwrap() {
        p.header.dest = "bob".into();
        hist = Frame::binary(marshal(&Packet::Logic(p)));
    }
    let index = resp_logic(CMD_OFFLINE_INDEX, 4, |p| {
        p.write_body(&MessageIndexResp {
            indexes: vec![ProtoIndex {
                message_id: 9,
                direction: 0,
                send_time: 1,
                account_b: "bob".into(),
                group: String::new(),
            }],
            has_more: false,
        });
    });
    let content = resp_logic(CMD_OFFLINE_CONTENT, 5, |p| {
        p.write_body(&MessageContentResp {
            messages: vec![PktMessage {
                message_id: 9,
                r#type: MESSAGE_TYPE_TEXT,
                body: "yo".into(),
                extra: String::new(),
            }],
        });
    });
    let client = logged_in(mint("alice"), vec![inbox, hist, index, content]);
    let items = client.inbox_list(200).await.unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].dest, "bob");
    assert_eq!(items[0].unread, 2);
    let history = client.history("bob", INBOX_KIND_USER, 0, 50).await.unwrap();
    assert_eq!(history[0].body, "yo");
    let idx = client.offline_index().await.unwrap();
    assert_eq!(idx[0].message_id, 9);
    let msgs = client.offline_content(&[9]).await.unwrap();
    assert_eq!(msgs[0].body, "yo");
}

#[tokio::test]
async fn send_message_keeps_caller_client_id() {
    let talk = resp_logic(CMD_CHAT_USER_TALK, 2, |p| {
        p.write_body(&MessageResp {
            message_id: 42,
            send_time: 7,
        });
    });
    let (client, outgoing) = logged_in_shared(vec![talk]);
    let result = client
        .send_message(
            "bob",
            INBOX_KIND_USER,
            OutgoingContent::Text("hello".into()),
            "stable-cid",
        )
        .await
        .unwrap();
    assert_eq!(result.message_id, 42);
    let frames = outgoing.lock().unwrap_or_else(|e| e.into_inner()).clone();
    match read(&frames[0].payload).unwrap() {
        Packet::Logic(p) => {
            let req: MessageReq = p.read_body().unwrap();
            assert_eq!(req.client_id, "stable-cid");
        }
        _ => panic!("expected logic"),
    }
}

#[tokio::test]
async fn sync_confirm_gate_blocks_ack() {
    let inbox = resp_logic(CMD_INBOX_LIST, 2, |p| {
        p.write_body(&InboxResp { items: vec![] });
    });
    let index = resp_logic(CMD_OFFLINE_INDEX, 3, |p| {
        p.write_body(&MessageIndexResp {
            indexes: vec![ProtoIndex {
                message_id: 11,
                direction: 0,
                send_time: 1,
                account_b: "bob".into(),
                group: String::new(),
            }],
            has_more: false,
        });
    });
    let content = resp_logic(CMD_OFFLINE_CONTENT, 4, |p| {
        p.write_body(&MessageContentResp {
            messages: vec![PktMessage {
                message_id: 11,
                r#type: MESSAGE_TYPE_TEXT,
                body: "later".into(),
                extra: String::new(),
            }],
        });
    });
    let empty_index = resp_logic(CMD_OFFLINE_INDEX, 6, |p| {
        p.write_body(&MessageIndexResp {
            indexes: vec![],
            has_more: false,
        });
    });
    let (client, outgoing) = logged_in_shared(vec![inbox, index, content, empty_index]);
    let (tx, mut rx) = tokio::sync::broadcast::channel(16);
    let gate = ConfirmGate::new();
    let stop = tokio::sync::Notify::new();
    let run = tokio::spawn({
        let gate = gate.clone();
        async move {
            let mut engine = SyncEngine::new();
            engine.run(&client, &tx, &gate, &stop).await
        }
    });
    let mut talks = 0usize;
    loop {
        let ev = tokio::time::timeout(Duration::from_secs(2), rx.recv())
            .await
            .expect("event")
            .expect("recv");
        match ev {
            SessionEvent::Talk(_) => talks += 1,
            SessionEvent::SyncProgress {
                page_pending: true, ..
            } => break,
            _ => {}
        }
    }
    assert_eq!(talks, 1);
    let frames = outgoing.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let acks = frames.iter().filter(|f| {
        matches!(read(&f.payload), Ok(Packet::Logic(p)) if p.header.command == CMD_CHAT_TALK_ACK)
    }).count();
    assert_eq!(acks, 0, "ack must wait for sync_confirm");
    gate.confirm(11);
    let pulled = tokio::time::timeout(Duration::from_secs(2), run)
        .await
        .expect("join")
        .expect("task")
        .expect("sync");
    assert_eq!(pulled, 1);
    let frames = outgoing.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let ack = frames
        .iter()
        .find_map(|f| match read(&f.payload) {
            Ok(Packet::Logic(p)) if p.header.command == CMD_CHAT_TALK_ACK => Some(p),
            _ => None,
        })
        .expect("ack after confirm");
    let req: MessageAckReq = ack.read_body().unwrap();
    assert_eq!(req.message_ids, vec![11]);
}

struct DropGw {
    accepts: AtomicU32,
}

#[async_trait]
impl Acceptor for DropGw {
    async fn accept(&self, conn: &mut dyn Conn, timeout: Duration) -> Result<String, CoreError> {
        self.accepts.fetch_add(1, Ordering::SeqCst);
        let frame = tokio::time::timeout(timeout, conn.read_frame())
            .await
            .map_err(|_| CoreError::HandshakeTimeout(timeout))??;
        let pkt = match read(&frame.payload) {
            Ok(Packet::Logic(p)) => p,
            _ => return Err(CoreError::Handshake("expected login.signin".into())),
        };
        let req: LoginReq = pkt
            .read_body()
            .map_err(|e| CoreError::Handshake(e.to_string()))?;
        let acc =
            account_from_token(&req.token).map_err(|e| CoreError::Handshake(e.to_string()))?;
        let n = self.accepts.load(Ordering::SeqCst);
        let id = format!("wg-drop_{acc}_{n}");
        let mut resp = LogicPkt::new(CMD_LOGIN_SIGN_IN, pkt.header.sequence, Bytes::new());
        resp.header.flag = Flag::Response as i32;
        resp.header.status = Status::Success as i32;
        resp.write_body(&LoginResp {
            channel_id: id.clone(),
        });
        conn.write_frame(OpCode::Binary, marshal(&Packet::Logic(resp)))
            .await?;
        Ok(id)
    }

    async fn on_channel_ready(&self, _channel_id: &str) -> Result<(), CoreError> {
        Err(CoreError::other("drop after login"))
    }
}

#[async_trait]
impl MessageListener for DropGw {
    async fn receive(&self, _handle: &dyn ChannelHandle, _payload: Bytes) {}
}

#[async_trait]
impl StateListener for DropGw {
    async fn disconnect(&self, _channel_id: &str) -> Result<(), CoreError> {
        Ok(())
    }
}

#[tokio::test]
async fn supervisor_reconnects_after_drop() {
    let handler = Arc::new(DropGw {
        accepts: AtomicU32::new(0),
    });
    let mut server = WsServer::bind("127.0.0.1:0").await.unwrap();
    server.set_acceptor(handler.clone());
    server.set_message_listener(handler.clone());
    server.set_state_listener(handler.clone());
    let addr = server.local_addr();
    let server = Arc::new(server);
    let running = server.clone();
    tokio::spawn(async move {
        running.start().await.unwrap();
    });
    tokio::time::sleep(Duration::from_millis(50)).await;

    let token = mint("alice");
    let url = format!("ws://{addr}/");
    let mut cfg = ClientConfig::new(url, token);
    cfg.handshake_timeout = Duration::from_secs(2);
    let sup = SessionSupervisor::start(cfg);
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        if handler.accepts.load(Ordering::SeqCst) >= 2 {
            break;
        }
        if tokio::time::Instant::now() > deadline {
            panic!(
                "reconnect did not happen, accepts={}",
                handler.accepts.load(Ordering::SeqCst)
            );
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    sup.stop();
    let _ = server.shutdown().await;
}

#[tokio::test]
async fn supervisor_radio_up_retries_immediately() {
    let token = mint("alice");
    let mut cfg = ClientConfig::new("ws://127.0.0.1:1/", token);
    cfg.handshake_timeout = Duration::from_millis(200);
    let sup = SessionSupervisor::start(cfg);
    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
    loop {
        if matches!(sup.state(), LinkState::Reconnecting { attempt } if attempt >= 1) {
            break;
        }
        if tokio::time::Instant::now() > deadline {
            panic!("never entered reconnecting, state={:?}", sup.state());
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let mut rx = sup.events();
    let start = tokio::time::Instant::now();
    sup.notify_radio_up();
    loop {
        let ev = tokio::time::timeout(Duration::from_millis(400), rx.recv())
            .await
            .expect("radio retry")
            .expect("recv");
        if matches!(ev, SessionEvent::Link(LinkState::Connecting)) {
            break;
        }
    }
    assert!(
        start.elapsed() < Duration::from_millis(400),
        "radio up should not wait out backoff"
    );
    sup.stop();
}

#[tokio::test]
async fn supervisor_stops_on_expired_token() {
    let token = generate(DEMO_DEFAULT_SECRET, "alice", "kim", 1).unwrap();
    let mut cfg = ClientConfig::new("ws://127.0.0.1:1/", token);
    cfg.handshake_timeout = Duration::from_millis(200);
    let sup = SessionSupervisor::start(cfg);
    let mut rx = sup.events();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    let mut saw_auth = false;
    loop {
        if matches!(sup.state(), LinkState::Reconnecting { .. }) {
            panic!("expired token must not reconnect");
        }
        if matches!(sup.state(), LinkState::Online) {
            panic!("expired token must not go online");
        }
        match tokio::time::timeout(Duration::from_millis(50), rx.recv()).await {
            Ok(Ok(SessionEvent::AuthFailed { .. })) => saw_auth = true,
            Ok(Ok(SessionEvent::Link(LinkState::Reconnecting { .. }))) => {
                panic!("expired token must not reconnect");
            }
            _ => {}
        }
        if saw_auth && matches!(sup.state(), LinkState::Offline) {
            break;
        }
        if tokio::time::Instant::now() > deadline {
            panic!(
                "expired token did not stop, state={:?} auth={saw_auth}",
                sup.state()
            );
        }
    }
    sup.stop();
}

struct RejectGw {
    accepts: AtomicU32,
}

#[async_trait]
impl Acceptor for RejectGw {
    async fn accept(&self, conn: &mut dyn Conn, timeout: Duration) -> Result<String, CoreError> {
        self.accepts.fetch_add(1, Ordering::SeqCst);
        let frame = tokio::time::timeout(timeout, conn.read_frame())
            .await
            .map_err(|_| CoreError::HandshakeTimeout(timeout))??;
        let pkt = match read(&frame.payload) {
            Ok(Packet::Logic(p)) => p,
            _ => return Err(CoreError::Handshake("expected login.signin".into())),
        };
        let mut resp = LogicPkt::new(CMD_LOGIN_SIGN_IN, pkt.header.sequence, Bytes::new());
        resp.header.flag = Flag::Response as i32;
        resp.header.status = Status::Unauthorized as i32;
        conn.write_frame(OpCode::Binary, marshal(&Packet::Logic(resp)))
            .await?;
        Err(CoreError::Handshake("unauthorized".into()))
    }
}

#[async_trait]
impl MessageListener for RejectGw {
    async fn receive(&self, _handle: &dyn ChannelHandle, _payload: Bytes) {}
}

#[async_trait]
impl StateListener for RejectGw {
    async fn disconnect(&self, _channel_id: &str) -> Result<(), CoreError> {
        Ok(())
    }
}

#[tokio::test]
async fn supervisor_stops_on_unauthorized_login() {
    let handler = Arc::new(RejectGw {
        accepts: AtomicU32::new(0),
    });
    let mut server = WsServer::bind("127.0.0.1:0").await.unwrap();
    server.set_acceptor(handler.clone());
    server.set_message_listener(handler.clone());
    server.set_state_listener(handler.clone());
    let addr = server.local_addr();
    let server = Arc::new(server);
    let running = server.clone();
    tokio::spawn(async move {
        running.start().await.unwrap();
    });
    tokio::time::sleep(Duration::from_millis(50)).await;

    let token = mint("alice");
    let url = format!("ws://{addr}/");
    let mut cfg = ClientConfig::new(url, token);
    cfg.handshake_timeout = Duration::from_secs(2);
    let sup = SessionSupervisor::start(cfg);
    let mut rx = sup.events();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
    let mut saw_auth = false;
    loop {
        if handler.accepts.load(Ordering::SeqCst) > 1 {
            panic!("unauthorized login must not reconnect");
        }
        if matches!(sup.state(), LinkState::Reconnecting { .. }) {
            panic!("unauthorized login must not reconnect");
        }
        match tokio::time::timeout(Duration::from_millis(50), rx.recv()).await {
            Ok(Ok(SessionEvent::AuthFailed { .. })) => saw_auth = true,
            _ => {}
        }
        if saw_auth
            && matches!(sup.state(), LinkState::Offline)
            && handler.accepts.load(Ordering::SeqCst) == 1
        {
            break;
        }
        if tokio::time::Instant::now() > deadline {
            panic!(
                "unauthorized login did not stop, state={:?} accepts={} auth={saw_auth}",
                sup.state(),
                handler.accepts.load(Ordering::SeqCst)
            );
        }
    }
    sup.stop();
    let _ = server.shutdown().await;
}
