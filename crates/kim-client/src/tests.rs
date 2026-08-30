use std::collections::VecDeque;
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;

use async_trait::async_trait;
use bytes::Bytes;
use kim_core::{
    Acceptor, Agent, Conn, Error as CoreError, Frame, MessageListener, OpCode, Server,
    StateListener,
};
use kim_protocol::pkt::{
    Flag, KickoutNotify, LoginReq, LoginResp, MessagePush, MessageReq, MessageResp, Status,
    UserListResp, UserProfile,
};
use kim_protocol::{
    generate, marshal, read, BasicPkt, LogicPkt, Packet, CMD_CHAT_USER_TALK, CMD_FRIEND_LIST,
    CMD_FRIEND_REQUEST, CMD_LOGIN_SIGN_IN, CODE_PING, DEMO_DEFAULT_SECRET, MESSAGE_TYPE_IMAGE,
    MESSAGE_TYPE_TEXT,
};
use kim_ws::WsServer;

use crate::client::KimClient;
use crate::config::{ClientConfig, DEFAULT_DEVICE, DEFAULT_LOCAL_URL, DEFAULT_PROD_URL};
use crate::events::Event;
use crate::login::login_on_conn;
use crate::session::MemorySession;
use crate::token::account_from_token;
use crate::wire::{
    decode_event, encode_dest_cmd, encode_ping, encode_user_image, encode_user_talk, is_kickout,
};

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
    assert!(err.to_string().contains("105") || err.to_string().contains("status"));

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
    assert!(matches!(err, crate::ClientError::Status(109)));
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
    async fn receive(&self, agent: &dyn Agent, payload: Bytes) {
        match read(&payload) {
            Ok(Packet::Basic(p)) if p.code == CODE_PING => {
                let _ = agent.push(marshal(&Packet::Basic(BasicPkt::pong()))).await;
            }
            Ok(Packet::Logic(p)) if p.header.command == CMD_CHAT_USER_TALK => {
                let mut resp = LogicPkt::new_from(&p.header);
                resp.header.flag = Flag::Response as i32;
                resp.header.status = Status::Success as i32;
                resp.write_body(&MessageResp {
                    message_id: 10001,
                    send_time: 2_000,
                });
                let _ = agent.push(marshal(&Packet::Logic(resp))).await;
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
                let _ = agent.push(marshal(&Packet::Logic(push))).await;
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
