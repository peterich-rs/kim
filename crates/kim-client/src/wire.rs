use bytes::Bytes;
use kim_core::Frame;
use kim_protocol::pkt::{
    AuthResp, Flag, GroupCreateNotify, KickoutNotify, LoginReq, MessageAckReq, MessagePush,
    MessageReq, MessageResp, Status,
};
use kim_protocol::{
    marshal, read, BasicPkt, LogicPkt, Packet, CMD_CHAT_GROUP_TALK, CMD_CHAT_TALK_ACK,
    CMD_CHAT_USER_TALK, CMD_GROUP_CREATE, CMD_LOGIN_RENEW, CMD_LOGIN_SIGN_IN, CODE_PONG,
    MESSAGE_TYPE_TEXT,
};

use crate::config::DEFAULT_DEVICE;
use crate::events::{Event, IncomingTalk, TalkResult};
use crate::ClientError;

pub fn encode_login(token: &str) -> Bytes {
    let mut pkt = LogicPkt::new(CMD_LOGIN_SIGN_IN, 1, Bytes::new());
    pkt.write_body(&LoginReq {
        token: token.to_string(),
        device: DEFAULT_DEVICE.to_string(),
    });
    marshal(&Packet::Logic(pkt))
}

pub fn encode_ping() -> Bytes {
    marshal(&Packet::Basic(BasicPkt::ping()))
}

pub fn encode_user_talk(seq: u32, dest: &str, body: &str, client_id: &str) -> Bytes {
    let mut pkt = LogicPkt::new(CMD_CHAT_USER_TALK, seq, Bytes::new());
    pkt.set_dest(dest);
    pkt.write_body(&MessageReq {
        r#type: MESSAGE_TYPE_TEXT,
        body: body.to_string(),
        extra: String::new(),
        client_id: client_id.to_string(),
    });
    marshal(&Packet::Logic(pkt))
}

pub fn encode_ack(seq: u32, message_id: i64) -> Bytes {
    let mut pkt = LogicPkt::new(CMD_CHAT_TALK_ACK, seq, Bytes::new());
    pkt.write_body(&MessageAckReq { message_id });
    marshal(&Packet::Logic(pkt))
}

pub fn is_kickout(pkt: &LogicPkt) -> Option<KickoutNotify> {
    if pkt.header.flag != Flag::Push as i32 {
        return None;
    }
    if pkt.header.command != CMD_LOGIN_SIGN_IN {
        return None;
    }
    pkt.read_body().ok()
}

pub fn decode_event(frame: &Frame) -> Result<Event, ClientError> {
    match read(&frame.payload)? {
        Packet::Basic(p) if p.code == CODE_PONG => Ok(Event::Pong),
        Packet::Basic(_) => Err(ClientError::other("unexpected basic packet")),
        Packet::Logic(p) => decode_logic(p),
    }
}

fn decode_logic(p: LogicPkt) -> Result<Event, ClientError> {
    if let Some(notify) = is_kickout(&p) {
        return Ok(Event::Kickout {
            channel_id: notify.channel_id,
        });
    }
    if p.header.flag == Flag::Push as i32 && p.header.command == CMD_LOGIN_RENEW {
        let body: AuthResp = p.read_body()?;
        return Ok(Event::TokenRenew {
            token: body.token,
            exp: body.exp,
        });
    }
    if p.header.flag == Flag::Push as i32 && p.header.command == CMD_GROUP_CREATE {
        let n: GroupCreateNotify = p.read_body()?;
        return Ok(Event::GroupCreate {
            group_id: n.group_id,
            members: n.members,
        });
    }
    if p.header.flag == Flag::Push as i32
        && (p.header.command == CMD_CHAT_USER_TALK || p.header.command == CMD_CHAT_GROUP_TALK)
    {
        let push: MessagePush = p.read_body()?;
        return Ok(Event::Talk(IncomingTalk {
            command: p.header.command,
            message_id: push.message_id,
            sender: push.sender,
            msg_type: push.r#type,
            body: push.body,
            extra: push.extra,
            send_time: push.send_time,
        }));
    }
    if p.header.flag == Flag::Response as i32
        && (p.header.command == CMD_CHAT_USER_TALK || p.header.command == CMD_CHAT_GROUP_TALK)
    {
        if p.header.status != Status::Success as i32 {
            return Ok(Event::Status {
                command: p.header.command,
                status: p.header.status,
                sequence: p.header.sequence,
            });
        }
        let resp: MessageResp = p.read_body()?;
        return Ok(Event::TalkResp(TalkResult {
            message_id: resp.message_id,
            send_time: resp.send_time,
            sequence: p.header.sequence,
        }));
    }
    Ok(Event::Status {
        command: p.header.command,
        status: p.header.status,
        sequence: p.header.sequence,
    })
}
