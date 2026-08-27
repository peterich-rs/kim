mod basic;
mod error;
mod logic;
mod magic;
mod wire;

pub mod pkt {
    include!(concat!(env!("OUT_DIR"), "/kim.pkt.rs"));
}

pub use basic::{BasicPkt, CODE_PING, CODE_PONG};
pub use error::ProtocolError;
pub use logic::LogicPkt;
pub use magic::{Magic, MAGIC_BASIC_PKT, MAGIC_LOGIC_PKT};
pub use wire::{
    service_name, CMD_DEMO_ECHO, META_DEST_CHANNELS, META_DEST_SERVER, SN_CHAT, SN_WGATEWAY,
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
}
