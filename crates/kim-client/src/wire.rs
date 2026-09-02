use bytes::Bytes;
use kim_core::Frame;
use kim_protocol::pkt::{
    AuthResp, Flag, FriendRequestNotify, GroupCreateNotify, HistoryReq, HistoryResp, InboxReq,
    InboxResp, KickoutNotify, LoginReq, MessageAckReq, MessageContentReq, MessageContentResp,
    MessageIndexReq, MessageIndexResp, MessagePush, MessageReq, MessageResp, Status, UserListResp,
    UserProfile, UserProfileUpdate, UserSearchReq, UserSearchResp,
};
use kim_protocol::{
    marshal, read, BasicPkt, LogicPkt, Packet, CMD_CHAT_GROUP_TALK, CMD_CHAT_TALK_ACK,
    CMD_CHAT_USER_TALK, CMD_FRIEND_INCOMING, CMD_FRIEND_LIST, CMD_FRIEND_REQUEST, CMD_GROUP_CREATE,
    CMD_HISTORY, CMD_INBOX_LIST, CMD_LOGIN_RENEW, CMD_LOGIN_SIGN_IN, CMD_OFFLINE_CONTENT,
    CMD_OFFLINE_INDEX, CMD_USER_PROFILE, CMD_USER_SEARCH, CMD_USER_UPDATE, CODE_PONG,
    INBOX_KIND_GROUP, MESSAGE_TYPE_IMAGE, MESSAGE_TYPE_TEXT, MESSAGE_TYPE_VIDEO,
    MESSAGE_TYPE_VOICE,
};

use crate::config::DEFAULT_DEVICE;
use crate::events::{
    Event, HistoryItem, InboxItem, IncomingTalk, Message, MessageIndex, OutgoingContent, Profile,
    TalkResult,
};
use crate::ClientError;

pub fn encode_login(token: &str) -> Bytes {
    let mut pkt = LogicPkt::new(CMD_LOGIN_SIGN_IN, 1, Bytes::new());
    pkt.write_body(&LoginReq {
        token: token.to_string(),
        device: DEFAULT_DEVICE.to_string(),
        ..Default::default()
    });
    marshal(&Packet::Logic(pkt))
}

pub fn encode_ping() -> Bytes {
    marshal(&Packet::Basic(BasicPkt::ping()))
}

pub fn encode_outgoing(
    seq: u32,
    dest: &str,
    kind: i32,
    content: &OutgoingContent,
    client_id: &str,
) -> Bytes {
    let command = if kind == INBOX_KIND_GROUP {
        CMD_CHAT_GROUP_TALK
    } else {
        CMD_CHAT_USER_TALK
    };
    let (msg_type, body, extra) = match content {
        OutgoingContent::Text(text) => (MESSAGE_TYPE_TEXT, text.as_str(), ""),
        OutgoingContent::Image { url, extra } => (MESSAGE_TYPE_IMAGE, url.as_str(), extra.as_str()),
        OutgoingContent::Voice { url, extra } => (MESSAGE_TYPE_VOICE, url.as_str(), extra.as_str()),
        OutgoingContent::Video { url, extra } => (MESSAGE_TYPE_VIDEO, url.as_str(), extra.as_str()),
    };
    let mut pkt = LogicPkt::new(command, seq, Bytes::new());
    pkt.set_dest(dest);
    pkt.write_body(&MessageReq {
        r#type: msg_type,
        body: body.to_string(),
        extra: extra.to_string(),
        client_id: client_id.to_string(),
    });
    marshal(&Packet::Logic(pkt))
}

pub fn encode_user_talk(seq: u32, dest: &str, body: &str, client_id: &str) -> Bytes {
    encode_outgoing(
        seq,
        dest,
        kim_protocol::INBOX_KIND_USER,
        &OutgoingContent::Text(body.to_string()),
        client_id,
    )
}

pub fn encode_user_talk_typed(
    seq: u32,
    dest: &str,
    msg_type: i32,
    body: &str,
    extra: &str,
    client_id: &str,
) -> Bytes {
    let content = match msg_type {
        MESSAGE_TYPE_IMAGE => OutgoingContent::Image {
            url: body.to_string(),
            extra: extra.to_string(),
        },
        MESSAGE_TYPE_VOICE => OutgoingContent::Voice {
            url: body.to_string(),
            extra: extra.to_string(),
        },
        MESSAGE_TYPE_VIDEO => OutgoingContent::Video {
            url: body.to_string(),
            extra: extra.to_string(),
        },
        _ => OutgoingContent::Text(body.to_string()),
    };
    encode_outgoing(
        seq,
        dest,
        kim_protocol::INBOX_KIND_USER,
        &content,
        client_id,
    )
}

