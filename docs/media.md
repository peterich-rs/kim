# 图片上传（R2）

Talk 帧已经有 `MessageReq.type = 2`。文件不进 WGateway / Chat / VPS。

```text
客户端  --JWT-->  POST upload.kim.ainexc.com/v1/objects   (Worker kim-media)
                      │  GET kim.ainexc.com/api/v1/auth/me  （校验 + 吊销）
                      │  SHA-256(body) → key = {sha256}.{ext}
                      ▼
                   R2 桶 kim-media
                      │  同内容已存在则跳过 put（去重）
客户端  <img>  GET media.kim.ainexc.com/{sha256}.{ext}
```

| | |
|--|--|
| 桶 | `kim-media`（WNAM, Standard）。`r2.dev` **关** |
| 读 | R2 自定义域 `https://media.kim.ainexc.com`（不占 Workers 10 万/天） |
| 写 | Worker `kim-media`，Custom Domain `upload.kim.ainexc.com`（自带边缘证书；Universal SSL 盖不住二级子域） |
| key | 内容寻址 `{sha256}.{ext}`（小写 hex）。ext 优先魔数 sniff，否则信 `Content-Type`。首传写 `customMetadata.acc`；命中去重不改 metadata |
| 上限 | 5 MiB；`image/jpeg` `png` `webp` `gif` |
| 鉴权 | `Authorization: Bearer` → Royal `GET /api/v1/auth/me` |

`type=2` 的 `body` 是上面的 `url`。`extra` 是紧凑 JSON `{"w":1200,"h":800}`（缺省可空）。Web 产品页渲染缩略图，点击全屏查看；Flutter 缩略图走 `cached_network_image`，预览走 `photo_view` + `dismissible_page`（pinch / 双击缩放、纵向滑关、Hero）。

`KimClient::talk_image` / `uploadImage` + `talkToUser(new Content(url, MessageType.Image, extra))`。业务侧继续存最终 URL；旧路径 `{account}/{yyyy}/{mm}/{uuid}.ext` 对象仍可读，无需迁移历史消息。

对象 key 不可猜（SHA-256）。自定义域公开读：知道 URL 就能看（免费档没有 WAF HMAC）。内容寻址下，持有文件即可算出 URL。不要开 `r2.dev`。删除 / refcount / 配额另议。

```bash
cd sdk/media && npm ci && npm test && npx wrangler deploy
```
