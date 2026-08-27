pub const META_DEST_SERVER: &str = "dest.server";
pub const META_DEST_CHANNELS: &str = "dest.channels";

pub const SN_WGATEWAY: &str = "wgateway";
pub const SN_CHAT: &str = "chat";

pub const CMD_DEMO_ECHO: &str = "chat.demo.echo";

pub fn service_name(command: &str) -> &str {
    command.split_once('.').map(|(s, _)| s).unwrap_or("default")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_on_first_dot() {
        assert_eq!(service_name("chat.user.talk"), "chat");
        assert_eq!(service_name("login.signin"), "login");
        assert_eq!(service_name("nopath"), "default");
    }
}
