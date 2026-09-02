# 可靠投递与离线同步（已落地）

对照小册第 20–21 章。在线 talk 仍以 [control-layer-chat.md](control-layer-chat.md) 为准。本文只记 **现在代码里的** ACK、写扩散和 Pull 离线。

投递语义是 **at-least-once + SDK 按 messageId 去重**。没有 `delivered` 列。

`KIM_PENDING_RECEIPT` **按进程解读**：Royal 是写权威，Chat 是读/ACK。compose 默认 0。先 Royal=1 再 Chat=1；禁止 Chat=1 且 Royal=0。关上 G-03 还要 Gateway `KIM_REQUIRE_JTI=1` 持续生效，以及 `deploy/scan-empty-jti.sh` 报 `empty_jti=0`。未走完 rollout **不要**从 [production-gaps.md](production-gaps.md) 删 G-03 / G-04 / G-10。可执行步骤见 [deploy.md](deploy.md)、[impl/b0-pending-receipt-rollout.md](impl/b0-pending-receipt-rollout.md)。

---

## 指令

| command | Flag | 谁处理 |
|---|---|---|
| `chat.talk.ack` | Request | Chat `do_talk_ack`。`Header.dest` 空。账号来自 session |
| `chat.offline.index` | Request | Chat `do_offline_index` |
| `chat.offline.content` | Request | Chat `do_offline_content`。一次最多 200 个 id |

网关对这三条仍然 `forward("chat")`。

| Status | 何时 |
|---|---|
| Success | ack / index / content 走完（ack 的 `messageId=0` 也是 Success，不改读索引） |
| InvalidPacketBody | body 解不开，或 content id 超过 200 |
| SystemException | store 失败 |
| SessionNotFound | 未登录 |

---

## 写扩散

`insert_user`：1 条 content + 2 条 index（发送方 `direction=1`，接收方 `direction=0`）。

`insert_group`：1 条 content + 每个成员 1 条 index。发送方 `direction=1`，其余 `0`。成员列表由 Handler 在 insert **之前** 从 `GroupDirectory` 取出。未知群或发送方不在成员中：`NotGroupMember`，不 insert。

落库是真相，在线 Push 是尽力。`insert_*` 成功后立刻尝试 `MessageResp` Success，再在 `TALK_PUSH_BUDGET`（3s）内 `get_locations` + `dispatch`。通过当前 filter / 用户存在 / 好友 / 黑名单（群聊：当前成员关系）之后，identical `clientId` 才从 `message_content` + `message_index` 重建 Push 与收件人，不信本次请求的 body / dest。删好友、拉黑或退群后的完全相同重试在 insert 前返回 109 / 107，不会重放。dispatch 失败或超时只打 `kim_dispatch_fail_total`，不再回 99。Royal writer=1 时同一事务写 `pending_delivery` receipt（`target_id` = JWT `jti`）；ACK 确认的是 message id 集合，不是 Snowflake 高水位。

离线拉取只读 `direction=0`。

---

## 读索引

Chat `KIM_PENDING_RECEIPT=0`（兼容）：key `chat:ack:{account}`（Redis）或进程内 map。TTL 30 天。`messageId==0` 不写。`offline.index` 仍按 `send_time` 高水位（见下）。

Chat `=1`（目标态，且 Royal writer 必须已是 1）：

- `chat.talk.ack` 确认 `{ messageId if != 0 } ∪ messageIds`，去重，上限 200。空集合 Success。超出 → `InvalidPacketBody`。服务端 `UPDATE pending_delivery SET acked_at`，**不 DELETE**。
- `chat.offline.index` 读该 session `jti` 的未确认 receipt。`resume=true` 才翻页（页 200，`has_more`）。遗留回路：`resume=false && messageId==0` 最多一页且 `has_more=false`；`resume=false && messageId!=0` 空页。空 jti → Success 空集。
- 登录：`add` loc 之后 `backfill_delivery`；失败则 delete loc 再 SystemException。
- `target_id` 是 JWT `jti`（续期复用），不是设备。

Chat=0 时 Royal=1 只会堆积 receipt，离线仍走高水位。Royal=0 时新 ack/index/backfill HTTP 返回 503 `pending-not-enabled`。

兼容路径（Chat=0）的起点（对齐小册 `getSentTime`）：

1. 请求 `messageId==0`：用服务端读索引（可能仍是 0）。
2. `messageId>0`：用 **content** 的 `send_time`；找不到则 now − 1 天。
3. 与 now − 15 天取较晚者。
4. `send_time > start`，LIMIT 2000。
5. 请求 `messageId>0` 时，返回前再 ACK 该 id。

`offline.content` 按请求 id **顺序** 返回。可见性：`message_content.app` 匹配且 `message_index` 存在 `(app, account_a, message_id)`；越权或不存在的 id **跳过**，整包仍 Success（不靠错误码探测 id）。Chat handler 覆盖 session 的 app/account；Chat→Royal 的 `MessageContentReq` 带这两个字段。无 HMAC 直打 Royal 是 401。

默认 Memory。`DATABASE_URL` + `--features postgres` 走 Postgres（列名 `group_id` / `msg_type`）。`REDIS_URL` + `--features redis` 时读索引走 Redis，与会话共用 URL。

---

## pkt-client

| 变量 | 行为 |
|---|---|
| `KIM_HOLD=1` 收到 talk Push | 默认延迟 `KIM_ACK_DELAY_MS`（200）后 `chat.talk.ack` |
| `KIM_SKIP_ACK=1` | HOLD 不 ACK |
| `KIM_SYNC_OFFLINE=1` | 登录后先 index（`KIM_ACK_FROM` 或 0）再分批 content |
| 默认 | 仍是 ping + echo，不拉离线 |

同一 `messageId` 的 Push 与离线 content 只打一次日志。

e2e：`services/chat/tests/e2e_offline.rs`。
