use std::fmt;

use crate::wire::{
    CMD_BLOCK_ADD, CMD_BLOCK_LIST, CMD_BLOCK_REMOVE, CMD_BOT_CREATE, CMD_BOT_DELETE,
    CMD_BOT_PENDING, CMD_BOT_REPLY, CMD_BOT_UPDATE, CMD_CHAT_GROUP_TALK, CMD_CHAT_TALK_ACK,
    CMD_CHAT_USER_TALK, CMD_DEMO_ECHO, CMD_FRIEND_ACCEPT, CMD_FRIEND_INCOMING, CMD_FRIEND_LIST,
    CMD_FRIEND_REJECT, CMD_FRIEND_REMOVE, CMD_FRIEND_REQUEST, CMD_GROUP_CREATE, CMD_GROUP_DETAIL,
    CMD_GROUP_JOIN, CMD_GROUP_MEMBERS, CMD_GROUP_QUIT, CMD_HISTORY, CMD_INBOX_LIST, CMD_INBOX_READ,
    CMD_LOGIN_RENEW, CMD_LOGIN_SIGN_IN, CMD_LOGIN_SIGN_OUT, CMD_OFFLINE_CONTENT, CMD_OFFLINE_INDEX,
    CMD_PRESENCE, CMD_RECEIPT_READ, CMD_ROOM_ENTER, CMD_ROOM_LEAVE, CMD_TYPING, CMD_USER_PROFILE,
    CMD_USER_SEARCH, CMD_USER_UPDATE, CMD_USER_UPDATED,
};

macro_rules! define_commands {
    ($($variant:ident => $cmd:ident),+ $(,)?) => {
        /// Wire `Header.command` parsed at the process boundary.
        ///
        /// Keep [`crate::CMD_LOGIN_SIGN_IN`] and the other `CMD_*` constants for
        /// codec/client packet construction. Internal dispatch uses this enum.
        #[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
        pub enum Command {
            $($variant,)+
        }

        impl Command {
            pub const ALL: &'static [Self] = &[$(Self::$variant,)+];

            #[must_use]
            pub fn parse(s: &str) -> Option<Self> {
                match s {
                    $($cmd => Some(Self::$variant),)+
                    _ => None,
                }
            }

            #[must_use]
            pub fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $cmd,)+
                }
            }
        }
    };
}

define_commands! {
    LoginSignIn => CMD_LOGIN_SIGN_IN,
    LoginSignOut => CMD_LOGIN_SIGN_OUT,
    LoginRenew => CMD_LOGIN_RENEW,
    DemoEcho => CMD_DEMO_ECHO,
    UserTalk => CMD_CHAT_USER_TALK,
    GroupTalk => CMD_CHAT_GROUP_TALK,
    GroupCreate => CMD_GROUP_CREATE,
    GroupJoin => CMD_GROUP_JOIN,
    GroupQuit => CMD_GROUP_QUIT,
    GroupDetail => CMD_GROUP_DETAIL,
    GroupMembers => CMD_GROUP_MEMBERS,
    TalkAck => CMD_CHAT_TALK_ACK,
    OfflineIndex => CMD_OFFLINE_INDEX,
    OfflineContent => CMD_OFFLINE_CONTENT,
    UserProfile => CMD_USER_PROFILE,
    UserUpdate => CMD_USER_UPDATE,
    UserUpdated => CMD_USER_UPDATED,
    UserSearch => CMD_USER_SEARCH,
    FriendRequest => CMD_FRIEND_REQUEST,
    FriendAccept => CMD_FRIEND_ACCEPT,
    FriendReject => CMD_FRIEND_REJECT,
    FriendRemove => CMD_FRIEND_REMOVE,
    FriendList => CMD_FRIEND_LIST,
    FriendIncoming => CMD_FRIEND_INCOMING,
    BlockAdd => CMD_BLOCK_ADD,
    BlockRemove => CMD_BLOCK_REMOVE,
    BlockList => CMD_BLOCK_LIST,
    InboxList => CMD_INBOX_LIST,
    InboxRead => CMD_INBOX_READ,
    History => CMD_HISTORY,
    RoomEnter => CMD_ROOM_ENTER,
    RoomLeave => CMD_ROOM_LEAVE,
    Presence => CMD_PRESENCE,
    Typing => CMD_TYPING,
    ReceiptRead => CMD_RECEIPT_READ,
    BotCreate => CMD_BOT_CREATE,
    BotDelete => CMD_BOT_DELETE,
    BotUpdate => CMD_BOT_UPDATE,
    BotReply => CMD_BOT_REPLY,
    BotPending => CMD_BOT_PENDING,
}

