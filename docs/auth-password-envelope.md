# Auth password envelope + HTTPS

See PR description for threat model, key env vars, and test commands.
Algorithm: x25519-seal (libsodium crypto_box_seal).
Env: KIM_AUTH_PASSWORD_SEAL_PRIVATE, KIM_AUTH_PASSWORD_SEAL_KEY_ID, KIM_AUTH_ALLOW_PLAINTEXT_PASSWORD.
Generate keys: cargo run -q -p kim-protocol --example gen_password_seal_key

## Threat model
- X25519 sealed box + HTTPS client enforcement; Argon2 unchanged.
## Wire
- GET /api/v1/auth/password-key then sealed fields on AuthReq.
## Tests
- kim-protocol password_seal; kim-client auth; royal sealed_password; web password_seal.
