# 图片上传（R2）

Talk 帧已经有 `MessageReq.type = 2`。文件不进 WGateway / Chat / VPS。

```text
客户端  --JWT-->  POST upload.kim.ainexc.com/v1/objects   (Worker kim-media)
                      │  GET kim.ainexc.com/api/v1/auth/me  （校验 + 吊销）
                      ▼
                   R2 桶 kim-media
                      │
客户端  <img>  GET media.kim.ainexc.com/{account}/{yyyy}/{mm}/{uuid}.ext
```

| | |
|--|--|
| 桶 | `kim-media`（WNAM, Standard）。`r2.dev` **关** |
| 读 | R2 自定义域 `https://media.kim.ainexc.com`（不占 Workers 10 万/天） |
| 写 | Worker `kim-media`，Custom Domain `upload.kim.ainexc.com`（自带边缘证书；Universal SSL 盖不住二级子域） |
| 上限 | 5 MiB；`image/jpeg` `png` `webp` `gif` |
| 鉴权 | `Authorization: Bearer` → Royal `GET /api/v1/auth/me` |

`type=2` 的 `body` 是上面的 `url`，`extra` 可放宽高。`KimClient::talk_image` / `uploadImage` + `talkToUser(new Content(url, MessageType.Image))`。

对象 key 不可猜。自定义域公开读：知道 URL 就能看（免费档没有 WAF HMAC）。不要开 `r2.dev`。

```bash
cd sdk/media && npm ci && npm test && npx wrangler deploy
```
