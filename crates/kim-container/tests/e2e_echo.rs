use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use bytes::Bytes;
use kim_container::{Container, ContainerOpts, HashSelector, InnerTcpDialer};
use kim_core::{Acceptor, Agent, Conn, Error, MessageListener, Server, StateListener};
use kim_naming::{DefaultRegistration, StaticNaming};
use kim_protocol::pkt::{Flag, InnerHandshakeReq, Status};
use kim_protocol::{
    marshal, read, read_logic, BasicPkt, LogicPkt, Packet, CMD_DEMO_ECHO, CODE_PONG,
    META_DEST_CHANNELS, META_DEST_SERVER,
};
use kim_tcp::TcpServer;
use kim_ws::{ClientOptions, WsClient, WsIdentityDialer, WsServer};
use prost::Message;
use tokio::sync::Mutex;

struct GatewayHandler {
    container: Arc<Container>,
}

#[async_trait]
impl Acceptor for GatewayHandler {
    async fn accept(&self, conn: &mut dyn Conn, timeout: Duration) -> Result<String, Error> {
        let frame = tokio::time::timeout(timeout, conn.read_frame())
            .await
            .map_err(|_| Error::HandshakeTimeout(timeout))??;
        Ok(String::from_utf8_lossy(&frame.payload).trim().to_string())
    }
}

#[async_trait]
impl MessageListener for GatewayHandler {
    async fn receive(&self, agent: &dyn Agent, payload: Bytes) {
        match read(&payload) {
            Ok(Packet::Basic(p)) if p.code == kim_protocol::CODE_PING => {
                let _ = agent.push(marshal(&Packet::Basic(BasicPkt::pong()))).await;
            }
            Ok(Packet::Logic(mut logic)) => {
                logic.header.channel_id = agent.id().to_string();
                let svc = logic.service_name().to_string();
                if self.container.forward(&svc, logic).await.is_err() {
                    let mut resp = read_logic(&payload).unwrap();
                    resp.header.channel_id = agent.id().to_string();
                    resp.header.flag = Flag::Response as i32;
                    resp.header.status = Status::ServiceUnavailable as i32;
                    let _ = agent.push(marshal(&Packet::Logic(resp))).await;
                }
            }
            _ => {}
        }
    }
}

#[async_trait]
impl StateListener for GatewayHandler {
    async fn disconnect(&self, _channel_id: &str) -> Result<(), Error> {
        Ok(())
    }
}

struct ChatHandler {
    container: Arc<Container>,
    seen: Arc<Mutex<Vec<String>>>,
}

#[async_trait]
impl Acceptor for ChatHandler {
    async fn accept(&self, conn: &mut dyn Conn, timeout: Duration) -> Result<String, Error> {
        let frame = tokio::time::timeout(timeout, conn.read_frame())
            .await
            .map_err(|_| Error::HandshakeTimeout(timeout))??;
        let req = InnerHandshakeReq::decode(frame.payload.as_ref())
            .map_err(|e| Error::Handshake(e.to_string()))?;
        Ok(req.service_id)
    }
}

#[async_trait]
impl MessageListener for ChatHandler {
    async fn receive(&self, _agent: &dyn Agent, payload: Bytes) {
        let mut pkt = match read_logic(&payload) {
            Ok(p) => p,
            Err(_) => {
                self.seen.lock().await.push("basic-or-bad".into());
                return;
            }
        };
        self.seen.lock().await.push(pkt.header.command.clone());
        pkt.header.flag = Flag::Response as i32;
        pkt.header.status = Status::Success as i32;
        let gw = pkt.get_meta(META_DEST_SERVER).unwrap_or("").to_string();
        let ch = pkt.header.channel_id.clone();
        pkt.set_meta(META_DEST_SERVER, &gw);
        pkt.set_meta(META_DEST_CHANNELS, &ch);
        let _ = self.container.push(&gw, pkt).await;
    }
}

#[async_trait]
impl StateListener for ChatHandler {
    async fn disconnect(&self, _channel_id: &str) -> Result<(), Error> {
        Ok(())
    }
}

fn ident(id: &str, name: &str) -> DefaultRegistration {
    DefaultRegistration {
        service_id: id.into(),
        service_name: name.into(),
        protocol: "tcp".into(),
        public_address: String::new(),
        public_port: 0,
        tags: vec![],
        meta: HashMap::new(),
    }
}

