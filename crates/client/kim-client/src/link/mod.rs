//! Link-control policy: drop reasons, backoff, session machine.

mod backoff;
pub(crate) mod machine;

pub(crate) use backoff::next_backoff;

use std::sync::atomic::AtomicU8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DropReason {
    ConnectFail,
    HandshakeTimeout,
    ReadError,
    WriteTimeout,
    Closed,
    Decode,
    IdleTimeout,
    ProbeFail,
    ConfirmTimeout,
    SyncFailed,
    Kickout,
    AuthFailed,
    Stop,
}

impl DropReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ConnectFail => "connect-fail",
            Self::HandshakeTimeout => "handshake-timeout",
            Self::ReadError => "read-error",
            Self::WriteTimeout => "write-timeout",
            Self::Closed => "closed",
            Self::Decode => "decode",
            Self::IdleTimeout => "idle-timeout",
            Self::ProbeFail => "probe-fail",
            Self::ConfirmTimeout => "confirm-timeout",
            Self::SyncFailed => "sync-failed",
            Self::Kickout => "kickout",
            Self::AuthFailed => "auth-failed",
            Self::Stop => "stop",
        }
    }

    pub fn is_fatal(self) -> bool {
        matches!(self, Self::Kickout | Self::AuthFailed | Self::Stop)
    }
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProbeSource {
    Radio = 1,
    Foreground = 2,
}

impl ProbeSource {
    pub(crate) fn from_u8(v: u8) -> Option<Self> {
        match v {
            1 => Some(Self::Radio),
            2 => Some(Self::Foreground),
            _ => None,
        }
    }

    pub(crate) fn store(into: &AtomicU8, src: Self) {
        into.store(src as u8, std::sync::atomic::Ordering::SeqCst);
    }
}
