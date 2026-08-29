# 多租户与灰度（已落地）

对照小册第 30 章。租户 = JWT `app`。分区 = Chat 的 `[self].zone`。

网关 `RouteSelector`（`examples/gateway`，不是 `kim-container::HashSelector`）读 header meta `app` / `account`。`GatewayHandler` 在 accept 写入 ChannelMeta，并在 ready / receive / disconnect 的每一次 `forward` 注入。登出 forward 之后才删 meta；abandon / ready 失败也会删。

配置：

```toml
[route]
route_by = "app"

[[route.zones]]
id = "zone_local"
weight = 100

[route.whitelist]
"kim-gray" = "zone_gray"
```

`slots` 不进 TOML。全 0 权重不会 `% 0`，退回 `hash_pick(channel_id)`。目标 zone 没有 Adult 时，退回全部 Adult（crc32，不是 rand）。只要还有 Adult 就不会返回空。

网关 `[[services]]`（无 Consul 时）的 chat 要带 `tags = ["zone:zone_local"]`，与 Chat `[self].zone` 一致。生产两套 Chat：`zone_local`（weight 100）与 `zone_gray`（weight 0 + `"kim-gray" = "zone_gray"`）。会话 key 不含 app：同一 account 跨 app 会互踢。