#[tokio::test]
async fn ping_stays_on_gateway_echo_roundtrips() {
    let mut chat_server = TcpServer::bind("127.0.0.1:0").await.unwrap();
    let chat_addr = chat_server.local_addr();
    let chat_naming = Arc::new(StaticNaming::from_slice(vec![]));
    let chat_c = Container::new(ContainerOpts {
        naming: chat_naming,
        identity: ident("chat-1", "chat"),
        dialer: Arc::new(InnerTcpDialer {
            local_service_id: "chat-1".into(),
        }),
        deps: vec![],
        adult_delay: Duration::from_millis(0),
        selector: Arc::new(HashSelector),
    });
    let seen = Arc::new(Mutex::new(Vec::new()));
    let chat_h = Arc::new(ChatHandler {
        container: chat_c.clone(),
        seen: seen.clone(),
    });
    chat_server.set_acceptor(chat_h.clone());
    chat_server.set_message_listener(chat_h.clone());
    chat_server.set_state_listener(chat_h);
    chat_c.attach_server(Arc::new(chat_server));
    let chat_run = chat_c.clone();
    tokio::spawn(async move {
        let _ = chat_run.start().await;
    });
    tokio::time::sleep(Duration::from_millis(30)).await;

    let mut gw_server = WsServer::bind("127.0.0.1:0").await.unwrap();
    let gw_addr = gw_server.local_addr();
    let naming = Arc::new(StaticNaming::from_slice(vec![DefaultRegistration {
        service_id: "chat-1".into(),
        service_name: "chat".into(),
        protocol: "tcp".into(),
        public_address: "127.0.0.1".into(),
        public_port: chat_addr.port(),
        tags: vec![],
        meta: HashMap::new(),
    }]));
    let gw_c = Container::new(ContainerOpts {
        naming,
        identity: ident("wg-1", "wgateway"),
        dialer: Arc::new(InnerTcpDialer {
            local_service_id: "wg-1".into(),
        }),
        deps: vec!["chat".into()],
        adult_delay: Duration::from_millis(0),
        selector: Arc::new(HashSelector),
    });
    let gw_h = Arc::new(GatewayHandler {
        container: gw_c.clone(),
    });
    gw_server.set_acceptor(gw_h.clone());
    gw_server.set_message_listener(gw_h.clone());
    gw_server.set_state_listener(gw_h);
    gw_c.attach_server(Arc::new(gw_server));
    let gw_run = gw_c.clone();
    tokio::spawn(async move {
        let _ = gw_run.start().await;
    });
    tokio::time::sleep(Duration::from_millis(80)).await;

    let mut client = WsClient::new(
        "alice",
        "test",
        ClientOptions {
            heartbeat: None,
            ..ClientOptions::default()
        },
    );
    client.set_dialer(Arc::new(WsIdentityDialer));
    client.connect(&format!("ws://{gw_addr}/")).await.unwrap();

    client
        .send(marshal(&Packet::Basic(BasicPkt::ping())))
        .await
        .unwrap();
    let pong = tokio::time::timeout(Duration::from_secs(2), client.read())
        .await
        .unwrap()
        .unwrap();
    match read(&pong.payload).unwrap() {
        Packet::Basic(p) => assert_eq!(p.code, CODE_PONG),
        _ => panic!("pong"),
    }

    let req = LogicPkt::new(CMD_DEMO_ECHO, 3, Bytes::from_static(b"hello pkt"));
    client.send(marshal(&Packet::Logic(req))).await.unwrap();
    let resp = tokio::time::timeout(Duration::from_secs(2), client.read())
        .await
        .unwrap()
        .unwrap();
    match read(&resp.payload).unwrap() {
        Packet::Logic(p) => {
            assert_eq!(p.header.sequence, 3);
            assert_eq!(p.header.flag, Flag::Response as i32);
            assert_eq!(p.header.status, Status::Success as i32);
            assert_eq!(&p.body[..], b"hello pkt");
        }
        _ => panic!("logic"),
    }

    tokio::time::sleep(Duration::from_millis(50)).await;
    let cmds = seen.lock().await.clone();
    assert_eq!(cmds, vec![CMD_DEMO_ECHO.to_string()]);

    let _ = gw_c.shutdown().await;
    let _ = chat_c.shutdown().await;
}

#[tokio::test]
async fn unavailable_without_chat() {
    let mut gw_server = WsServer::bind("127.0.0.1:0").await.unwrap();
    let gw_addr = gw_server.local_addr();
    let naming = Arc::new(StaticNaming::from_slice(vec![DefaultRegistration {
        service_id: "chat-1".into(),
        service_name: "chat".into(),
        protocol: "tcp".into(),
        public_address: "127.0.0.1".into(),
        public_port: 1,
        tags: vec![],
        meta: HashMap::new(),
    }]));
    let gw_c = Container::new(ContainerOpts {
        naming,
        identity: ident("wg-1", "wgateway"),
        dialer: Arc::new(InnerTcpDialer {
            local_service_id: "wg-1".into(),
        }),
        deps: vec!["chat".into()],
        adult_delay: Duration::from_millis(0),
        selector: Arc::new(HashSelector),
    });
    let gw_h = Arc::new(GatewayHandler {
        container: gw_c.clone(),
    });
    gw_server.set_acceptor(gw_h.clone());
    gw_server.set_message_listener(gw_h.clone());
    gw_server.set_state_listener(gw_h);
    gw_c.attach_server(Arc::new(gw_server));
    let gw_run = gw_c.clone();
    tokio::spawn(async move {
        let _ = gw_run.start().await;
    });
    tokio::time::sleep(Duration::from_millis(40)).await;

    let mut client = WsClient::new(
        "alice",
        "test",
        ClientOptions {
            heartbeat: None,
            ..ClientOptions::default()
        },
    );
    client.set_dialer(Arc::new(WsIdentityDialer));
    client.connect(&format!("ws://{gw_addr}/")).await.unwrap();
    let req = LogicPkt::new(CMD_DEMO_ECHO, 1, Bytes::from_static(b"x"));
    client.send(marshal(&Packet::Logic(req))).await.unwrap();
    let resp = tokio::time::timeout(Duration::from_secs(2), client.read())
        .await
        .unwrap()
        .unwrap();
    match read(&resp.payload).unwrap() {
        Packet::Logic(p) => {
            assert_eq!(p.header.status, Status::ServiceUnavailable as i32);
        }
        _ => panic!("logic"),
    }
    let _ = gw_c.shutdown().await;
}