impl fmt::Display for Command {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONSTANTS: &[&str] = &[
        CMD_LOGIN_SIGN_IN,
        CMD_LOGIN_SIGN_OUT,
        CMD_LOGIN_RENEW,
        CMD_DEMO_ECHO,
        CMD_CHAT_USER_TALK,
        CMD_CHAT_GROUP_TALK,
        CMD_GROUP_CREATE,
        CMD_GROUP_JOIN,
        CMD_GROUP_QUIT,
        CMD_GROUP_DETAIL,
        CMD_GROUP_MEMBERS,
        CMD_CHAT_TALK_ACK,
        CMD_OFFLINE_INDEX,
        CMD_OFFLINE_CONTENT,
        CMD_USER_PROFILE,
        CMD_USER_UPDATE,
        CMD_USER_UPDATED,
        CMD_USER_SEARCH,
        CMD_FRIEND_REQUEST,
        CMD_FRIEND_ACCEPT,
        CMD_FRIEND_REJECT,
        CMD_FRIEND_REMOVE,
        CMD_FRIEND_LIST,
        CMD_FRIEND_INCOMING,
        CMD_BLOCK_ADD,
        CMD_BLOCK_REMOVE,
        CMD_BLOCK_LIST,
        CMD_INBOX_LIST,
        CMD_INBOX_READ,
        CMD_HISTORY,
        CMD_ROOM_ENTER,
        CMD_ROOM_LEAVE,
        CMD_PRESENCE,
        CMD_TYPING,
        CMD_RECEIPT_READ,
        CMD_BOT_CREATE,
        CMD_BOT_DELETE,
        CMD_BOT_UPDATE,
        CMD_BOT_REPLY,
        CMD_BOT_PENDING,
    ];

    #[test]
    fn covers_every_cmd_constant() {
        assert_eq!(Command::ALL.len(), CONSTANTS.len());
        assert_eq!(Command::ALL.len(), 40);
        for s in CONSTANTS {
            assert!(Command::parse(s).is_some(), "missing {s}");
        }
    }

    #[test]
    fn parse_roundtrip() {
        for cmd in Command::ALL {
            assert_eq!(Command::parse(cmd.as_str()), Some(*cmd));
            assert_eq!(cmd.to_string(), cmd.as_str());
        }
    }

    #[test]
    fn parse_unknown_is_none() {
        assert_eq!(Command::parse("no.such"), None);
        assert_eq!(Command::parse(""), None);
        assert_eq!(Command::parse("login.signin "), None);
    }

    #[test]
    fn as_str_matches_wire_constants() {
        assert_eq!(Command::LoginSignIn.as_str(), CMD_LOGIN_SIGN_IN);
        assert_eq!(Command::UserTalk.as_str(), CMD_CHAT_USER_TALK);
        assert_eq!(Command::GroupTalk.as_str(), CMD_CHAT_GROUP_TALK);
        assert_eq!(Command::TalkAck.as_str(), CMD_CHAT_TALK_ACK);
        assert_eq!(Command::Presence.as_str(), CMD_PRESENCE);
        assert_eq!(Command::LoginRenew.as_str(), CMD_LOGIN_RENEW);
    }
}
