# Auth password envelope + HTTPS

Clients seal login / register / change-password with libsodium-compatible
`crypto_box_seal` (alg `x25519-seal`) before the wire. Royal unseals, then
Argon2 hashes and verifies as before.

Generate a key:

```
cargo run -q -p kim-protocol --example gen_password_seal_key
```

## Wire

1. `GET /api/v1/auth/password-key` returns `{ key_id, alg: "x25519-seal", public_key_b64 }`.
   - `404` when no key is configured (local / upgrade).
   - `Cache-Control: no-store`.
2. Clients send `AuthReq.password_sealed` / `PasswordChangeReq.*_sealed` plus `key_id`.
3. Plaintext `password` is accepted only when the server allows it.

Nonce for the sealed box is `BLAKE2b-192(ephemeral_pk ‖ recipient_pk)` — the
libsodium `crypto_box_seal` construction (Rust `crypto_box` 0.9, Web tweetnacl
+ `@noble/hashes` `dkLen: 24`).

## Env

| Variable | Production (strict) | Notes |
|---|---|---|
| `KIM_AUTH_PASSWORD_SEAL_PRIVATE` | required | Standard base64 of a 32-byte X25519 secret |
| `KIM_AUTH_PASSWORD_SEAL_KEY_ID` | optional | Defaults to `x25519-` + SHA-256 fingerprint of the public key |
| `KIM_AUTH_ALLOW_PLAINTEXT_PASSWORD` | forbidden | `1` / `true` / `yes` accepts plaintext while a key is configured. Use only for a non-strict rollout |

`KIM_ENV=production` or a non-empty `CONSUL_HTTP_ADDR` is strict. Strict Royal
refuses to start if the seal key is missing or plaintext is allowed.

## Threat model

- **In scope.** The password on the Royal HTTP hop after TLS is terminated
  (Caddy, dumps, access logs of the request body). The sealed box is ciphertext
  to anything that is not the Royal private key.
- **Depends on TLS.** Sealed boxes do **not** authenticate the published public
  key. If HTTPS is broken, an attacker can replace `GET /password-key` with their
  own key and recover the plaintext. Optional hardening: pin `key_id` (or the
  public key) in the client build.
- **Downgrade.** Clients reject non-loopback `http://`. A MITM cannot 404
  `password-key` over plaintext HTTP to force a plaintext login. Strict
  production will not start without a seal key.
- **Key rotation.** Clients cache a published key for 5 minutes. On
  `400 password key id mismatch` they drop the cache and fetch once more. They
  do not cache a 404, so enabling the envelope is picked up on the next attempt.
- **Not in scope.** Password bytes in the Dart / JS heap, IME, OS clipboard,
  Argon2 parameters, JWT.

## Tests

- `kim-protocol` `password_seal`
- `kim-client` auth (including key-id retry)
- Royal `register_login_with_sealed_password`
- Web `password_seal` (JS opens the `crypto_box` 0.9.1 fixture)
