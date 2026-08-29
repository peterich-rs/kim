# 用户资料、好友与服务端会话（已落地）

产品层：账号变成用户，私聊只在好友之间，会话列表和历史在服务端。长连接指令仍走网关 `forward("chat")`。Royal HTTP 给 Chat 的 `Http*` 适配器用；改密走公开 `POST /api/v1/auth/password`。

**不要**把资料 / 好友写进 `TcpServer` / `WsServer`。

---

## 人

`users` 增加 `nickname` / `avatar` / `bio`。注册与 upsert 时昵称默认等于账号。昵称 1–32 字，简介 ≤160，头像 URL ≤512。

| command | dest | 行为 |
|---|---|---|
| `chat.user.profile` | 空=自己，否则对方账号 | 返回 `UserProfile` |
| `chat.user.update` | 空 | 改自己的昵称 / 头像 / 简介 |
| `chat.user.search` | 空 | body `UserSearchReq.query`：精确账号或昵称前缀，最多 20 条。排除自己和拉黑 |

搜索全站可搜（小产品、用来加好友）。拉黑双方互相不可搜到。

改密：`POST /api/v1/auth/password`，`Authorization: Bearer`，body `PasswordChangeReq`。

---

## 关系

`friend_requests` / `friendships`（`account_a < account_b`）/ `blocks`。

| command | dest | 行为 |
|---|---|---|
| `chat.friend.request` | 对方账号 | 申请。对向已有申请则自动成友。已是好友 Success。在线对方收到 `Flag=Push` 的 `FriendRequestNotify` |
| `chat.friend.accept` | 申请人 | 成友；申请人收到 Push |
| `chat.friend.reject` | 申请人 | 丢掉申请 |
| `chat.friend.remove` | 好友 | 删好友，不拉黑 |
| `chat.friend.list` / `chat.friend.incoming` | 空 | `UserListResp` |
| `chat.block.add` / `remove` / `list` | 对方 / 空 | 拉黑会拆好友并取消双方申请 |

`chat.user.talk`：dest 未注册仍是 `UserNotFound=108`。拉黑 `Blocked=110`。非好友 `NotFriends=109`。自己给自己仍可发。群聊不查好友。

---

## 会话

`message_index` 已按账号写下 inbox。新增 `conversation_reads`（每会话 `last_read_id`）。ACK 仍只管离线窗口，不要混。

| command | dest | 行为 |
|---|---|---|
| `chat.inbox.list` | 空 | body `InboxReq.limit`（默认 50，最大 100）。每项含对方/群、最后一条、未读 |
| `chat.history` | 会话 dest | `HistoryReq.{beforeId,limit,kind}`。`kind` 0 私聊 1 群。按 `messageId` 倒序 |
| `chat.inbox.read` | 会话 dest | `ConversationReadReq.{messageId,kind}`。游标只前移 |

换设备登录：先 `inbox.list`，点开会话再 `history`，然后 `inbox.read`。

---

## Status

已落地号不改。新增 `NotFriends=109`、`Blocked=110`，都在 1xx：SDK 不重试、不关连接。
