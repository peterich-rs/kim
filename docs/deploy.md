# 部署

对照小册第 32 章。本机开发仍是 `cargo run` + Memory，不需要 Docker。VPS 跑 `deploy/compose.yml`：gateway / chat / Redis / Postgres。镜像由 GitHub Actions 推到 GHCR，部署 job SSH 上去 `pull && up`。

## 本机开发（默认）

进程仍听 loopback。不要把 JWT 写进仓库。

```text
gateway :8001
chat    :8002
```

可选本机基础设施（旧习惯，不是默认 Demo）：

```bash
# 需要本机已装 Docker 时
docker compose -f deploy/compose.yml --env-file deploy/kim.env.example pull redis postgres
```

更省事：继续 Memory，不启 Redis / Postgres。

## Docker 栈（一台干净的 VPS）

`deploy/compose.yml` 自带 chat、chat-gray、gateway、royal、router、Consul、Redis、Postgres。`migrate` 先跑 `kim-server-migrate`；Chat / Royal 进程不再内嵌 DDL。Redis / Postgres **不**映射到宿主机端口。网关映射 `127.0.0.1:8001`，lookup `127.0.0.1:8088`，token `127.0.0.1:8080`，Consul HTTPS API `127.0.0.1:8501`（明文 8500 已关）。

从旧明文 Consul 切到 HTTPS + gossip `encrypt` + ACL 时，**不能**复用 `kim_consul_data`：旧 raft 没有这把 encrypt key，agent 起不来，健康检查失败，后面 `consul-acl` / Royal / Chat 全停。升级旧 VPS 时一次性执行以下人工迁移；自动部署不会删除任何数据卷：

```bash
cd /opt/kim/deploy
docker compose --env-file kim.env stop consul
docker compose --env-file kim.env rm -f consul
docker volume rm kim_consul_data
docker compose --env-file kim.env --profile metrics up -d
```

自动部署会同步并校验 `consul/agent.hcl`、`consul/acl-init.sh` 为普通文件；旧版短挂载误建的同名空目录也会在上传前清理。Compose 的 Consul 文件挂载禁止自动建目录，缺文件会立即报错。Consul agent 以 uid 100 读取 server / CA 私钥和 `secrets.hcl`；业务容器通过专用补充组 gid 10001 只读取共享 client key。`remote-up.sh` 每次 **force-recreate `consul-acl`**（oneshot 已成功时，bind-mount 脚本变更不会自行重跑），再执行独立迁移任务，逐个更新 Royal；两实例健康且签名 scope 检查一致后，依次更新 chat-gray / chat / gateway / router 并等待 `healthy`，并探测 `127.0.0.1:8080/health` 与 `:8088/health`。任一项失败都非 0，并打印上述服务日志；`compose up -d` 成功不等于 Router 已注册。手工排障也可看 `docker logs kim-consul-1` / `docker logs kim-royal-1` / `docker logs kim-router-1`：`permission denied` 是 TLS/secrets 文件权限不匹配（bootstrap 会修复）；`encryption key` / `invalid config` 是旧数据卷；`consul register 403` 是 Router ACL 缺少 `service "router" { policy = "write" }`。

已有 VPS 的 `kim.env` **已有密钥不会**被 `bootstrap.sh` 改写。脚本会 **preflight**：补齐缺失的 Consul TLS leaf、`secrets.hcl`（含 gossip `encrypt`），并对空的必填键 **只追加**（`KIM_ENV`、`CONSUL_HTTP_ADDR`、`KIM_IMAGE`、从 URL 抄出的 `REDIS_PASSWORD` / `POSTGRES_PASSWORD`、新 HMAC / Consul token；`CONSUL_MANAGEMENT_TOKEN` 若 `secrets.hcl` 已有则复用）。`REDIS_URL` 没有密码时会写入新密码并改 URL（下次 `compose up` Redis 带 `--requirepass`）。Postgres 数据卷只在首次初始化读密码，因此 **不会**给已有库另造 `POSTGRES_PASSWORD`。不会只因 CA 文件存在就跳过。Gossip 共享密钥只进 Consul agent 的 `secrets.hcl`，不进业务容器。`consul-acl` 创建 token 失败必须非 0（compose 不得放行业务）。Gateway 在 `REDIS_URL` 已配置但 revoke store 打不开时 **拒绝启动**，不得跳过吊销检查。部署新栈前确认：`KIM_ENV=production`、`REDIS_PASSWORD` / 带密码的 `REDIS_URL`、每服务 `CONSUL_TOKEN_*`、Consul 私有 CA 与 client cert、非 demo 的 `KIM_JWT_SECRET` / `KIM_INTERNAL_HMAC_SECRET`。缺任一项，生产进程拒绝启动。Chat **不再**直连 `DATABASE_URL`：消息 / 好友 / AgentSpec 云副本都经 `ROYAL_URL` HMAC，Postgres 只由 Royal 写。

