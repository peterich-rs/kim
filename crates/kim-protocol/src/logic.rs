use bytes::{BufMut, Bytes, BytesMut};
use prost::Message;

use crate::error::ProtocolError;
use crate::magic::MAGIC_LOGIC_PKT;
use crate::pkt::Header;
use crate::wire::service_name;

#[derive(Clone)]
pub struct LogicPkt {
    pub header: Header,
    pub body: Bytes,
}

impl LogicPkt {
    pub fn new(command: impl Into<String>, sequence: u32, body: Bytes) -> Self {
        Self {
            header: Header {
                command: command.into(),
                sequence,
                body_length: body.len() as u32,
                ..Header::default()
            },
            body,
        }
    }

    /// Copy `header` for a response or push; body is empty and `body_length` is 0.
    pub fn new_from(header: &Header) -> Self {
        let mut header = header.clone();
        header.body_length = 0;
        Self {
            header,
            body: Bytes::new(),
        }
    }

    pub fn write_body(&mut self, msg: &impl Message) {
        let buf = msg.encode_to_vec();
        self.header.body_length = buf.len() as u32;
        self.body = Bytes::from(buf);
    }

    pub fn read_body<T: Message + Default>(&self) -> Result<T, ProtocolError> {
        T::decode(self.body.as_ref()).map_err(ProtocolError::from)
    }

    pub fn service_name(&self) -> &str {
        service_name(&self.header.command)
    }

    pub fn set_meta(&mut self, key: &str, value: &str) {
        self.header.meta.retain(|m| m.key != key);
        self.header.meta.push(crate::pkt::Meta {
            key: key.to_string(),
            value: value.to_string(),
        });
    }

    pub fn get_meta(&self, key: &str) -> Option<&str> {
        self.header
            .meta
            .iter()
            .find(|m| m.key == key)
            .map(|m| m.value.as_str())
    }

    pub fn del_meta(&mut self, key: &str) {
        self.header.meta.retain(|m| m.key != key);
    }

    pub fn set_dest(&mut self, dest: impl Into<String>) {
        self.header.dest = dest.into();
    }

    pub fn encode(&self) -> Bytes {
        let header = self.header.encode_to_vec();
        let mut buf = BytesMut::with_capacity(8 + header.len() + self.body.len());
        buf.extend_from_slice(&MAGIC_LOGIC_PKT);
        buf.put_u32(header.len() as u32);
        buf.extend_from_slice(&header);
        buf.extend_from_slice(&self.body);
        buf.freeze()
    }

    pub fn decode(rest: &[u8]) -> Result<Self, ProtocolError> {
        if rest.len() < 4 {
            return Err(ProtocolError::Incomplete);
        }
        let header_len = u32::from_be_bytes([rest[0], rest[1], rest[2], rest[3]]) as usize;
        if rest.len() < 4 + header_len {
            return Err(ProtocolError::Incomplete);
        }
        let header = Header::decode(&rest[4..4 + header_len])?;
        let body_start = 4 + header_len;
        let need = header.body_length as usize;
        if rest.len() < body_start + need {
            return Err(ProtocolError::Incomplete);
        }
        if rest.len() > body_start + need {
            tracing_optional_extra(rest.len() - (body_start + need));
        }
        let body = Bytes::copy_from_slice(&rest[body_start..body_start + need]);
        Ok(Self { header, body })
    }
}

