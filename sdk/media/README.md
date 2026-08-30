# KIM media Worker

Uploads images into the `kim-media` R2 bucket. Public reads are the R2 custom
domain `https://media.kim.ainexc.com/...` (not this Worker), so downloads do not
count against the Workers Free 100k/day cap.

```bash
cd sdk/media
npm ci
npm test
npx wrangler deploy
```

`POST https://upload.kim.ainexc.com/v1/objects`

- `Authorization: Bearer <Royal JWT>`
- `Content-Type: image/jpeg|png|webp|gif`
- raw body, max 5 MiB
- 201 `{ key, url, contentType, bytes }`

Auth: Worker calls `GET {ROYAL_ORIGIN}/api/v1/auth/me` (revocation included).
If `ROYAL_ORIGIN` is empty, it verifies HS256 with secret `JWT_SECRET`.
