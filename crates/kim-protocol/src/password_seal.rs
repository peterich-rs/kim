//! X25519 sealed-box password envelope (libsodium `crypto_box_seal` compatible).
//!
//! Clients seal the UTF-8 password to the server's published public key.
//! Royal opens with the private key, then Argon2 hashes/verifies as before.

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use crypto_box::aead::OsRng;
use crypto_box::{PublicKey, SecretKey};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Algorithm id published by `GET /api/v1/auth/password-key`.
pub const PASSWORD_SEAL_ALG: &str = "x25519-seal";

/// Royal `400` body when `AuthReq.key_id` does not match the live key.
pub const PASSWORD_KEY_ID_MISMATCH: &str = "password key id mismatch";

const KEY_LEN: usize = 32;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PasswordSealError {
    #[error("invalid password seal key")]
    InvalidKey,
    #[error("password seal failed")]
    Seal,
    #[error("password unseal failed")]
    Unseal,
    #[error("key id mismatch")]
    KeyIdMismatch,
}

/// Server-side seal material.
#[derive(Clone)]
pub struct PasswordSealKey {
    secret: SecretKey,
    public: PublicKey,
    key_id: String,
}

impl PasswordSealKey {
    /// Build from a 32-byte X25519 secret. `key_id` defaults to a short fingerprint.
    pub fn from_secret_bytes(
        secret: [u8; KEY_LEN],
        key_id: Option<String>,
    ) -> Result<Self, PasswordSealError> {
        let secret = SecretKey::from(secret);
        let public = secret.public_key();
        let key_id = match key_id.filter(|s| !s.trim().is_empty()) {
            Some(id) => id.trim().to_string(),
            None => fingerprint_public(public.as_bytes()),
        };
        Ok(Self {
            secret,
            public,
            key_id,
        })
    }

    /// Parse `KIM_AUTH_PASSWORD_SEAL_PRIVATE` (standard base64 of 32 raw bytes).
    pub fn from_private_b64(
        private_b64: &str,
        key_id: Option<String>,
    ) -> Result<Self, PasswordSealError> {
        let raw = B64
            .decode(private_b64.trim())
            .map_err(|_| PasswordSealError::InvalidKey)?;
        let bytes: [u8; KEY_LEN] = raw
            .as_slice()
            .try_into()
            .map_err(|_| PasswordSealError::InvalidKey)?;
        Self::from_secret_bytes(bytes, key_id)
    }

    /// Generate a fresh keypair (demo / tests).
    pub fn generate(key_id: Option<String>) -> Self {
        let secret = SecretKey::generate(&mut OsRng);
        let public = secret.public_key();
        let key_id = match key_id.filter(|s| !s.trim().is_empty()) {
            Some(id) => id.trim().to_string(),
            None => fingerprint_public(public.as_bytes()),
        };
        Self {
            secret,
            public,
            key_id,
        }
    }

    pub fn key_id(&self) -> &str {
        &self.key_id
    }

    pub fn alg(&self) -> &'static str {
        PASSWORD_SEAL_ALG
    }

    pub fn public_key_bytes(&self) -> [u8; KEY_LEN] {
        *self.public.as_bytes()
    }

    pub fn public_key_b64(&self) -> String {
        B64.encode(self.public.as_bytes())
    }

    pub fn private_key_b64(&self) -> String {
        B64.encode(self.secret.to_bytes())
    }

    pub fn seal(&self, plaintext: &[u8]) -> Result<Vec<u8>, PasswordSealError> {
        seal_with_public(&self.public, plaintext)
    }

    pub fn unseal(&self, ciphertext: &[u8]) -> Result<Vec<u8>, PasswordSealError> {
        self.secret
            .unseal(ciphertext)
            .map_err(|_| PasswordSealError::Unseal)
    }

    pub fn ensure_key_id(&self, key_id: &str) -> Result<(), PasswordSealError> {
        if key_id.trim() == self.key_id {
            Ok(())
        } else {
            Err(PasswordSealError::KeyIdMismatch)
        }
    }
}

/// Client-side public key material from the password-key endpoint.
#[derive(Clone, Debug)]
pub struct PasswordSealPublic {
    pub key_id: String,
    pub public: PublicKey,
}

impl PasswordSealPublic {
    pub fn from_b64(
        key_id: impl Into<String>,
        public_b64: &str,
    ) -> Result<Self, PasswordSealError> {
        let raw = B64
            .decode(public_b64.trim())
            .map_err(|_| PasswordSealError::InvalidKey)?;
        let bytes: [u8; KEY_LEN] = raw
            .as_slice()
            .try_into()
            .map_err(|_| PasswordSealError::InvalidKey)?;
        Ok(Self {
            key_id: key_id.into(),
            public: PublicKey::from(bytes),
        })
    }

    pub fn seal(&self, plaintext: &[u8]) -> Result<Vec<u8>, PasswordSealError> {
        seal_with_public(&self.public, plaintext)
    }
}

fn seal_with_public(public: &PublicKey, plaintext: &[u8]) -> Result<Vec<u8>, PasswordSealError> {
    public
        .seal(&mut OsRng, plaintext)
        .map_err(|_| PasswordSealError::Seal)
}

fn fingerprint_public(pk: &[u8]) -> String {
    let dig = Sha256::digest(pk);
    format!("x25519-{}", hex_8(&dig[..8]))
}

fn hex_8(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0xf) as usize] as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seal_open_roundtrip() {
        let key = PasswordSealKey::generate(Some("test-key".into()));
        assert_eq!(key.key_id(), "test-key");
        assert_eq!(key.alg(), PASSWORD_SEAL_ALG);
        let sealed = key.seal(b"secret123").expect("seal");
        assert_ne!(sealed.as_slice(), b"secret123");
        let opened = key.unseal(&sealed).expect("open");
        assert_eq!(opened, b"secret123");
    }

    #[test]
    fn public_client_seal_matches_server_open() {
        let key = PasswordSealKey::generate(None);
        let client = PasswordSealPublic::from_b64(key.key_id(), &key.public_key_b64()).unwrap();
        let sealed = client.seal(b"password-ok!!").unwrap();
        assert_eq!(key.unseal(&sealed).unwrap(), b"password-ok!!");
    }

    #[test]
    fn private_b64_roundtrip() {
        let key = PasswordSealKey::generate(Some("k1".into()));
        let restored =
            PasswordSealKey::from_private_b64(&key.private_key_b64(), Some("k1".into())).unwrap();
        let sealed = restored.seal(b"abc12345").unwrap();
        assert_eq!(key.unseal(&sealed).unwrap(), b"abc12345");
    }

    #[test]
    fn key_id_mismatch() {
        let key = PasswordSealKey::generate(Some("a".into()));
        assert!(key.ensure_key_id("b").is_err());
        assert!(key.ensure_key_id("a").is_ok());
        assert!(key.ensure_key_id("").is_err());
    }
}
