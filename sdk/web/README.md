# @kim/web-sdk

TypeScript 客户端，连本仓库的 `gateway`（WebSocket）。对照小册第 23–24 章；线格式以 `kim-protocol` 为准，见 [docs/web-sdk.md](../../docs/web-sdk.md)。

需要 Node 22+（自带 `WebSocket`）。浏览器同样用全局 `WebSocket`。

## 用法

Token 由调用方签发（JWT HS256，`acc` / `app` / `exp`）。不要把 token 放进 URL。

```ts
import { KIMClient, KIMEvent, Content } from "@kim/web-sdk";

const cli = new KIMClient("ws://127.0.0.1:8001/", { token });
cli.register([KIMEvent.Closed, KIMEvent.Kickout], console.info);
cli.onmessage((m) => console.info(m.sender, m.body));
cli.onofflinemessage((om) => console.info("offline users", om.listUsers()));

const { success, err } = await cli.login();
if (!success) throw err;

await cli.talkToUser("bob", new Content("hello"));
await cli.logout();
```

产品页（`sdk/web/app`）用 React + React Router + Tailwind 做登录 / 注册 / 聊天，走 Royal 拿 JWT。小册 demo 仍用 `DEMO_DEFAULT_SECRET` 本机签发，见 `examples/pkt-client`。

## 浏览器里点

产品聊天默认连生产后台（不必起 Royal / Chat / 网关）：

```bash
cd sdk/web && npm run app
```

打开 http://127.0.0.1:5173/ 。Vite 把 `/api` 代理到 `https://kim.ainexc.com`，WebSocket 直连 `wss://kim.ainexc.com/`。换源站：`KIM_ORIGIN=https://example.com npm run app`。

本机全套后台：

```bash
cargo run -p royal
cargo run -p chat
cargo run -p gateway
cd sdk/web && npm run app:local
```

公网（Cloudflare Worker + VPS 源站）：`ainexc.com` 区里 `kim.ainexc.com` 橙云指向 VPS 后：

```bash
cd sdk/web && npm run deploy:app
```

打开 https://kim.ainexc.com 。

小册 demo（本机 mint）：

```bash
cargo run -p chat
cargo run -p gateway
cd sdk/web && npm run demo
```

## 开发

```bash
npm ci
npm test
cargo build -p chat -p gateway   # 仅 e2e
npm run test:e2e
npm run gen-proto   # proto 变更后重生成 src/proto/pkt.json
```

`pkt.proto` 的唯一来源是 `crates/kim-protocol/proto/pkt.proto`。
