pub const META_DEST_SERVER: &str = "dest.server";
pub const META_DEST_CHANNELS: &str = "dest.channels";

pub const SN_WGATEWAY: &str = "wgateway";
pub const SN_CHAT: &str = "chat";
/// Login service name. Equal to [`SN_CHAT`]: login and chat share a process.
pub const SN_LOGIN: &str = "chat";

pub const CMD_LOGIN_SIGN_IN: &str = "login.signin";
pub const CMD_LOGIN_SIGN_OUT: &str = "login.signout";
pub const CMD_DEMO_ECHO: &str = "chat.demo.echo";
pub const CMD_CHAT_USER_TALK: &str = "chat.user.talk";
pub const CMD_CHAT_GROUP_TALK: &str = "chat.group.talk";
pub const CMD_GROUP_CREATE: &str = "chat.group.create";

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
        // Accept must not use service_name for login: this is `"login"`, not SN_LOGIN.
        assert_eq!(service_name("login.signin"), "login");
        assert_eq!(service_name(CMD_LOGIN_SIGN_IN), "login");
        assert_eq!(service_name("nopath"), "default");
        assert_eq!(SN_LOGIN, SN_CHAT);
        assert_eq!(SN_LOGIN, "chat");
    }
}
