# 部署

对照小册第 32 章。本机开发仍是 `cargo run` + Memory，不需要 Docker。VPS 跑 `deploy/compose.yml`：gateway / chat / Redis / Postgres。镜像由 GitHub Actions 推到 GHCR，部署 job SSH 上去 `pull && up`。

## 本机开发（默认）

进程仍听 loopback。不要把 JWT 写进仓库。

```text
fake-gateway :8001
fake-chat    :8002
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
| `deploy/Caddyfile` | `--profile edge` 时栈自己占 80/443 |
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

- 这套 compose 独占 80/443：`docker compose --env-file kim.env --profile edge up -d`。DNS A 记录先灰云（DNS only），方便 Caddy HTTP-01。
- 宿主机已经有反代：不要开 `edge`，把该站点指到 `127.0.0.1:8001`（WebSocket 关读超时）。

公网 TGateway（裸 TCP+TLS）和同城双活：**以后**。UFW 默认只放 22/80/443。

## CI

| Workflow | 何时 | 做什么 |
|---|---|---|
| `ci.yml` | push / PR | fmt、clippy、test |
| `image.yml` | `main` / tag `v*` | 编 linux/amd64，推 `ghcr.io/<owner>/kim` |
| `deploy.yml` | tag `v*` 或手动 | SSH 到 VPS，rsync compose，`remote-up.sh` |

GitHub Secrets（只放在 GitHub，不进 git）：

- `KIM_VPS_HOST` — SSH 目标，例如 `root@203.0.113.10`
- `KIM_VPS_SSH_KEY` — **专用** ed25519 私钥；公钥在 VPS `authorized_keys`

VPS **不**放 GitHub 写权限 PAT。`kim.env` 只在 `/opt/kim/deploy/`，CI 不上传这份文件；没有则 `bootstrap.sh` 生成一次。

部署用的 SSH 公钥与日常登录钥匙分开。
