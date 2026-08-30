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

`deploy/compose.yml` 自带 chat、chat-gray、gateway、royal、router、Consul、Redis、Postgres。Redis / Postgres **不**映射到宿主机端口。网关映射 `127.0.0.1:8001`，lookup `127.0.0.1:8088`，token `127.0.0.1:8080`，Consul UI `127.0.0.1:8500`。

已有 VPS 的 `kim.env` **不会**被 `bootstrap.sh` 改写。部署新栈前手工确认容器环境（compose 已注入 `ROYAL_URL` / `CONSUL_HTTP_ADDR`）。Chat **不再**直连 `DATABASE_URL`。

| 路径 | 用途 |
|---|---|
| `Dockerfile` | gateway / chat / royal / router（`consul` + chat/royal 的 redis,postgres） |
| `deploy/compose.yml` | 生产栈 |
| `deploy/chat.toml` / `gateway.toml` / `royal.toml` / `router.toml` | 容器内配置（听 `0.0.0.0`） |
| `deploy/kim.env.example` | 环境变量模板；真正的 `kim.env` 只活在 VPS |
| `deploy/Caddyfile` | `--profile edge` 时栈自己占 80/443（docker DNS：`royal:8080`） |
| `deploy/host.Caddyfile` | 宿主机 Caddy 的 `kim.ainexc.com` 块（loopback 端口） |
| `deploy/bootstrap.sh` | 第一次在 VPS 上生成 `kim.env`（不打印密钥） |
| `deploy/remote-up.sh` | CI 调用：login GHCR → pull → up |

```bash
# 有 Docker 的机器上本地试跑镜像（先 build）
docker build -t ghcr.io/peterich-rs/kim:local .
cp deploy/kim.env.example deploy/kim.env   # 改密钥后再 up
# 在仓库根目录：
KIM_IMAGE=ghcr.io/peterich-rs/kim:local docker compose -f deploy/compose.yml --env-file deploy/kim.env up -d
```

TLS：

- 产品页用 Worker Route 时：`kim.ainexc.com` **橙云** A/AAAA 指向 VPS，Caddy 仍是源站。浏览器 TLS 在 Cloudflare。zone SSL 是 **Full**（CF→源站 HTTPS，不校验源站证书）。Origin 证书 + Full Strict 更好，但 SSL 模式是整站的，不要为 kim 单独改到 Strict。灰云则 Worker 不会接到流量。橙云后 Caddy HTTP-01 续期看不到挑战；Let’s Encrypt 到期前换成 Origin 证书或 DNS-01。
- 不用 Worker、compose 自己占 80/443：`docker compose --env-file kim.env --profile edge up -d`，DNS 可灰云做 Caddy HTTP-01。
- 宿主机已经有反代：不要开 `edge`。把 `deploy/host.Caddyfile` 的 `kim.ainexc.com` 块合进 `/etc/caddy/Caddyfile`：`/api/v1/auth/*` → `127.0.0.1:8080`（Royal），`/api/lookup*` → `:8088`，Upgrade → `:8001`（关读超时）。**不要**把整站 `reverse_proxy` 到网关，否则注册 POST 会 404。

公网 TGateway（裸 TCP+TLS）和同城双活：**以后**。UFW 默认只放 22/80/443。

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
| `ci.yml` | push / PR | fmt、clippy、test |
| `image.yml` | `main` / tag `v*` | 编 linux/amd64，推 `ghcr.io/<owner>/kim`。`v*` 成功后再部署 |
| `deploy.yml` | Image 在 `v*` 成功后，或手动 | SSH 到 VPS，rsync compose，`remote-up.sh` |
| `web.yml` | `main` 上 `sdk/web/**` 或手动 | `npm run build:app` 后 `wrangler deploy` 到 `kim.ainexc.com`。Job 的 `if` 不能读 `secrets`；缺 token 时 deploy step 失败 |
| `media.yml` | `main` 上 `sdk/media/**` 或手动 | `npm test` 后 `wrangler deploy` 到 `upload.kim.ainexc.com` |

GitHub Secrets（只放在 GitHub，不进 git）：

- `KIM_VPS_HOST` — SSH 目标，例如 `root@203.0.113.10`
- `KIM_VPS_SSH_KEY` — **专用** ed25519 私钥；公钥在 VPS `authorized_keys`
- `CLOUDFLARE_API_TOKEN` — Worker 发布（Zone.Workers Routes Edit + Account.Workers Scripts Edit）
- `CLOUDFLARE_ACCOUNT_ID` — 同上账号

VPS **不**放 GitHub 写权限 PAT。`kim.env` 只在 `/opt/kim/deploy/`，CI 不上传这份文件；没有则 `bootstrap.sh` 生成一次。

部署用的 SSH 公钥与日常登录钥匙分开。
