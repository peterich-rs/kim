# Web SDK（已落地）

对照小册第 23–24 章。服务端指令仍以 [link-layer-login.md](link-layer-login.md)、[control-layer-chat.md](control-layer-chat.md)、[reliable-delivery.md](reliable-delivery.md)、[group-royal.md](group-royal.md) 为准。本文只记 **现在代码里的** TypeScript 客户端：`sdk/web`。

`pkt-client` 仍是 Rust CLI 演示，不是 SDK。浏览器 / Node 22+ 走 `@kim/web-sdk`。

**不要**把 token 塞进 Upgrade URL。**不要**把 SDK 状态机写进 `WsServer` / `TcpServer`。

---

## 包

路径：`sdk/web`。连 `gateway` 的 WebSocket（本机 `ws://127.0.0.1:8001/`）。

```ts
const cli = new KIMClient(gatewayURL, { token })
cli.register([KIMEvent.Closed, KIMEvent.Kickout], onEvent)
cli.onmessage(onOnline)
cli.onofflinemessage(onOffline)
await cli.login()
await cli.talkToUser("bob", new Content("hello"))
await cli.logout()
```

Token 由调用方提供（JWT HS256，claims `acc` / `app` / `exp` / 可选 `jti`）。SDK **不**签发 Token，只从 payload 读 `acc`（不验签；验签在网关）。产品页 `sdk/web/app`（React + React Router + Tailwind）：`POST /api/v1/auth/register` 与 `/api/v1/auth/login`（protobuf `AuthReq`/`AuthResp`）拿 JWT，`POST /api/v1/auth/logout` 吊销。小册 demo：Vite 本机仍用 `mint.ts` 本地密钥。`pkt-client` 默认本地 `generate`；`KIM_AUTH_URL` + `KIM_PASSWORD` 走 Royal `/api/v1/auth/login`。

可选 `ClientOptions.routerUrl`：`login()` 先 `GET {routerUrl}/api/lookup`，`Authorization: Bearer <token>`，再用返回的 `ws`。构造函数 `wsurl` 不改。Token 永远不进 Upgrade URL。

`LoginReq` 没有 tags 字段，构造函数里的 `tags` 不会上线。

---

## 和本仓库对齐的线格式（不要抄小册多出来的 payload 长度）

BasicPkt：`magic(4) | code u16 LE | len u16 LE | body`。空 ping 是

`c3 15 a7 65 01 00 00 00`

（code=1 小端。小册示例把两个字节写反了，以 `kim-protocol` 为准。）

LogicPkt：`magic(4) | header_len u32 BE | Header protobuf | body`。

body 长度是 **Header.bodyLength**。没有小册 `from()` 里 header 后面那 4 字节 payload 长度。多写那 4 字节会进 protobuf body，登录会失败。

`messageId` / `sendTime` 是 int64，SDK 用 `bigint`。

`LoginResp` 只有 `channelId`。`account` 来自 JWT `acc`，不是 LoginResp。

---

## 状态机

```text
INIT → CONNECTING → CONNECTED → CLOSING → CLOSED
         │              │
         │              └── 异常断开：若 reconnect=true 且不是 Kickout，则 INIT 再 login
         └── 登录成功后先拉离线，再切 CONNECTED
```

| 状态 | 行为 |
|---|---|
| CONNECTING | 已挂 `onmessage`，可 `request()`；**不** ACK、**不**把在线 Push 回调给 UI |
| CONNECTED | 心跳、ACK loop、`onmessage` |
| CLOSING | 主动 logout；`onclose` 不再重连 |
| Kickout | `channelId` 必须等于自己的；不等则忽略。匹配则 `Kickout` 事件 + logout，不重连 |

离线同步期间不能 ACK：服务端读索引不区分在线/离线。同步中途 ACK 一条在线消息，更早的离线会丢。同步窗口里到达的 Push 只进本地 Store，等索引回调；`messageId` 去重。

---

## 请求-响应

`Map<sequence, Pending>`。客户端自增 sequence（登录固定 1，之后从 2）。`Flag=Response` 用同一 sequence 解开 Promise。超时 `KIMStatus.RequestTimeout=1001`；未连接 `SendFailed=1002`。这两个数不上线。