停机（G-07 / G-32）：进程听 SIGTERM 和 SIGINT。顺序是 **先从 Consul 摘自己**，再停 accept，有界 drain 在途连接任务（默认 15s），然后关连接并 abort 剩余任务（含 TcpClient 心跳）。Royal / Router 同样先 deregister 再 axum graceful HTTP，不再 `process::exit`。K8s `terminationGracePeriodSeconds` 应大于 15s + Consul RTT。没有「请换网关」Push。未在集群里杀进程验证。

滚动：**同一窗口**切换镜像 + `kim.env` + Consul ACL/mTLS + Redis 密码。分镜像滚动时 **Royal 先于 Chat**（先签名后验签）。禁止「先发认 token 的代码打旧 HTTP Consul」。紧急用新二进制打旧 Consul 只允许 `KIM_ENV=development`（生产禁止长期）。Cloudflare TLS 只覆盖公网用户 → Caddy/WGateway，不进 Consul。

租户冻结（`app=kim`）另加一条：**Chat / Gateway / Royal 切到 `login:loc:v2` / `login:sn:v2` 之后，再重启全部 Gateway**，断开仍持有旧 `login:sn:*` 的 TCP。新 Gateway 只拒新的非 kim 登录；不排空则旧 kim-gray 长连接仍可能打到新 Chat。灰度白名单按 account，不是 `kim-gray` JWT；目标 zone 无实例时不要指望回退正式池。

pending receipt（默认关）顺序不可颠倒。compose 默认保持 0。切片稿：[impl/b0-pending-receipt-rollout.md](impl/b0-pending-receipt-rollout.md)。

1. 先发 Web SDK（`resume=true`、按 `has_more` 循环、页 200、persist 后再 batch ACK）。
2. migrate `0007`（空表；可与 1 并行）。GC 随 Royal。
3. 部署兼容代码：`KIM_REQUIRE_JTI=0`；Royal/Chat `KIM_PENDING_RECEIPT=0`。Gateway 已写 `Session.jti`。
4. 改 VPS `kim.env`：`KIM_REQUIRE_JTI=1`，`docker compose up -d gateway`。此后保持开。无 jti JWT 必须重新登录。
5. `deploy/scan-empty-jti.sh` 必须 exit 0（`empty_jti=0 invalid=0 wrong_type=0`）。不要用 talk 路径抽样 gauge。Royal 日志每 60s 打 `kim_location_without_jti` 以及 invalid/wrong_type/scanned。
6. `KIM_PENDING_RECEIPT_ROYAL=1`，Chat 仍 0，**两个 Royal 一起翻**：
   `docker compose -f deploy/compose.yml --env-file deploy/kim.env up -d royal royal-2`。
   分别 `exec -T royal printenv KIM_PENDING_RECEIPT` 与 `exec -T royal-2 printenv KIM_PENDING_RECEIPT`，都必须是 `1`。
   只重启 `royal` 会留下 `royal-2` writer=0，Chat 轮询仍可能打到旧实例。
   两实例都是 1 之后，用已知账号/设备发 canary，确认 `pending_delivery` 有对应新行。
7. `KIM_PENDING_RECEIPT_CHAT=1`，`docker compose up -d chat chat-gray`。
8. 回滚：Chat 先 0，再 Royal 0（同样 `up -d royal royal-2`）。禁止 Chat=1 且 Royal=0。

未走完 4–7 **不要**从 [production-gaps.md](production-gaps.md) 删 G-03 / G-04 / G-10。语义见 [reliable-delivery.md](reliable-delivery.md)。

inbox 物化读（`KIM_INBOX_MATERIALIZED`，royal / royal-2 only）：

1. 确认目标栈已含 inbox advisory lock 覆盖（legacy + pending + mark_read，含 sender）。
2. `deploy/backfill-inbox.sh`（经 Compose postgres 容器执行，可重跑；不要在宿主机对 `postgres:5432` 跑 psql）。
3. `deploy/psql-compose.sh deploy/diff-inbox.sql` 必须空结果（元组 oracle，不是旧 GROUP BY）。
4. kim.env 置 `KIM_INBOX_MATERIALIZED=1`，`up -d royal royal-2`。Chat 进程不读此键。
5. 回滚：置 0，同样两实例一起重启。物化表留存、双写继续。

