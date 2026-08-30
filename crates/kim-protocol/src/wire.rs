pub const META_DEST_SERVER: &str = "dest.server";
pub const META_DEST_CHANNELS: &str = "dest.channels";
pub const META_APP: &str = "app";
pub const META_ACCOUNT: &str = "account";

pub const SN_WGATEWAY: &str = "wgateway";
pub const SN_TGATEWAY: &str = "tgateway";
pub const SN_CHAT: &str = "chat";
pub const SN_ROYAL: &str = "royal";
/// Login service name. Equal to [`SN_CHAT`]: login and chat share a process.
pub const SN_LOGIN: &str = "chat";

pub const CMD_LOGIN_SIGN_IN: &str = "login.signin";
pub const CMD_LOGIN_SIGN_OUT: &str = "login.signout";
pub const CMD_LOGIN_RENEW: &str = "login.renew";
pub const CMD_DEMO_ECHO: &str = "chat.demo.echo";
pub const CMD_CHAT_USER_TALK: &str = "chat.user.talk";
pub const CMD_CHAT_GROUP_TALK: &str = "chat.group.talk";
pub const CMD_GROUP_CREATE: &str = "chat.group.create";
pub const CMD_GROUP_JOIN: &str = "chat.group.join";
pub const CMD_GROUP_QUIT: &str = "chat.group.quit";
pub const CMD_GROUP_DETAIL: &str = "chat.group.detail";
pub const CMD_GROUP_MEMBERS: &str = "chat.group.members";
pub const CMD_CHAT_TALK_ACK: &str = "chat.talk.ack";
pub const CMD_OFFLINE_INDEX: &str = "chat.offline.index";
pub const CMD_OFFLINE_CONTENT: &str = "chat.offline.content";
pub const CMD_USER_PROFILE: &str = "chat.user.profile";
pub const CMD_USER_UPDATE: &str = "chat.user.update";
pub const CMD_USER_SEARCH: &str = "chat.user.search";
pub const CMD_FRIEND_REQUEST: &str = "chat.friend.request";
pub const CMD_FRIEND_ACCEPT: &str = "chat.friend.accept";
pub const CMD_FRIEND_REJECT: &str = "chat.friend.reject";
pub const CMD_FRIEND_REMOVE: &str = "chat.friend.remove";
pub const CMD_FRIEND_LIST: &str = "chat.friend.list";
pub const CMD_FRIEND_INCOMING: &str = "chat.friend.incoming";
pub const CMD_BLOCK_ADD: &str = "chat.block.add";
pub const CMD_BLOCK_REMOVE: &str = "chat.block.remove";
pub const CMD_BLOCK_LIST: &str = "chat.block.list";
pub const CMD_INBOX_LIST: &str = "chat.inbox.list";
pub const CMD_INBOX_READ: &str = "chat.inbox.read";
pub const CMD_HISTORY: &str = "chat.history";

pub const INBOX_KIND_USER: i32 = 0;
pub const INBOX_KIND_GROUP: i32 = 1;

pub const MESSAGE_TYPE_TEXT: i32 = 1;
pub const MESSAGE_TYPE_IMAGE: i32 = 2;
pub const MESSAGE_TYPE_VOICE: i32 = 3;
pub const MESSAGE_TYPE_VIDEO: i32 = 4;

/// First path segment of `command`.
///
/// `service_name("login.signin")` is `"login"`, which is not a Naming service.
/// Accept must `forward(SN_LOGIN)` and must not use this for the login uplink.
pub fn service_name(command: &str) -> &str {
    command.split_once('.').map(|(s, _)| s).unwrap_or("default")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_on_first_dot() {
        assert_eq!(service_name("chat.user.talk"), "chat");
        assert_eq!(service_name(CMD_CHAT_GROUP_TALK), "chat");
        assert_eq!(service_name(CMD_GROUP_CREATE), "chat");
        assert_eq!(service_name(CMD_GROUP_JOIN), "chat");
        assert_eq!(service_name(CMD_GROUP_QUIT), "chat");
        assert_eq!(service_name(CMD_GROUP_DETAIL), "chat");
        assert_eq!(service_name(CMD_GROUP_MEMBERS), "chat");
        assert_eq!(service_name(CMD_CHAT_TALK_ACK), "chat");
        assert_eq!(service_name(CMD_OFFLINE_INDEX), "chat");
        assert_eq!(service_name(CMD_OFFLINE_CONTENT), "chat");
        assert_eq!(service_name(CMD_USER_PROFILE), "chat");
        assert_eq!(service_name(CMD_FRIEND_REQUEST), "chat");
        assert_eq!(service_name(CMD_INBOX_LIST), "chat");
        assert_eq!(service_name(CMD_HISTORY), "chat");
        assert_eq!(service_name("chat.offline.index"), "chat");
        // Accept must not use service_name for login: this is `"login"`, not SN_LOGIN.
        assert_eq!(service_name("login.signin"), "login");
        assert_eq!(service_name(CMD_LOGIN_SIGN_IN), "login");
        assert_eq!(service_name("nopath"), "default");
        assert_eq!(SN_LOGIN, SN_CHAT);
        assert_eq!(SN_LOGIN, "chat");
        assert_eq!(SN_TGATEWAY, "tgateway");
        assert_eq!(META_APP, "app");
        assert_eq!(META_ACCOUNT, "account");
    }
}