`talkToUser` / `talkToGroup`：状态码 `[300, 400)` 重试（默认 3 次），**同一次发送复用 `MessageReq.clientId`**。`UserNotFound=108` 与 `ContentBlocked` / `NotGroupMember` 一样不重试。`>= 400` 关连接（可重连）。

心跳若收到 `login.renew` Push（`AuthResp`），SDK 更新内存 token；产品页 `ontoken` 写回 `localStorage`。

ACK 是 fire-and-forget 的 `chat.talk.ack`，不进 sendq。循环大约每 `ackForceAfterMs`（默认 3s）对 `lastMessage` 发一次；到达不足 `ackDelayMs`（默认 500ms）则再等。未 ACK 超过 10 条不等待。

---

## 离线

登录成功后循环 `chat.offline.index`，起点是本地 `Store.lastId()`。索引分组进 `OfflineMessages`（用户 / 群），`loadUser` / `loadGroup` 再按页拉 `chat.offline.content`（最多 200 id）。

默认 `MemoryStore`。浏览器可用 `KeyValueStore(localStorage)` / `browserStore()`。

---

## 群

`createGroup` / `joinGroup` / `quitGroup` / `groupDetail` / `groupMembers`。dest 规则与 [group-royal.md](group-royal.md) 相同。`GroupCreateNotify` 走 `ongroupcreate`，不是 `onmessage`。

资料 / 好友 / 会话：`profile` / `updateProfile` / `searchUsers` / `friendRequest` / `friendAccept` / `friendList` / `friendIncoming` / `blockAdd` / `inbox` / `history` / `markRead`。私聊非好友返回 `NotFriends=109`。好友申请 Push 走 `onfriendrequest`。产品页登录后拉 inbox，点开会话再拉 history。改密走 `POST /api/v1/auth/password`。

SDK **不发** `login.signout`；断开由网关 Disconnect 转发。

---

## 本机怎么跑

先 Chat 再网关，见根 README。然后：

```bash
cd sdk/web
npm ci
npm test
# 需要已编译的 chat / gateway：
cargo build -p chat -p gateway
npm run test:e2e
```

e2e 自己起临时端口，不占用 `:8001`。

## 浏览器 Demo

产品页默认连生产后台（不必起本机进程）：

```bash
cd sdk/web && npm run app
```

打开 http://127.0.0.1:5173/ 。未登录进登录/注册页；登录后进会话列表。Vite 把 `/api` 代理到 `https://kim.ainexc.com`，WebSocket 直连 `wss://kim.ainexc.com/`。同一账号两个标签会互踢。Token 只由 Royal 签发。换源站：`KIM_ORIGIN=https://example.com npm run app`。

本机全套（先 Royal `:8080`，再 Chat，再网关 `:8001`）：

```bash
CHAT_URL=http://127.0.0.1:9002 RUST_LOG=info cargo run -p royal
ROYAL_URL=http://127.0.0.1:8080 RUST_LOG=info cargo run -p chat
ROYAL_URL=http://127.0.0.1:8080 RUST_LOG=info cargo run -p gateway
cd sdk/web && npm run app:local
```

生产构建（`npm run build:app`）把 WebSocket 指到当前页的 `wss://` 主机，由 Worker 回源 VPS 网关。

小册 demo（本机 mint，不打 Royal）：

```bash
cd sdk/web && npm run demo
```

然后 `?acc=alice&dest=bob`。Demo 用 `DEMO_DEFAULT_SECRET` 在页面里签 JWT，只适合本机。

网关默认绑 `127.0.0.1`，手机访问 Vite 也连不上本机网关。要对手机玩，把 `gateway` 的 listen 改成 `0.0.0.0:8001`，页面网关填 `ws://电脑局域网IP:8001/`。

网关连 Chat 的内连会发心跳（30s）。如果 Chat 已经死掉，页面会停在「连接中」直到超时；这时按 **先 Chat 再网关** 重启两个进程。

---

## 非目标

把 Token 放进 URL、改 `kim-protocol` 的 LoginResp、localforage。TGateway 是 `services/tgateway`（TCP + 同一套 `GatewayHandler`），不是第二套 SDK。