| 路径 | 用途 |
|---|---|
| `Dockerfile` | 隔离上下文的镜像配方；gateway / tgateway / chat / royal / router / kim-server-migrate，`--locked --all-features` |
| `tools/build/prepare-server-context.py` | 只复制服务端依赖闭包并生成独立 Cargo workspace，不需要手机、桌面或 Codex 的 manifest/源码 |
| `.github/workflows/image.yml` | 使用生成目录构建 linux/amd64；PR 不推送，main/tag 推送 GHCR，tag 成功后部署 |
| `deploy/compose.yml` | 生产栈 |
| `deploy/chat.toml` / `gateway.toml` / `royal.toml` / `router.toml` | 容器内配置（听 `0.0.0.0`） |
| `deploy/kim.env.example` | 环境变量模板；真正的 `kim.env` 只活在 VPS |
| `deploy/Caddyfile` | `--profile edge` 时栈自己占 80/443（docker DNS：`royal:8080`） |
| `deploy/host.Caddyfile` | 宿主机 Caddy 的 `kim.ainexc.com` 块（loopback 端口） |
| `deploy/bootstrap.sh` | 生成 `kim.env`（一次）、Consul 私有 CA/mTLS、gossip encrypt、每服务 ACL token；已有 `kim.env` 做 TLS/`secrets.hcl` preflight，并追加空的必填键（不打印密钥，不改已有值，永不写 `change-me`） |
| `deploy/remote-up.sh` / `check-bot-scope.py` | CI 调用：pull → migrate → Royal 逐个健康与 scope 门禁 → Chat / Gateway / Router 逐个更新 |
| `deploy/psql-compose.sh` | 在 postgres 容器内跑 SQL（回填 / diff） |
| `deploy/backfill-inbox.sh` / `diff-inbox.sql` / `scan-empty-jti.sh` | inbox 回填与 pending-receipt SCAN 门闩 |

```bash
# 有 Docker 的机器上本地试跑镜像（先 build）
# 需要 Python 3.11+，目标目录必须尚不存在。
python3 tools/build/prepare-server-context.py /tmp/kim-server-image
docker build -t ghcr.io/peterich-rs/kim:local /tmp/kim-server-image
cp deploy/kim.env.example deploy/kim.env   # 改密钥后再 up
# 在仓库根目录：
KIM_IMAGE=ghcr.io/peterich-rs/kim:local docker compose -f deploy/compose.yml --env-file deploy/kim.env up -d
```

根 Cargo workspace 仍包含客户端与桌面 Agent，不能直接作为 Docker context。Image 工作流和本地构建都先运行生成脚本，再以生成目录作为 context；生成目录同时包含独立的 Cargo.toml、服务端锁文件和根 Dockerfile 的副本。修改源码后需生成新目录。锁文件更新流程见根 README 的「开发」。PR 对同一工作流执行镜像构建，并检查六个二进制、配置文件和非 root 用户。

TLS：

- 产品页用 Worker Route 时：`kim.ainexc.com` **橙云** A/AAAA 指向 VPS，Caddy 仍是源站。浏览器 TLS 在 Cloudflare。zone SSL 是 **Full**（CF→源站 HTTPS，不校验源站证书）。Origin 证书 + Full Strict 更好，但 SSL 模式是整站的，不要为 kim 单独改到 Strict。灰云则 Worker 不会接到流量。橙云后 Caddy HTTP-01 续期看不到挑战；Let’s Encrypt 到期前换成 Origin 证书或 DNS-01。
- 不用 Worker、compose 自己占 80/443：`docker compose --env-file kim.env --profile edge up -d`，DNS 可灰云做 Caddy HTTP-01。
- 宿主机已经有反代：不要开 `edge`。把 `deploy/host.Caddyfile` 的 `kim.ainexc.com` 块合进 `/etc/caddy/Caddyfile`：`/api/v1/auth/*` → `127.0.0.1:8080`（Royal），`/api/lookup` → `:8088`，Upgrade → `:8001`（关读超时）。**不要**把整站 `reverse_proxy` 到网关，否则注册 POST 会 404。

公网 TGateway 进程内 rustls 已落地。`tls_cert` / `tls_key` / `max_connections` 写在 `deploy/tgateway.toml` **文件根**（不要放进 `[self]` 或 `[route.whitelist]`：后者是 `HashMap<String, String>`，整数会让 `load_config` 启动失败）。证书路径空则同一二进制走明文。默认 compose **不**起 tgateway。怎么暴露（灰云或独立 IP:port）和同城双活仍是以后。UFW 默认只放 22/80/443。

## Bot 控制与删除合同升级

