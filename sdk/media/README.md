# KIM media Worker

Uploads images into the `kim-media` R2 bucket. Public reads are the R2 custom
domain `https://media.kim.ainexc.com/...` (not this Worker), so downloads do not
count against the Workers Free 100k/day cap.

Object keys are content-addressed: `{sha256}.{ext}` (lowercase hex). Identical
bytes reuse the same key (`deduped: true`, no second `put`). Extension prefers
magic-byte sniff, then the request `Content-Type`. First write stores
`customMetadata.acc`; a dedupe hit leaves metadata unchanged. Legacy
`{account}/{yyyy}/{mm}/{uuid}.ext` objects remain readable.

```bash
cd sdk/media
npm ci
npm test
npx wrangler deploy
```

`POST https://upload.kim.ainexc.com/v1/objects`

- `Authorization: Bearer <Royal JWT>`
- `Content-Type: image/jpeg|png|webp|gif` (used when sniff cannot classify)
- raw body, max 5 MiB
- 201 `{ key, url, contentType, bytes, sha256, deduped }`
  - `sha256`: lowercase hex of the body
  - `deduped`: `true` when the object already existed (no `put`)

Auth: Worker calls `GET {ROYAL_ORIGIN}/api/v1/auth/me` (revocation included).
If `ROYAL_ORIGIN` is empty, it verifies HS256 with secret `JWT_SECRET`.
