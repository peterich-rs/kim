# 多租户与灰度（已落地）

对照小册第 30 章。**租户已冻结为 JWT `app=kim`。** 分区 = Chat 的 `[self].zone`。灰度是 **account → zone** 白名单，不是第二租户、也不是 `kim-gray` JWT。

网关 Accept 在 parse JWT 之后拒绝 `app != "kim"`（`Unauthorized=105`）。Royal 生产（`strict_runtime`）要求 `KIM_APP=kim`。Chat `receive` 在非 `login.signIn` 路径拒绝 `session.app != kim`。持有 `kim-gray` 或其它 app 的 token 必须重新登录。

网关 `RouteSelector`（`services/gateway`，不是 `kim-container::HashSelector`）读 header meta `account`（以及 `app`，仅用于 `route_by=app` 的旧配置）。`GatewayHandler` 在 accept 写入 ChannelMeta，并在 ready / receive / disconnect 的每一次 `forward` 注入。登出 forward 之后才删 meta；abandon / ready 失败也会删。

配置：

```toml
[route]
route_by = "account"

[[route.zones]]
id = "zone_local"
weight = 100

[[route.zones]]
id = "zone_gray"
weight = 0

[route.whitelist]
# account -> zone。不要再写 "kim-gray"。
# "alice" = "zone_gray"
```

`route_by` 必须是 `account`。若仍 `route_by=app` 且全是 `kim`，哈希退化成单 zone。`slots` 不进 TOML。全 0 权重不会 `% 0`，退回 `hash_pick(channel_id)`。

白名单命中且目标 zone 没有 Adult：**返回空路由**（`warn` gray zone empty），**禁止**静默回退正式 Chat。非白名单账号在 slot 算出的 zone 无实例时，仍可回退全部 Adult。

网关 `[[services]]`（无 Consul 时）的 chat 要带 `tags = ["zone:zone_local"]`，与 Chat `[self].zone` 一致。生产两套 Chat：`zone_local`（weight 100）与 `zone_gray`（weight 0 + account 白名单）。

会话 Redis key 是 v2 前缀，仍不含 app：`login:loc:v2:{account}`、`login:sn:v2:{channel}`。心跳 `touch_session` 走同一套 key。loc cache 默认关（`KIM_LOC_CACHE=1` / `true` 才 wrap）；生产 compose 显式 `0`。

滚动：Chat / Gateway / Royal **同一窗口**切到 v2 key（不双写），**然后再重启全部 Gateway**，断开仍持有旧 `login:sn:*` 的 TCP。新 Gateway 只挡住新的非 kim 登录；不排空则旧长连接的 session blob 仍可能被新 Chat `cache.get` 执行。回滚到旧镜像后新 key 不可见，全员需重登。