fn tracing_optional_extra(_n: usize) {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{read, Packet};

    #[test]
    fn logic_roundtrip() {
        let pkt = LogicPkt::new("chat.demo.echo", 7, Bytes::from_static(b"hi"));
        let bytes = pkt.encode();
        match read(&bytes).unwrap() {
            Packet::Logic(got) => {
                assert_eq!(got.header.command, "chat.demo.echo");
                assert_eq!(got.header.sequence, 7);
                assert_eq!(got.header.body_length, 2);
                assert_eq!(&got.body[..], b"hi");
            }
            _ => panic!("expected logic"),
        }
    }

    #[test]
    fn incomplete_header() {
        assert!(matches!(
            LogicPkt::decode(&[0, 0, 0, 10]),
            Err(ProtocolError::Incomplete)
        ));
    }

    #[test]
    fn meta_replace() {
        let mut pkt = LogicPkt::new("chat.demo.echo", 1, Bytes::new());
        pkt.set_meta("dest.server", "a");
        pkt.set_meta("dest.server", "b");
        assert_eq!(pkt.get_meta("dest.server"), Some("b"));
        assert_eq!(
            pkt.header
                .meta
                .iter()
                .filter(|m| m.key == "dest.server")
                .count(),
            1
        );
        pkt.del_meta("dest.server");
        assert!(pkt.get_meta("dest.server").is_none());
    }

    #[test]
    fn new_from_and_body_roundtrip() {
        let mut pkt = LogicPkt::new("login.signin", 9, Bytes::new());
        pkt.header.channel_id = "wg-1_alice_1".into();
        pkt.write_body(&crate::pkt::LoginReq {
            token: "tok".into(),
            device: String::new(),
        });
        let got: crate::pkt::LoginReq = pkt.read_body().unwrap();
        assert_eq!(got.token, "tok");
        assert_eq!(pkt.header.body_length as usize, pkt.body.len());

        let resp = LogicPkt::new_from(&pkt.header);
        assert_eq!(resp.header.command, "login.signin");
        assert_eq!(resp.header.sequence, 9);
        assert_eq!(resp.header.channel_id, "wg-1_alice_1");
        assert_eq!(resp.header.body_length, 0);
        assert!(resp.body.is_empty());
        let cloned = pkt.clone();
        assert_eq!(cloned.header.command, pkt.header.command);
    }

    #[test]
    fn set_dest_survives_marshal_read() {
        let mut pkt = LogicPkt::new("chat.user.talk", 1, Bytes::new());
        pkt.set_dest("alice");
        assert_eq!(pkt.header.dest, "alice");
        match read(&pkt.encode()).unwrap() {
            Packet::Logic(got) => assert_eq!(got.header.dest, "alice"),
            _ => panic!("expected logic"),
        }
    }

    #[test]
    fn message_req_resp_push_roundtrip() {
        let mut req_pkt = LogicPkt::new("chat.user.talk", 1, Bytes::new());
        req_pkt.write_body(&crate::pkt::MessageReq {
            r#type: crate::MESSAGE_TYPE_TEXT,
            body: "hello".into(),
            extra: "x".into(),
            client_id: String::new(),
        });
        let req: crate::pkt::MessageReq = req_pkt.read_body().unwrap();
        assert_eq!(req.r#type, crate::MESSAGE_TYPE_TEXT);
        assert_eq!(req.body, "hello");
        assert_eq!(req.extra, "x");

        let mut resp_pkt = LogicPkt::new("chat.user.talk", 1, Bytes::new());
        resp_pkt.write_body(&crate::pkt::MessageResp {
            message_id: 42,
            send_time: 99,
        });
        let resp: crate::pkt::MessageResp = resp_pkt.read_body().unwrap();
        assert_eq!(resp.message_id, 42);
        assert_eq!(resp.send_time, 99);

        let mut push_pkt = LogicPkt::new("chat.user.talk", 1, Bytes::new());
        push_pkt.write_body(&crate::pkt::MessagePush {
            message_id: 42,
            r#type: crate::MESSAGE_TYPE_IMAGE,
            body: "img".into(),
            extra: "e".into(),
            sender: "bob".into(),
            send_time: 100,
        });
        let push: crate::pkt::MessagePush = push_pkt.read_body().unwrap();
        assert_eq!(push.message_id, 42);
        assert_eq!(push.r#type, crate::MESSAGE_TYPE_IMAGE);
        assert_eq!(push.body, "img");
        assert_eq!(push.extra, "e");
        assert_eq!(push.sender, "bob");
        assert_eq!(push.send_time, 100);
    }

    #[test]
    fn ack_and_offline_roundtrip() {
        let mut ack_pkt = LogicPkt::new("chat.talk.ack", 1, Bytes::new());
        ack_pkt.write_body(&crate::pkt::MessageAckReq { message_id: 7 });
        let ack: crate::pkt::MessageAckReq = ack_pkt.read_body().unwrap();
        assert_eq!(ack.message_id, 7);

        let mut idx_req = LogicPkt::new("chat.offline.index", 1, Bytes::new());
        idx_req.write_body(&crate::pkt::MessageIndexReq { message_id: 0 });
        let got: crate::pkt::MessageIndexReq = idx_req.read_body().unwrap();
        assert_eq!(got.message_id, 0);

        let mut idx_resp = LogicPkt::new("chat.offline.index", 1, Bytes::new());
        idx_resp.write_body(&crate::pkt::MessageIndexResp {
            indexes: vec![crate::pkt::MessageIndex {
                message_id: 8,
                direction: 0,
                send_time: 11,
                account_b: "alice".into(),
                group: String::new(),
            }],
        });
        let idx: crate::pkt::MessageIndexResp = idx_resp.read_body().unwrap();
        assert_eq!(idx.indexes.len(), 1);
        assert_eq!(idx.indexes[0].message_id, 8);

        let mut content_req = LogicPkt::new("chat.offline.content", 1, Bytes::new());
        content_req.write_body(&crate::pkt::MessageContentReq {
            message_ids: vec![8, 9],
            ..Default::default()
        });
        let creq: crate::pkt::MessageContentReq = content_req.read_body().unwrap();
        assert_eq!(creq.message_ids, vec![8, 9]);

        let mut content_resp = LogicPkt::new("chat.offline.content", 1, Bytes::new());
        content_resp.write_body(&crate::pkt::MessageContentResp {
            messages: vec![crate::pkt::Message {
                message_id: 8,
                r#type: crate::MESSAGE_TYPE_TEXT,
                body: "hi".into(),
                extra: String::new(),
            }],
        });
        let cresp: crate::pkt::MessageContentResp = content_resp.read_body().unwrap();
        assert_eq!(cresp.messages[0].body, "hi");
    }

    #[test]
    fn group_join_and_notify_roundtrip() {
        let mut join = LogicPkt::new("chat.group.join", 1, Bytes::new());
        join.write_body(&crate::pkt::GroupJoinReq {
            account: "bob".into(),
            group_id: "G1".into(),
        });
        let got: crate::pkt::GroupJoinReq = join.read_body().unwrap();
        assert_eq!(got.account, "bob");

        let mut n = LogicPkt::new("chat.group.create", 1, Bytes::new());
        n.write_body(&crate::pkt::GroupCreateNotify {
            group_id: "G1".into(),
            members: vec!["alice".into(), "bob".into()],
        });
        let notify: crate::pkt::GroupCreateNotify = n.read_body().unwrap();
        assert_eq!(notify.members.len(), 2);
    }
}