pub fn encode_user_image(seq: u32, dest: &str, url: &str, extra: &str, client_id: &str) -> Bytes {
    encode_outgoing(
        seq,
        dest,
        kim_protocol::INBOX_KIND_USER,
        &OutgoingContent::Image {
            url: url.to_string(),
            extra: extra.to_string(),
        },
        client_id,
    )
}

pub fn encode_ack(seq: u32, message_id: i64) -> Bytes {
    let mut pkt = LogicPkt::new(CMD_CHAT_TALK_ACK, seq, Bytes::new());
    pkt.write_body(&MessageAckReq {
        message_id,
        ..Default::default()
    });
    marshal(&Packet::Logic(pkt))
}

pub fn encode_ack_batch(seq: u32, message_ids: &[i64]) -> Bytes {
    let mut pkt = LogicPkt::new(CMD_CHAT_TALK_ACK, seq, Bytes::new());
    pkt.write_body(&MessageAckReq {
        message_ids: message_ids.to_vec(),
        ..Default::default()
    });
    marshal(&Packet::Logic(pkt))
}

pub fn encode_inbox_list(seq: u32, limit: i32) -> Bytes {
    let mut pkt = LogicPkt::new(CMD_INBOX_LIST, seq, Bytes::new());
    pkt.write_body(&InboxReq { limit });
    marshal(&Packet::Logic(pkt))
}

pub fn encode_history(seq: u32, dest: &str, kind: i32, before_id: i64, limit: i32) -> Bytes {
    let mut pkt = LogicPkt::new(CMD_HISTORY, seq, Bytes::new());
    pkt.set_dest(dest);
    pkt.write_body(&HistoryReq {
        before_id,
        limit,
        kind,
    });
    marshal(&Packet::Logic(pkt))
}

pub fn encode_offline_index(seq: u32) -> Bytes {
    let mut pkt = LogicPkt::new(CMD_OFFLINE_INDEX, seq, Bytes::new());
    pkt.write_body(&MessageIndexReq {
        message_id: 0,
        resume: true,
    });
    marshal(&Packet::Logic(pkt))
}

pub fn encode_offline_content(seq: u32, ids: &[i64]) -> Bytes {
    let mut pkt = LogicPkt::new(CMD_OFFLINE_CONTENT, seq, Bytes::new());
    pkt.write_body(&MessageContentReq {
        message_ids: ids.to_vec(),
        account: String::new(),
        app: String::new(),
    });
    marshal(&Packet::Logic(pkt))
}

pub fn encode_dest_cmd(command: &str, seq: u32, dest: &str) -> Bytes {
    let mut pkt = LogicPkt::new(command, seq, Bytes::new());
    pkt.set_dest(dest);
    marshal(&Packet::Logic(pkt))
}

pub fn encode_empty_cmd(command: &str, seq: u32) -> Bytes {
    marshal(&Packet::Logic(LogicPkt::new(command, seq, Bytes::new())))
}

pub fn encode_user_update(seq: u32, nickname: &str, avatar: &str, bio: &str) -> Bytes {
    let mut pkt = LogicPkt::new(CMD_USER_UPDATE, seq, Bytes::new());
    pkt.write_body(&UserProfileUpdate {
        nickname: nickname.to_string(),
        avatar: avatar.to_string(),
        bio: bio.to_string(),
    });
    marshal(&Packet::Logic(pkt))
}

