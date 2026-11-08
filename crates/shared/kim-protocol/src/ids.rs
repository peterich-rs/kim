use std::borrow::Borrow;
use std::fmt;
use std::sync::Arc;

use crate::ProtocolError;

pub const ACCOUNT_MIN: usize = 3;
pub const ACCOUNT_MAX: usize = 32;

/// Opaque account identity. Hash/Borrow use the string contents so
/// `HashMap<AccountId, _>.get("alice")` works.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct AccountId(Arc<str>);

/// Opaque channel identity. `parse` rejects empty; `from_trusted` allows it
/// for Location legacy payloads.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct ChannelId(Arc<str>);

/// Opaque talk destination (account or group). `parse` rejects empty.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct DestId(Arc<str>);

/// Opaque gateway identity. `parse` rejects empty; `from_trusted` allows it.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct GatewayId(Arc<str>);

impl AccountId {
    /// Wire/storage value already accepted by [`Self::parse`] (or generated
    /// internally). Takes `&str` so `ChannelId::from_trusted(account_id)` does
    /// not compile. Never use this for registration/JWT — call [`Self::parse`].
    #[must_use]
    pub fn from_trusted(s: &str) -> Self {
        Self(Arc::from(s))
    }

    /// Trim, length 3–32, ascii alphanumeric or `_`.
    pub fn parse(s: &str) -> Result<Self, ProtocolError> {
        let s = s.trim();
        if s.len() < ACCOUNT_MIN || s.len() > ACCOUNT_MAX {
            return Err(ProtocolError::InvalidAccount);
        }
        if !s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            return Err(ProtocolError::InvalidAccount);
        }
        Ok(Self(Arc::from(s)))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub fn as_arc(&self) -> &Arc<str> {
        &self.0
    }
}

macro_rules! impl_nonempty_id {
    ($name:ident, $err:expr) => {
        impl $name {
            /// Wire/storage value. Empty is allowed here (Location legacy).
            /// Takes `&str` so a different ID type cannot be passed directly.
            #[must_use]
            pub fn from_trusted(s: &str) -> Self {
                Self(Arc::from(s))
            }

            pub fn parse(s: &str) -> Result<Self, ProtocolError> {
                let s = s.trim();
                if s.is_empty() {
                    return Err($err);
                }
                Ok(Self(Arc::from(s)))
            }

            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }

            #[must_use]
            pub fn as_arc(&self) -> &Arc<str> {
                &self.0
            }
        }
    };
}

impl_nonempty_id!(ChannelId, ProtocolError::InvalidChannelId);
impl_nonempty_id!(GatewayId, ProtocolError::InvalidGatewayId);
impl_nonempty_id!(DestId, ProtocolError::InvalidDest);

macro_rules! impl_id_traits {
    ($name:ident) => {
        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                self.as_str()
            }
        }

        impl Borrow<str> for $name {
            fn borrow(&self) -> &str {
                self.as_str()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(self.as_str())
            }
        }

        impl From<$name> for String {
            fn from(id: $name) -> Self {
                id.as_str().to_owned()
            }
        }
    };
}

impl_id_traits!(AccountId);
impl_id_traits!(ChannelId);
impl_id_traits!(DestId);
impl_id_traits!(GatewayId);

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn account_parse_alice() {
        let id = AccountId::parse("alice").unwrap();
        assert_eq!(id.as_str(), "alice");
        assert_eq!(AccountId::parse("  alice  ").unwrap().as_str(), "alice");
    }

    #[test]
    fn account_parse_too_short() {
        assert!(matches!(
            AccountId::parse("ab"),
            Err(ProtocolError::InvalidAccount)
        ));
    }

    #[test]
    fn account_parse_too_long() {
        let s = "a".repeat(33);
        assert!(matches!(
            AccountId::parse(&s),
            Err(ProtocolError::InvalidAccount)
        ));
    }

    #[test]
    fn account_parse_hyphen_rejected() {
        assert!(matches!(
            AccountId::parse("alice-bob"),
            Err(ProtocolError::InvalidAccount)
        ));
    }

    #[test]
    fn account_parse_alnum_underscore() {
        let id = AccountId::parse("Alice_01").unwrap();
        assert_eq!(id.as_str(), "Alice_01");
    }

    #[test]
    fn channel_parse_empty_rejected() {
        assert!(matches!(
            ChannelId::parse(""),
            Err(ProtocolError::InvalidChannelId)
        ));
        assert!(matches!(
            ChannelId::parse("   "),
            Err(ProtocolError::InvalidChannelId)
        ));
    }

    #[test]
    fn channel_from_trusted_allows_empty() {
        let id = ChannelId::from_trusted("");
        assert_eq!(id.as_str(), "");
    }

    #[test]
    fn account_borrow_hashmap_lookup() {
        let mut m = HashMap::new();
        m.insert(AccountId::parse("alice").unwrap(), 1);
        assert_eq!(m.get("alice"), Some(&1));
    }
}
