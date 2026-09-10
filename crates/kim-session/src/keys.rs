/// Redis Pub/Sub channel. Payload is the account whose loc/session cache
/// every Chat instance must drop. Writers `PUBLISH`; cache listeners `SUBSCRIBE`.
pub const LOC_INV_CHANNEL: &str = "kim:loc:inv";

/// Redis / Memory session key: `login:sn:v2:{channel}`.
pub fn key_session(channel: &str) -> String {
    format!("login:sn:v2:{channel}")
}

/// Location key: `login:loc:v2:{account}`. `device` is ignored; callers still
/// pass it because [`kim_router::SessionStorage`] is unchanged.
pub fn key_location(account: &str, device: &str) -> String {
    let _ = device;
    format!("login:loc:v2:{account}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_key() {
        assert_eq!(key_session("wg-1_alice_1"), "login:sn:v2:wg-1_alice_1");
    }

    #[test]
    fn loc_inv_channel_is_stable() {
        assert_eq!(LOC_INV_CHANNEL, "kim:loc:inv");
    }

    #[test]
    fn location_key_empty_device() {
        assert_eq!(key_location("alice", ""), "login:loc:v2:alice");
    }

    #[test]
    fn location_key_with_device() {
        assert_eq!(key_location("alice", "phone"), "login:loc:v2:alice");
    }
}