pub fn encode_user_search(seq: u32, query: &str) -> Bytes {
    let mut pkt = LogicPkt::new(CMD_USER_SEARCH, seq, Bytes::new());
    pkt.write_body(&UserSearchReq {
        query: query.to_string(),
    });
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
    if p.header.flag == Flag::Push as i32 && p.header.command == CMD_FRIEND_REQUEST {
        let n: FriendRequestNotify = p.read_body()?;
        return Ok(Event::FriendRequest {
            from: n.from_account,
            nickname: n.from_nickname,
        });
    }
    if p.header.flag == Flag::Push as i32
        && (p.header.command == CMD_CHAT_USER_TALK || p.header.command == CMD_CHAT_GROUP_TALK)
    {
        let command = p.header.command.clone();
        let header_dest = p.header.dest.clone();
        let push: MessagePush = p.read_body()?;
        let dest = if command == CMD_CHAT_GROUP_TALK {
            header_dest
        } else {
            push.sender.clone()
        };
        return Ok(Event::Talk(IncomingTalk {
            command,
            dest,
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
    if p.header.flag == Flag::Response as i32
        && (p.header.command == CMD_FRIEND_LIST
            || p.header.command == CMD_FRIEND_INCOMING
            || p.header.command == CMD_USER_SEARCH)
    {
        if p.header.status != Status::Success as i32 {
            return Ok(Event::Status {
                command: p.header.command,
                status: p.header.status,
                sequence: p.header.sequence,
            });
        }
        let users = if p.header.command == CMD_USER_SEARCH {
            let resp: UserSearchResp = p.read_body()?;
            resp.users
        } else {
            let resp: UserListResp = p.read_body()?;
            resp.users
        };
        return Ok(Event::UserList {
            command: p.header.command,
            sequence: p.header.sequence,
            users: users
                .into_iter()
                .map(|u| Profile::from_wire(u.account, u.nickname, u.avatar))
                .collect(),
        });
    }
    if p.header.flag == Flag::Response as i32
        && (p.header.command == CMD_USER_PROFILE || p.header.command == CMD_USER_UPDATE)
    {
        if p.header.status != Status::Success as i32 {
            return Ok(Event::Status {
                command: p.header.command,
                status: p.header.status,
                sequence: p.header.sequence,
            });
        }
        let u: UserProfile = p.read_body()?;
        return Ok(Event::Profile {
            sequence: p.header.sequence,
            profile: Profile::from_wire(u.account, u.nickname, u.avatar),
        });
    }
    if p.header.flag == Flag::Response as i32 && p.header.command == CMD_INBOX_LIST {
        if p.header.status != Status::Success as i32 {
            return Ok(Event::Status {
                command: p.header.command,
                status: p.header.status,
                sequence: p.header.sequence,
            });
        }
        let resp: InboxResp = p.read_body()?;
        return Ok(Event::Inbox {
            sequence: p.header.sequence,
            items: resp
                .items
                .into_iter()
                .map(|i| InboxItem {
                    dest: i.dest,
                    kind: i.kind,
                    title: i.title,
                    avatar: i.avatar,
                    last_body: i.last_body,
                    last_sender: i.last_sender,
                    last_message_id: i.last_message_id,
                    last_send_time: i.last_send_time,
                    unread: i.unread,
                })
                .collect(),
        });
    }
    if p.header.flag == Flag::Response as i32 && p.header.command == CMD_HISTORY {
        if p.header.status != Status::Success as i32 {
            return Ok(Event::Status {
                command: p.header.command,
                status: p.header.status,
                sequence: p.header.sequence,
            });
        }
        let dest = p.header.dest.clone();
        let resp: HistoryResp = p.read_body()?;
        return Ok(Event::History {
            sequence: p.header.sequence,
            dest,
            messages: resp
                .messages
                .into_iter()
                .map(|m| HistoryItem {
                    message_id: m.message_id,
                    msg_type: m.r#type,
                    body: m.body,
                    extra: m.extra,
                    sender: m.sender,
                    send_time: m.send_time,
                    direction: m.direction,
                })
                .collect(),
        });
    }
    if p.header.flag == Flag::Response as i32 && p.header.command == CMD_OFFLINE_INDEX {
        if p.header.status != Status::Success as i32 {
            return Ok(Event::Status {
                command: p.header.command,
                status: p.header.status,
                sequence: p.header.sequence,
            });
        }
        let resp: MessageIndexResp = p.read_body()?;
        return Ok(Event::OfflinePage {
            sequence: p.header.sequence,
            indexes: resp
                .indexes
                .into_iter()
                .map(|i| MessageIndex {
                    message_id: i.message_id,
                    direction: i.direction,
                    send_time: i.send_time,
                    account_b: i.account_b,
                    group: i.group,
                })
                .collect(),
            has_more: resp.has_more,
        });
    }
    if p.header.flag == Flag::Response as i32 && p.header.command == CMD_OFFLINE_CONTENT {
        if p.header.status != Status::Success as i32 {
            return Ok(Event::Status {
                command: p.header.command,
                status: p.header.status,
                sequence: p.header.sequence,
            });
        }
        let resp: MessageContentResp = p.read_body()?;
        return Ok(Event::OfflineContent {
            sequence: p.header.sequence,
            messages: resp
                .messages
                .into_iter()
                .map(|m| Message {
                    message_id: m.message_id,
                    msg_type: m.r#type,
                    body: m.body,
                    extra: m.extra,
                })
                .collect(),
        });
    }
    Ok(Event::Status {
        command: p.header.command,
        status: p.header.status,
        sequence: p.header.sequence,
    })
}
