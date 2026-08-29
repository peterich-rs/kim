mod basic;
mod error;
mod logic;
mod magic;
mod token;
mod wire;

pub mod pkt {
    include!(concat!(env!("OUT_DIR"), "/kim.pkt.rs"));
}

pub use basic::{BasicPkt, CODE_PING, CODE_PONG};
pub use error::ProtocolError;
pub use logic::LogicPkt;
pub use magic::{Magic, MAGIC_BASIC_PKT, MAGIC_LOGIC_PKT};
pub use token::{
    generate, generate_with_jti, parse, token_revoke_key, Claims, DEMO_DEFAULT_SECRET,
};
pub use wire::{
    service_name, CMD_BLOCK_ADD, CMD_BLOCK_LIST, CMD_BLOCK_REMOVE, CMD_CHAT_GROUP_TALK,
    CMD_CHAT_TALK_ACK, CMD_CHAT_USER_TALK, CMD_DEMO_ECHO, CMD_FRIEND_ACCEPT, CMD_FRIEND_INCOMING,
    CMD_FRIEND_LIST, CMD_FRIEND_REJECT, CMD_FRIEND_REMOVE, CMD_FRIEND_REQUEST, CMD_GROUP_CREATE,
    CMD_GROUP_DETAIL, CMD_GROUP_JOIN, CMD_GROUP_MEMBERS, CMD_GROUP_QUIT, CMD_HISTORY,
    CMD_INBOX_LIST, CMD_INBOX_READ, CMD_LOGIN_RENEW, CMD_LOGIN_SIGN_IN, CMD_LOGIN_SIGN_OUT,
    CMD_OFFLINE_CONTENT, CMD_OFFLINE_INDEX, CMD_USER_PROFILE, CMD_USER_SEARCH, CMD_USER_UPDATE,
    INBOX_KIND_GROUP, INBOX_KIND_USER, MESSAGE_TYPE_IMAGE, MESSAGE_TYPE_TEXT, MESSAGE_TYPE_VIDEO,
    MESSAGE_TYPE_VOICE, META_ACCOUNT, META_APP, META_DEST_CHANNELS, META_DEST_SERVER, SN_CHAT,
    SN_LOGIN, SN_ROYAL, SN_TGATEWAY, SN_WGATEWAY,
};

use bytes::Bytes;

pub enum Packet {
    Basic(BasicPkt),
    Logic(LogicPkt),
}

pub fn read(buf: &[u8]) -> Result<Packet, ProtocolError> {
    if buf.len() < 4 {
        return Err(ProtocolError::Incomplete);
    }
    let magic = [buf[0], buf[1], buf[2], buf[3]];
    let rest = &buf[4..];
    if magic == MAGIC_BASIC_PKT {
        Ok(Packet::Basic(BasicPkt::decode(rest)?))
    } else if magic == MAGIC_LOGIC_PKT {
        Ok(Packet::Logic(LogicPkt::decode(rest)?))
    } else {
        Err(ProtocolError::BadMagic)
    }
}

pub fn marshal(pkt: &Packet) -> Bytes {
    match pkt {
        Packet::Basic(p) => p.encode(),
        Packet::Logic(p) => p.encode(),
    }
}

pub fn read_logic(buf: &[u8]) -> Result<LogicPkt, ProtocolError> {
    match read(buf)? {
        Packet::Logic(p) => Ok(p),
        Packet::Basic(_) => Err(ProtocolError::NotLogic),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bad_magic() {
        assert!(matches!(read(&[1, 2, 3, 4]), Err(ProtocolError::BadMagic)));
    }

    #[test]
    fn status_numbers_stable() {
        use pkt::Status;
        assert_eq!(Status::Success as i32, 0);
        assert_eq!(Status::InvalidPacket as i32, 1);
        assert_eq!(Status::CommandNotFound as i32, 2);
        assert_eq!(Status::ServiceUnavailable as i32, 3);
        assert_eq!(Status::SystemException as i32, 99);
        assert_eq!(Status::InvalidPacketBody as i32, 101);
        assert_eq!(Status::InvalidCommand as i32, 103);
        assert_eq!(Status::Unauthorized as i32, 105);
        assert_eq!(Status::ContentBlocked as i32, 106);
        assert_eq!(Status::NotGroupMember as i32, 107);
        assert_eq!(Status::UserNotFound as i32, 108);
        assert_eq!(Status::NotFriends as i32, 109);
        assert_eq!(Status::Blocked as i32, 110);
        assert_eq!(Status::NoDestination as i32, 300);
        assert_eq!(Status::SessionNotFound as i32, 404);
        assert!(Status::try_from(100).is_err());
        assert!(Status::try_from(300).is_ok());
    }
}
