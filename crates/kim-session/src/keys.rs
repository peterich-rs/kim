/// Redis / Memory session key: `login:sn:{channel}`.
pub fn key_session(channel: &str) -> String {
    format!("login:sn:{channel}")
}

/// Location key: `login:loc:{account}` when `device` is empty, else `login:loc:{account}:{device}`.
pub fn key_location(account: &str, device: &str) -> String {
    if device.is_empty() {
        format!("login:loc:{account}")
    } else {
        format!("login:loc:{account}:{device}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_key() {
        assert_eq!(key_session("wg-1_alice_1"), "login:sn:wg-1_alice_1");
    }

    #[test]
    fn location_key_empty_device() {
        assert_eq!(key_location("alice", ""), "login:loc:alice");
    }

    #[test]
    fn location_key_with_device() {
        assert_eq!(key_location("alice", "phone"), "login:loc:alice:phone");
    }
}
