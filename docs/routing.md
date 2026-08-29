# 智能路由（已落地）

对照小册第 29 章。crate `kim-router` 仍是指令分发。HTTP 查找是 `services/router`，本机 `127.0.0.1:8088`，VPS 经 Caddy `GET /api/lookup*`。生产 `CONSUL_HTTP_ADDR` 非空时从 Consul 找 `wgateway` / `tgateway`；没有 TGateway 时 `tcp=""`。

```bash
GET /api/lookup
Authorization: Bearer <jwt>
# 也可 ?token= 或 /api/lookup/:token；不要把 token 放进 WS Upgrade URL
# ?ip= 覆盖客户端 IP（测试）
```

JSON：`{ "utc", "location", "ws", "tcp" }`。没有 booklet 的 `domains[]`。

Geo：loopback → `default_location`；`[[ip_map]]` 精确 IP。没有 ip2region 文件。

步骤：IP→country→region→权重 IDC→`StaticNaming::find("wgateway"|"tgateway", ["IDC:{idc}"])`。`find` 对 tags 做 AND。空 tags = 不过滤。

SDK：`ClientOptions.routerUrl`。`login()` 用查找结果；构造函数 `wsurl` 当 fallback，不改它。