先运行同一镜像的 `kim-server-migrate`，再逐个更新 `royal`、`royal-2`，确认两实例健康且 `/internal/v1/bot/store-scope` 返回相同的 PG scope 后，再逐个更新 `chat-gray`、`chat`。`0017` 已部署时继续增加 `0018`，不修改旧迁移。迁移 runner 从 `DATABASE_URL` 环境变量读取地址，避免把密码写进命令行。`remote-up.sh` 已执行这一顺序；`check-bot-scope.py` 会签名探测每个 Royal，任一版本不支持、不可达或数据集不一致都会在更新 Chat 前失败。

Chat 的生产 `ROYAL_URL` 会同时装配消息 HTTP adapter 和 BotControl。只有消息走直连 PG / Memory 时才需另配 `BOT_CONTROL_URL`；它指定 bot 身份、公开目录和社交关系的权威。跨数据集的删除由实际消息 store 执行，不能用 URL 相同代替 scope 校验。

验收包含：创建、配置写入、错误 owner 删除被拒绝、正确删除、重复删除成功、清理失败后身份已不存在仍能恢复，以及另一个普通会话保持完整。历史已删身份但没有回执的数据作为独立运维修复处理，不根据消息对端自动补 owner 授权。状态码和内部合同见 [group-royal.md](group-royal.md#bot-生命周期与删除恢复)。

## 产品 H5（Cloudflare Worker）

静态页走 Workers Static Assets，登录 protobuf 和 WebSocket 仍回源 VPS。`kim.ainexc.com` 必须是 Cloudflare 橙云 DNS（A/AAAA 指向 VPS）。**不要**把该主机名做成 Worker Custom Domain，否则后台收不到请求。

```bash
cd sdk/web
npm ci
npm run deploy:app
```

打开 https://kim.ainexc.com 。Worker 路由是 `kim.ainexc.com/*`（zone `ainexc.com`）。`workers_dev` 关闭：账号没有 workers.dev 子域，CI 也不能交互注册。VPS 上 Caddy 继续反代 `/api/v1/auth/*` 和 Upgrade。

## CI

| Workflow | 何时 | 做什么 |
|---|---|---|
| `ci.yml` | `main` push / PR，或手动全量 | 按完整 Git diff 选择 Rust、PostgreSQL / Redis、Web、Media、移动端检查；始终校验 workflow / 仓库门禁，`CI passed` 汇总结果 |
| `server.yml` | CI 按服务端改动调用，或手动 | 不检出子模块，解析并编译隔离的服务端依赖图；缓存隔离 workspace 的 `target` |
| `image.yml` | 服务端 / 镜像输入的 PR、`main` push，或 tag `v*` / 手动 | 编 linux/amd64；PR 只 smoke test，main/tag 推 `ghcr.io/<owner>/kim`。`v*` 成功后再部署 |
| `deploy.yml` | Image 在 `v*` 成功后，或手动 | SSH 到 VPS，同步并校验部署文件，执行 `remote-up.sh`；不会自动删除数据卷 |
| `web.yml` | `main` 上 `sdk/web/**` / 根 npm 配置，或手动 | `npm run build:app` 后 `wrangler deploy` 到 `kim.ainexc.com`。Job 的 `if` 不能读 `secrets`；缺 token 时 deploy step 失败 |
| `media.yml` | `main` 上 `sdk/media/**` / 根 npm 配置，或手动 | `npm test` 后 `wrangler deploy` 到 `upload.kim.ainexc.com` |

npm 检查、发布和 Dependabot 统一使用根 `package-lock.json` workspace，固定 npm 11.19.0。Docker 层缓存使用 `kim-server-amd64` scope；`buildkit-cache-dance` 另外持久化 Dockerfile 的 Cargo registry / target mounts，因为 `type=gha` 层缓存不会导出这些 mount 的内容。PR / main 新提交取消同分支的旧镜像构建，版本 tag 构建不取消。

GitHub Secrets（只放在 GitHub，不进 git）：

- `KIM_VPS_HOST` — SSH 目标，例如 `root@203.0.113.10`
- `KIM_VPS_SSH_KEY` — **专用** ed25519 私钥；公钥在 VPS `authorized_keys`
- `CLOUDFLARE_API_TOKEN` — Worker 发布（Zone.Workers Routes Edit + Account.Workers Scripts Edit）
- `CLOUDFLARE_ACCOUNT_ID` — 同上账号

VPS **不**放 GitHub 写权限 PAT。`kim.env` 只在 `/opt/kim/deploy/`，CI 不上传这份文件；没有则 `bootstrap.sh` 生成一次。

部署用的 SSH 公钥与日常登录钥匙分开。
