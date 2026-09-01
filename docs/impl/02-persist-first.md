# 落库即成功并从持久化重建 Push

对应 [production-gaps.md](../production-gaps.md) **G-09**（H0 语义里「insert 成功 → `MessageResp`；duplicate 从落库 content/index 重建再 dispatch」那一段）。**不**把 H0 的 `try_send` / `kim_mailbox_full_total`（G-30）拉进本切片。

Addressing review feedback（P1）：(1) 已落库仍等 `dispatch().await` 才 Success——`send_binary` 无超时，Bob 慢则 Alice 超时重试；必须 **先 resp 再有界投递**（见「核查：落库后被 Push 阻塞」）；(2) 保留 G-03/G-14 时 **不能删除 G-09**；(3) duplicate 必须在可变鉴权之前 preflight，不一致返回冲突而不是「条件式重放」。

本切片合入后 **不删 G-09**（对齐切片 1 对 G-02/G-08 的处理）。拆成：

| 本切片关的 | 仍算 G-09 未关、依赖别人 |
|---|---|
| 发送方在 insert Ok 之后于有界时间内收到 Success（不再因 dispatch/locations 失败回 99） | 漏 Push 的可靠补偿：Snowflake 高水位仍会吞洞（G-03）；Flutter 登录不 pull（G-14） |
| identical `(clientId, dest, type, body, extra, kind)` 从落库 Fanout 重放 | 网关下行仍 `send().await`（G-30）；本切片只给 **talk 的 get_locations+dispatch** 加预算，不改 `Channel` |
| 改 dest/body 的同一 clientId → `IdempotencyConflict`，不把旧消息打到新 dest | |

**明确不关：**

| 条目 | 本切片之后 |
|---|---|
| G-03 / G-04 / G-10 | ACK 仍是 Snowflake 高水位；`chat.talk.ack` / `offline.index` 不改 |
| G-14 | Flutter / `kim-client` 登录后仍不 pull；**不**在 connect 时服务端 sync inbox |
| G-30 | mailbox 仍 `send().await`；无 `kim_mailbox_full_total`。talk 路径用 `timeout` 包住 push，满信箱不再挡住 **Success** |
| G-01 | Royal HTTP 仍裸；Compose Redis / Consul 不变 |
| G-05 | 寻址仍只有 account |
| H5 | 无 outbox 表、无分批 index；发送时成员快照 = 当时写下的 `message_index` 行 |

「已落库」**不等于**接收端可恢复收到。文档不得把 offline pull 写成 G-09 的关闭条件。

日期：2026-09-01。

---

## Feasibility Assessment

可行，且不需要 schema 变更。

1. 写扩散已经把正文放进 `message_content`、收件人放进 `message_index`（`0001_messages.sql`）。duplicate 的权威就是这两张表，不是本次 `MessageReq`。
2. `InsertResult` 和 Royal `InsertMessageResp` 今天只有 `messageId` / `sendTime` / `duplicate`。proto3 嵌套 `InsertFanout fanout = 4`（prost `Option`）向后兼容：旧 Chat 读新 Royal 会忽略子消息。**新 Chat + 旧 Royal** 是 `fanout=None` → `recipients=[]` → **不 dispatch、Success**（看起来像全员离线），**不会**打空 body 的 Push。必须按下面的发布顺序，并用空 `recipients` 打 `kim_dispatch_fail_total`，不能静默。
3. `Context::dispatch`（`crates/kim-router/src/context.rs` 约 86–131 行）已经「每个网关都试、记下第一个 Err」。部分成功仍返回 Err，**不能**当「从未 Push」的权威。Handler 不再把该 Err 映射成客户端 99，语义本身不动。
4. 单测 `dispatch_fail_is_system_exception_without_success_resp`（`talk.rs` 约 813 行）把错误契约锁死了，必须改写，不是另开测试文件。
5. `offline_content` 是 G-02 可见性 API：只返回 `msg_type/body/extra`，且按调用方 `account_a` 过滤。发送方的 SEND 行不够重建群的全部收件人。本切片 **不** 复用它做 fanout。

阻塞项：无。代码同仓；**运行时**必须先新 Royal（或共享 `KIM_IMAGE`），再新 Chat。见 Key design decisions §1 发布框。

---

## Current Surface Inventory

| 表面 | 现状 | 本切片 |
|---|---|---|
| `do_user_talk`（`services/chat/src/talk.rs` 37–195） | dest 空 / decode / filter / exists / block / friend → **先** `get_locations` → `insert_user` → 用 **本次** `req` 组 Push → `if !duplicate { dispatch }`；dispatch Err → 99 且 **不** Success | 非空 `clientId` 先 `lookup_idempotency`；命中且字段一致 → 跳过鉴权直接 `persist_then_push`；不一致 → 111。未命中才 filter/friend 再 insert。**先 Success，再有界** locations+dispatch |
| `do_group_talk`（同文件 197–322） | filter → `members()` → `insert_group` → `get_locations(members)` → duplicate 跳过、dispatch Err=99 | 同样 preflight。`members()` 只服务 **新 insert**。重放收件人 = index 快照 |
| `InsertResult`（`store/mod.rs` 53–57） | `{ message_id, send_time, duplicate }` | 嵌 `Fanout` |
| `MessageStore::lookup_idempotency` | 无 | `(app, sender, client_id) → Option<InsertResult>`；未命中 `Ok(None)` |
| Memory duplicate（`store/mod.rs` 304–313、369–376） | 幂等表命中只回 id/time | 命中后从 `contents` + `indexes` 填 `Fanout` |
| Postgres `insert_fanout`（`store/postgres.rs` 65–198） | `SELECT message_id, send_time FROM message_idempotency`；ON CONFLICT 回滚后再读同样两列 | 同上，再 JOIN content/index |
| `HttpMessageStore`（`royal.rs` 149–211） | 映射三字段 | 映射新 proto 字段 |
| Royal `insert_user` / `insert_group`（`services/royal/src/lib.rs` 208–283） | 编码三字段 | 编码 fanout |
| `InsertMessageResp`（`pkt.proto` 186–190） | 三个字段 | proto3 嵌套 `InsertFanout fanout = 4`（prost `Option`） |
| `Context::dispatch` | 跳过本 session `channel_id`；按 `gate_id` 合包；先记第一个 Err | **不改** |
| `KimMetrics`（`crates/kim-metrics/src/lib.rs`） | `kim_talk_total{kind}`；`on_talk` 在 `ChatHandler::receive`（`lib.rs` 518–524） | 新增 `kim_dispatch_fail_total`；talk handler 在 dispatch/locations 失败时 `inc` |
| `FailStore`（`talk.rs` 486–565） | 实现全部 `MessageStore` 方法 | 加 `lookup_idempotency` → `Ok(None)` |
| `same_client_id_does_not_insert_or_push_twice`（约 648） | 两次 Success、同一 id；**断言 Push 恰好 1** | identical 重试 → 两次 Push（at-least-once） |
| `send_binary`（`crates/kim-core/src/channel.rs` 225–238） | 无超时 `tx.send().await` | **不改** Channel。talk 用 `timeout(TALK_PUSH_BUDGET, locations+dispatch)` |
| `Status`（`pkt.proto` 10–27） | 到 `Blocked=110` | 新增 `IdempotencyConflict = 111`（1xx，Web 不重试） |
| Web `isRetryable`（`sdk/web/src/status.ts` 38–39） | 仅 300–399 | **不改**。persist 成功不再回 99，Web 不会把「已落库」当失败重试 |
| `docs/control-layer-chat.md` 48、58、75、91–96 | SystemException 含 dispatch 失败；duplicate「不再 Push」 | 代码落地时改文档 |

单聊今天 **insert 前进寻址**；群聊 **insert 后再寻址**。locations 非 `NotFound` 时：单聊 99 且 **未 insert**（`get_location_other_is_system_exception_without_insert_or_push`，约 844）；群聊 99 且 **已 insert**（`group_get_locations_other_is_system_exception_after_insert`，约 1237）。统一到 insert 之后后，两条都变成「已 insert + Success + 无 Push」。

幂等键仍是 `(app, sender, client_id)`（`0005_message_idempotency.sql`）。**不含 dest**。同一 `clientId` 改 dest 仍命中第一次的行。

---

## Design

落库是真相，在线 Push 是尽力。`insert_*` 返回 `Ok`（或 idempotency 命中）之后，发送方在 **`TALK_PUSH_BUDGET` 内**看到 `Status::Success` + `MessageResp`——Success **先于** `get_locations` / `dispatch` 发出。网关 push 失败、超时、locations 故障，只 `warn` + `kim_dispatch_fail_total`。客户端按 `messageId` 去重。

`talk.rs` **禁止**再用 `req.r#type` / `req.body` / `req.extra` / `header.dest` / 当前 `members()` 组 `MessagePush` 或 dispatch 目标。唯一输入是已持久化的 `Fanout`。

这 **不**等于接收端一定能拉到：G-03 高水位仍会吞洞，G-14 Flutter 仍不 pull。G-09 条目保留。

### Key design decisions

1. **用 enrichment 拿到权威 Push + 收件人（方案 A），不用二次 `load_fanout` HTTP（B），不用「首次信请求、duplicate 信库」（C）。proto 用嵌套 `InsertFanout`，不用扁平标量。**
   - **选定：** 扩展 `InsertResult` 与 `InsertMessageResp`。`insert_user` / `insert_group` 无论 first 还是 duplicate 都返回同一 `Fanout`。第一次用**刚写入的字段**填这个结构（不是 talk 层读 `req.body`）；duplicate 在 Memory/Postgres 里 load content+index。Chat 生产路径仍是一次 Royal HTTP。
   - **拒绝 B：** `MessageStore::load_fanout` 在每次 insert 后再打一枪。G-16 已经嫌 Royal HTTP 多；duplicate 与 first 还要两套入口。
   - **拒绝 C：** first 走 `req.body`、duplicate 走 store，正好是 G-09 要拆掉的窗口。即便「刚写入的值等于 req」，也必须在 store 层收成 `Fanout` 再交给 talk。
   - **选定嵌套 `InsertFanout fanout = 4`，拒绝扁平字段 4–10。** prost 对 proto3 message 生成 `Option<InsertFanout>`，旧 Royal 是 `None`；幽灵 content 是 `Some` 且 `recipients=[]`。扁平 `type`/`body`/`recipients` 没有 presence，Chat 分不出「旧 Royal」和「空正文 + 空收件人」。不在 Chat 侧对空 fanout 回退到本次请求。
   - **混版本失败模式（订正）：** 新 Chat + 旧 Royal → `fanout=None` → `HttpMessageStore` 填空 `Fanout`（`recipients=[]`）→ `persist_then_push` **跳过 `get_locations` 与 `dispatch`** → 发送方 **Success**。这是「静默无 Push，看起来像全员离线」，**不是**空 body 的 Push。必须 `warn` + `on_dispatch_fail`，否则 `chat-gray` 单独滚到新镜像、Royal 仍旧时线上 Push 会死且无指标。旧 Royal **已经落库**，不得把混版本映射回客户端 99（那会把 G-09 再引进来）。
   - **发布 / 回滚（合同，不是「同仓同发即可」）：**

     | 动作 | 顺序 |
     |---|---|
     | 升级 | 先 Royal（或一次发共享 `KIM_IMAGE`）。compose 里 `chat` / `chat-gray` 都 `depends_on: royal` 且 `image: ${KIM_IMAGE}`（`deploy/compose.yml` 约 88–165 行），同镜像时顺序自动满足。分镜像 / 金丝雀时：**先新 Royal，再新 Chat**（含 `chat-gray`）。 |
     | 回滚 | **先 Chat（含 chat-gray），再 Royal。** 新 Chat 对着旧 Royal 是本切片的洞。 |
     | 禁止 | 新 Chat 跑在旧 Royal 上；只滚 `chat-gray` 而 Royal 停在旧 fanout。 |

2. **落库与「告诉 Alice 成功」解绑：insert Ok 之后立刻 `resp(Success)`，再有界地 `get_locations` + `dispatch`。** 详见下一节「核查：落库后被 Push 阻塞」。

3. **群重放的收件人 = 该 `message_id` 的 index `account_a`，不是现在的 `groups.members()`。**
   - 后加入者这次重试 **不会** 收到旧消息。已退出但仍留着 index 行的人 **会** 再收到一次 Push（at-least-once）。接受；H5 outbox 以后再说。
   - `members()` **只**在 preflight 未命中、要做 **新 insert** 时跑。identical 重试（退群后同一 clientId+dest+body）**跳过** `members()`，按 index 快照补投。这是 P1-3 相对旧稿「退群 → 107 不重放」的订正。

4. **空 `clientId` 不去重。** 每次请求新 insert，dispatch 用该次写入的 `Fanout`。行为与今天一致，只是 Push 改从 store 结构来。

5. **自聊。** index 仍是 SEND+RECV 两行、同一 account。`recipients` 去重成一个账号。`dispatch` 仍跳过本 `channel_id`；发送方其它设备仍会收到。Success 不变。

6. **非空 `clientId`：在 filter / friend / members 之前 `lookup_idempotency`。命中且请求与落库 Fanout **完全一致** → 跳过可变鉴权，直接重放。不一致 → `IdempotencyConflict=111`，不重放、不 insert。**
   - **拒绝旧稿「鉴权过了才 insert，duplicate 再忽略 dest」：** 好友关系变化、退群、词表更新会让 **完全相同的重试** 再也补不上 Push，等于把 G-09 的 duplicate 合同做成「条件式重放」。
   - **拒绝「不一致仍打给原来的 dest」**（原 G-09 建议那条单测）：客户端以为在给 carol 发，服务端却把旧 bob 消息再推一遍。改成 111，客户端换新 `clientId`。
   - **一致：** `fanout.kind` 对得上本 command（user/group）、`fanout.dest == header.dest`、`msg_type/body/extra` 等于本次 `MessageReq`。不比 recipients（请求里没有）。
   - **未命中：** 走今天的 filter / exists / friend / block / `members()`，再 insert。新产品消息仍先鉴权再落库。
   - **空 `clientId`：** 不 lookup。
   - **额外 RPC：** 生产 Chat 对每次带 `clientId` 的 talk 多一次 Royal `POST /api/v1/message/idempotency`；404 → 未命中。第一次发送 = lookup 404 + insert；identical 重试 = 一次 lookup、无 insert。G-16 的 HTTP 次数 +1，接受（否则无法在鉴权前知道 duplicate）。
   - **并发首次：** 两个请求都 miss lookup，都过鉴权，insert 的 `ON CONFLICT` 仍保证一条 content；两边都会 `persist_then_push`（at-least-once）。

7. **`kim_dispatch_fail_total` 在这些路径 +1（含 duplicate 重放）：`dispatch` 返回 Err；locations 非 NotFound；`fanout.recipients` 为空（混版本 / 幽灵 content）。**
   - label 对齐 `kim_talk_total`：`service_id`、`service_name`、`kind=user|group`。不加 `reason`（基数、H0 只要这一根计数器）。全员离线（`NotFound`）**不** +1。
   - 不改 `Context::dispatch` 的部分成功 Err。

8. **加 `MessageStore::lookup_idempotency`；不新增表，不 sqlx migrate。** 幂等表已是 `(app, sender, client_id)`。`insert_*` 仍在命中时返回带 `Fanout` 的 `InsertResult`（给 lookup 未覆盖的并发窗口）。`FailStore` 的 lookup 回 `Ok(None)`。空 members 幽灵 content 仍是 G-16/G-01 的直接 HTTP 问题。content 在、index 为空：`recipients=[]`，`kind=User`，`dest=""`，**不是** `DecodeError`。Memory 与 Postgres **共用** `fanout_from_index_rows`。

### Concrete types

`services/chat/src/store/mod.rs`：

```rust
pub struct Fanout {
    pub msg_type: i32,
    pub body: String,
    pub extra: String,
    pub sender: String,
    pub dest: String,
    pub kind: MessageKind,
    pub recipients: Vec<String>,
}

pub struct InsertResult {
    pub message_id: i64,
    pub send_time: i64,
    pub duplicate: bool,
    pub fanout: Fanout,
}
```

`recipients`：index 行 `account_a` 去重、保持写入顺序。单聊 `[sender, dest]`（自聊一个）；群聊 = insert 当时的 members 顺序。

`insert_user` / `insert_group` 的返回值变宽。另加：

```rust
async fn lookup_idempotency(
    &self,
    app: &str,
    sender: &str,
    client_id: &str,
) -> Result<Option<InsertResult>, StoreError>;
```

`client_id` 为空时 talk **不调用**。命中：`duplicate=true`，`fanout` 与 insert 命中同一套 `load_fanout`。未命中：`Ok(None)`。Postgres：`SELECT message_id FROM message_idempotency` 再 `load_fanout`。Memory：幂等 HashMap。`HttpMessageStore`：`POST /api/v1/message/idempotency`，404 → `None`。

所有 `InsertResult` 构造点（Memory、Postgres、Http）一起改，否则不能编译。`FailStore` 必须实现 lookup。

crate 内 helper（`store/mod.rs`，`pub(crate)`）。**Memory duplicate 与 Postgres duplicate 都只走 `fanout_from_index_rows`**，不要 Memory 读 `StoredMessage.kind/dest`、Postgres 走另一套。

```rust
pub(crate) fn unique_accounts(accounts: impl IntoIterator<Item = String>) -> Vec<String> {
    // HashSet 去重，保序。对齐 directory::normalize_members。
}

/// 只含真实 index 行（无 NULL）。空 slice = 幽灵 content。
pub(crate) fn fanout_from_index_rows(
    msg_type: i32,
    body: String,
    extra: String,
    rows: &[(String, String, i32, String)], // account_a, account_b, direction, group_id
) -> Fanout {
    if rows.is_empty() {
        return Fanout {
            msg_type,
            body,
            extra,
            sender: String::new(),
            dest: String::new(),
            kind: MessageKind::User,
            recipients: Vec::new(),
        };
    }
    let recipients = unique_accounts(rows.iter().map(|r| r.0.clone()));
    if let Some((_, account_b, _, group_id)) = rows.iter().find(|r| !r.3.is_empty()) {
        return Fanout {
            msg_type,
            body,
            extra,
            sender: account_b.clone(), // 群 index 的 account_b 都是发送方
            dest: group_id.clone(),
            kind: MessageKind::Group,
            recipients,
        };
    }
    let (sender, dest) = match rows.iter().find(|r| r.2 == DIRECTION_SEND) {
        Some((a, b, _, _)) => (a.clone(), b.clone()),
        None => {
            let (a, b, _, _) = &rows[0]; // RECV：account_a=dest, account_b=sender
            (b.clone(), a.clone())
        }
    };
    Fanout {
        msg_type,
        body,
        extra,
        sender,
        dest,
        kind: MessageKind::User,
        recipients,
    }
}

/// 第一次 insert：把刚写入的 index 元组交给同一 helper。
pub(crate) fn fanout_from_write(
    kind: MessageKind,
    req: &InsertMessage,
    members: &[String],
) -> Fanout {
    let tuples: Vec<(String, String, i32, String)> = match kind {
        MessageKind::User => vec![
            (req.sender.clone(), req.dest.clone(), DIRECTION_SEND, String::new()),
            (req.dest.clone(), req.sender.clone(), DIRECTION_RECV, String::new()),
        ],
        MessageKind::Group => members
            .iter()
            .map(|m| {
                let dir = if m == &req.sender {
                    DIRECTION_SEND
                } else {
                    DIRECTION_RECV
                };
                (m.clone(), req.sender.clone(), dir, req.dest.clone())
            })
            .collect(),
    };
    fanout_from_index_rows(req.msg_type, req.body.clone(), req.extra.clone(), &tuples)
}
```

自聊：SEND+RECV 同一 account → `unique_accounts` 长度 1。群：`group_id` 非空 → dest=group_id，sender=`account_b`。单聊：SEND 的 `account_a`/`account_b`，否则对调 RECV。

`inbox::parse_kind`（0=user，1=group）给 HTTP 映射复用，不要再写一份。

### proto

`crates/kim-protocol/proto/pkt.proto`，`InsertMessageResp` 字段 1–3 不动。**嵌套子消息**，不要扁平 4–10：

```protobuf
message InsertFanout {
  int32 type = 1;                 // MessageReq.type / MessagePush.type
  string body = 2;
  string extra = 3;
  string sender = 4;
  string dest = 5;                // 单聊对端账号；群聊 group id
  int32 kind = 6;                 // 0 user, 1 group（与 InboxItem.kind 相同）
  repeated string recipients = 7;
}

message InsertMessageResp {
  int64 messageId = 1;
  int64 sendTime = 2;
  bool duplicate = 3;
  InsertFanout fanout = 4;        // proto3 optional message → prost Option
}
```

`MessageResp` / `MessagePush` / `MessageReq` **不改**。长连接发送方仍只收到 id/time。

`Status` 追加（1xx，Web `isRetryable` 不覆盖）：

```protobuf
Blocked = 110;
IdempotencyConflict = 111; // same clientId, different dest/type/body/extra/kind
```

lookup HTTP（仅 Chat→Royal，不是长连接 command）：

```protobuf
message IdempotencyQuery {
  string sender = 1;
  string clientId = 2;
}
```

路径 `POST /api/v1/message/idempotency`。空 sender/clientId → 400。未命中 → **404**（`Http` 404 → `Ok(None)`，与群 NotFound 同一套）。命中 → `InsertMessageResp`（`duplicate=true` + `fanout`）。app 仍是进程 `st.app`（G-05）。

`sdk/web/scripts/gen-proto.mjs` 重写 `sdk/web/src/proto/pkt.json`。Web 运行时不用 `InsertMessageResp`（那是 Chat↔Royal HTTP）。

Royal（新版本永远填 `Some`）：

```rust
Ok(encode(&InsertMessageResp {
    message_id: inserted.message_id,
    send_time: inserted.send_time,
    duplicate: inserted.duplicate,
    fanout: Some(InsertFanout {
        r#type: inserted.fanout.msg_type,
        body: inserted.fanout.body,
        extra: inserted.fanout.extra,
        sender: inserted.fanout.sender,
        dest: inserted.fanout.dest,
        kind: match inserted.fanout.kind {
            MessageKind::User => 0,
            MessageKind::Group => 1,
        },
        recipients: inserted.fanout.recipients,
    }),
}))
```

`HttpMessageStore`：`resp.fanout` 为 `Some` 时用 `parse_kind` 映射（未知 `kind` 当 `User` 并 `warn`）。为 `None`（旧 Royal）时：

```rust
warn!("royal insert resp missing fanout");
Fanout {
    msg_type: 0,
    body: String::new(),
    extra: String::new(),
    sender: String::new(),
    dest: String::new(),
    kind: MessageKind::User,
    recipients: Vec::new(),
}
```

talk 层看到空 `recipients` 后 `warn` + `on_dispatch_fail`，不 dispatch，仍 Success。

### SQL（无 migrate）

`message_content` **没有** sender/dest；它们在 `message_index`（`account_a` / `account_b` / `group_id` / `direction`）。已有 `message_index_message_id`。`msg_type` 与 `direction` 都是 `SMALLINT`（`0001_messages.sql`）→ sqlx `i16`，再 `i32::from`。

Duplicate（early hit 与 ON CONFLICT 回读）共用 `load_fanout`。LEFT JOIN 在幽灵 content 上产出 **一行 content + index 列全 NULL**，不是 0 行。`query_as` 进非 Option `String`/`i32` 会 `DecodeError`。

```rust
/// JOIN 行。index 列在幽灵 content 时为 None。
type FanoutSqlRow = (
    i16,            // c.msg_type SMALLINT
    String,         // c.body
    String,         // c.extra
    Option<String>, // i.account_a
    Option<String>, // i.account_b
    Option<i16>,    // i.direction SMALLINT
    Option<String>, // i.group_id
);

async fn load_fanout(
    pool: &PgPool,
    app: &str,
    message_id: i64,
) -> Result<Fanout, StoreError> {
    let rows: Vec<FanoutSqlRow> = sqlx::query_as(
        "SELECT c.msg_type, c.body, c.extra,
                i.account_a, i.account_b, i.direction, i.group_id
         FROM message_content c
         LEFT JOIN message_index i
           ON i.message_id = c.id AND i.app = c.app
         WHERE c.id = $1 AND c.app = $2
         ORDER BY i.id",
    )
    .bind(message_id)
    .bind(app)
    .fetch_all(pool)
    .await
    .map_err(pg_err)?;

    let Some(first) = rows.first() else {
        return Err(StoreError::Backend("fanout missing".into()));
    };
    let msg_type = i32::from(first.0);
    let body = first.1.clone();
    let extra = first.2.clone();
    let index_rows: Vec<(String, String, i32, String)> = rows
        .into_iter()
        .filter_map(|(_, _, _, a, b, dir, gid)| {
            Some((a?, b?, i32::from(dir?), gid.unwrap_or_default()))
        })
        .collect();
    Ok(fanout_from_index_rows(msg_type, body, extra, &index_rows))
}
```

```sql
SELECT c.msg_type, c.body, c.extra,
       i.account_a, i.account_b, i.direction, i.group_id
FROM message_content c
LEFT JOIN message_index i
  ON i.message_id = c.id AND i.app = c.app
WHERE c.id = $1 AND c.app = $2
ORDER BY i.id
```

`ORDER BY i.id`：index 主键是 insert 循环里 `idgen.next_id()` 的雪花（`postgres.rs` 135–152），升序 = 写入顺序。duplicate 重放的 `recipients` 顺序才能对上 `group_two_gates_coalesce_skip_sender_and_omit_offline` 的合包顺序。幽灵行 `i.id` 为 NULL，Postgres `ORDER BY` 默认 NULLS LAST，单行无影响。

分流：

| 查询结果 | 处理 |
|---|---|
| 0 行（无 content） | `StoreError::Backend("fanout missing")` → talk 99（幂等指向空洞） |
| ≥1 行，index 列全 None（幽灵） | `filter_map` 后空 slice → `recipients=[]`，`kind=User`，`dest=""`，`sender=""`。**不是** `DecodeError` |
| ≥1 行，有 index | `fanout_from_index_rows` 按上表规则 |

第一次 insert **不要**再 SELECT：`fanout_from_write` 把刚写入的元组交给 **同一个** `fanout_from_index_rows`。空 members → 空 slice → first 与 duplicate **都是** `recipients=[]`、`kind=User`、`dest=""`、`sender=""`。Chat talk 走不到（`members()` 必含发送方）。G-16 范围不变。两端 first/duplicate 都走 helper，禁止再按 `req.kind` 填 Fanout。

Memory duplicate：`contents.get(message_id)` 只取 `msg_type/body/extra`；`indexes` 里同 `message_id` 的行按 Vec 写入顺序映射成元组，交给 **同一个** `fanout_from_index_rows`。缺 content → `StoreError`。不要用 `StoredMessage.kind/sender/dest`。

幂等表 schema 不变。

### 核查：落库后被 Push 阻塞

对照当前源码（不是文档推断）。

**今天 user 路径**（`talk.rs`）：

1. `get_locations`（128–135）在 **insert 之前**。Redis / `OtherLocationStore` 失败 → 99，**库里没有行**。
2. `insert_user`（138–159）。
3. `ctx.dispatch().await`（170–176），仅 `!duplicate && !locs.is_empty()`。
4. dispatch Err → `resp_with_error(SystemException)` 并 `return`，**不**发 Success。
5. 走到 192 才 `resp(Success, MessageResp)`。

**今天 group 路径**（251–321）：先 `insert_group`，再 `get_locations`（276），再 `dispatch`（294），最后 `resp(Success)`。insert 已成功时，locations/dispatch 失败或挂起都会让 Alice 看不到 Success。

**`dispatch` 实际在等什么**

```text
talk.rs:171     ctx.dispatch(&push, &locs).await
context.rs:120  dispatcher.push(gate_id, channel_ids, pkt).await
chat/lib.rs:81  ContainerDispatcher → Container::push (container.rs:186)
kim-tcp/server.rs:129  channels.get(gateway_id).push(payload)
channel.rs:145/220     Channel::push → send_binary
channel.rs:233         tx.send(WriteOp::Frame { ... }).await   // 无超时
```

`ChannelOpts` 注释写「满了 Push 会失败」（`channel.rs` 32–33），默认 `write_queue = 64`（41 行；`kim-tcp` / `kim-ws` server 硬编码 64）。实现是 `mpsc::Sender::send().await`：队列满时 **等腾槽，不返回 Err**。注释与代码不一致，这是 G-30；本切片 **不**改 `send_binary`。

Chat 进程里 **一条网关 TCP = 一个 Channel**。Alice 的 Success（`context.rs:164` `push_to_sender` → `dispatcher.push(session.gate_id, [alice.channel])`）和给 Bob 的 Push 若 `gate_id` 相同，抢同一条写队列。今日顺序是先等 Bob 的帧进队，再给 Alice 回 Success：Bob 慢 → 网关读 Chat 变慢 → Chat 写队列堆满 → `dispatch` 永不返回 → 消息已落库，Alice 当超时并可能重试。

读循环还在 `listener.receive(...).await`（`channel.rs:199`）。慢 Push 同时挡住这条链路上后续拆帧（G-29）。本切片只保证 **本条 talk 的 Success 不再排在本次 Bob Push 之后**；预算内 handler 仍占读循环，那是 G-29。

**选定修复（不改 Channel）**

```text
insert_* Ok 或 identical lookup 命中
  → 立刻 ctx.resp(Success, MessageResp)    // 先占用网关写队列的一个槽
  → timeout(TALK_PUSH_BUDGET, get_locations + dispatch)
  → 超时 / dispatch Err / locations 非 NotFound：warn + kim_dispatch_fail_total
  → locations NotFound（全员离线）：不 Push、不打失败指标
```

| 拒绝 | 原因 |
|---|---|
| 只把 dispatch Err 改成 Success，仍 `await dispatch` 后再 resp | 处理不了「Push 永远不返回」 |
| 本切片改 `try_send` / 满则断连 | 那是 G-30 整段 |
| 把 `resp` 也包进同一 timeout | Alice 的 Success 和 Bob 的 Push 解绑后，Success 应尽快发出；resp 失败只 warn |

剩余：网关写队列在 **本次 talk 之前**已经 64/64 满时，Alice 的 `resp` 自己也会 `send().await`。那是既有积压，不是「等这次 Bob Push」。记在 G-30。

生产 `TALK_PUSH_BUDGET = 3s`。超时 drop `dispatch` future，取消这次 `send().await`；帧可能已进队或未进队。Alice 已收到 Success；identical 重试会再 Push。单测注入 50ms。

### talk 控制流

抽出 `persist_then_push`，user/group 共用。**去掉** `if !inserted.duplicate`。**先 Success，再有界投递。**

```rust
const TALK_PUSH_BUDGET: Duration = Duration::from_secs(3);

fn fanout_matches_req(kind: MessageKind, dest: &str, req: &MessageReq, f: &Fanout) -> bool {
    f.kind == kind
        && f.dest == dest
        && f.msg_type == req.r#type
        && f.body == req.body
        && f.extra == req.extra
}

async fn persist_then_push(
    ctx: &Context,
    inserted: &InsertResult,
    kind_label: &str, // "user" | "group"
    metrics: Option<&KimMetrics>,
    push_budget: Duration,
) {
    let resp = MessageResp {
        message_id: inserted.message_id,
        send_time: inserted.send_time,
    };
    // 必须在 get_locations / dispatch 之前。发送方的 Success 不得等接收端信箱。
    if let Err(err) = ctx.resp(Status::Success, Some(&resp)).await {
        warn!(%err, "resp failed");
    }

    let push = MessagePush {
        message_id: inserted.message_id,
        r#type: inserted.fanout.msg_type,
        body: inserted.fanout.body.clone(),
        extra: inserted.fanout.extra.clone(),
        sender: inserted.fanout.sender.clone(),
        send_time: inserted.send_time,
    };
    let accounts = unique_accounts(inserted.fanout.recipients.iter().cloned());
    let push_fut = async {
        if accounts.is_empty() {
            warn!(
                message_id = inserted.message_id,
                "fanout recipients empty; skip push"
            );
            if let Some(m) = metrics {
                m.on_dispatch_fail(kind_label);
            }
            return;
        }
        let locs = match ctx.get_locations(&accounts).await {
            Ok(v) => v,
            Err(SessionError::NotFound) => Vec::new(),
            Err(err) => {
                warn!(%err, "get_locations failed");
                if let Some(m) = metrics {
                    m.on_dispatch_fail(kind_label);
                }
                Vec::new()
            }
        };
        if locs.is_empty() {
            return;
        }
        if let Err(err) = ctx.dispatch(&push, &locs).await {
            warn!(%err, "dispatch failed");
            if let Some(m) = metrics {
                m.on_dispatch_fail(kind_label);
            }
        }
    };
    if tokio::time::timeout(push_budget, push_fut).await.is_err() {
        warn!(
            message_id = inserted.message_id,
            "talk push budget exceeded"
        );
        if let Some(m) = metrics {
            m.on_dispatch_fail(kind_label);
        }
    }
    info!(
        dest = %inserted.fanout.dest,
        message_id = inserted.message_id,
        send_time = inserted.send_time,
        duplicate = inserted.duplicate,
        recipients = accounts.len(),
        "talk"
    );
}
```

`do_user_talk` / `do_group_talk` 在 decode 之后、filter 之前：

```text
if dest.is_empty() → NoDestination
if !req.client_id.is_empty():
    match store.lookup_idempotency(app, sender, client_id)
        Some(row) if fanout_matches_req(...) → persist_then_push(&row); return
        Some(_) → IdempotencyConflict=111; return
        None → 继续
        Err → SystemException; return
filter / exists / friend / block   # 仅未命中
members()                          # 仅群、未命中
insert_*
persist_then_push
```

生产 `push_budget = TALK_PUSH_BUDGET`。单元测试可传入更短预算。`RecordingDispatcher` 增加 `hang_on(gateway)`：先记下 Push（与 `fail_on` 一样），再 `std::future::pending().await`。

空 accounts 不调用 `get_locations`：Memory/Redis 实现空列表会 `NotFound`（`kim-session` memory.rs 约 84–102 行）。空 recipients 已在上面打过 `on_dispatch_fail`，不要再走 locations。

单聊 `fanout_from_write` 的 recipients 是 `[sender, dest]`（SEND 再 RECV），与今天 `do_user_talk` 的 `[dest, sender]` **相反**。`dispatch` 按 `recvs` 首次出现的 `gate_id` 合包。不冻结 1:1 网关顺序；现有测试只让 bob 在线，不受影响。

### Metrics 接线

今天 `ChatHandler` 拥有 `metrics: Mutex<Option<Arc<KimMetrics>>>`（`lib.rs` 103、431–447、518–524）；`ChatSvc` 只有目录 Arc（69–75、218–224）。Router 闭包捕获的是 `ChatSvc`。

**一把 mutex，放在 `ChatSvc`。删除 `ChatHandler.metrics`。** 同时去掉 `svc` 上的 `#[allow(dead_code)]`——handler 必须留着 `svc` 才能 `with_metrics`。

```rust
#[derive(Clone)]
pub(crate) struct ChatSvc {
    store: Arc<dyn MessageStore>,
    groups: Arc<dyn GroupDirectory>,
    filter: Arc<dyn ContentFilter>,
    users: Arc<dyn UserDirectory>,
    social: Arc<dyn SocialDirectory>,
    metrics: Arc<Mutex<Option<Arc<KimMetrics>>>>,
}

impl ChatHandler {
    pub fn with_metrics(&self, m: Arc<KimMetrics>) {
        *self.svc.metrics.lock().unwrap_or_else(|e| e.into_inner()) = Some(m);
    }

    fn metrics(&self) -> Option<Arc<KimMetrics>> {
        self.svc.metrics.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
}
```

`receive` 的 `on_talk` 走 `self.metrics()`。Handler 闭包：

```rust
let metrics = svc.metrics.lock().unwrap_or_else(|e| e.into_inner()).clone();
do_user_talk(..., metrics.as_deref()).await;
```

`main.rs` 仍 `handler.with_metrics(m.clone())`（约 245 行），不必改签名。单元测试 `serve_user_talk` 传 `None`；dispatch-fail 测试构造 `KimMetrics::new("chat-test", "chat")`，用 `registry().gather()` 断言计数（不必绑 HTTP）。

```rust
// crates/kim-metrics/src/lib.rs
let dispatch_fail_total = IntCounterVec::new(
    Opts::new("kim_dispatch_fail_total", "talk persist ok but online push did not complete"),
    &["service_id", "service_name", "kind"],
)?;

pub fn on_dispatch_fail(&self, kind: &str) {
    self.dispatch_fail_total
        .with_label_values(&[self.service_id.as_str(), self.service_name.as_str(), kind])
        .inc();
}
```

`kind` 只允许 `"user"` / `"group"`（与 `on_talk` 相同）。locations 非 NotFound、`dispatch` Err、空 `recipients` 共用这根计数器。

### 文档（代码 PR 落地时改，本设计 PR 只挂指针）

`docs/control-layer-chat.md`：

- Status 表（约 48 行）：`SystemException=99` 改为 insert / members / exists·friend·block **存储**失败。去掉「dispatch 失败」。
- 约 58 行同类。
- 约 75 行 duplicate：「非空 clientId 先查幂等。字段完全一致则返回第一次的 `messageId` / `sendTime`，从落库重建 Push 并有界尽力 dispatch；dest/body/type/extra/kind 不一致 → `IdempotencyConflict=111`。」
- 「在线不是确定值」表（约 91–96）：「在线但网关 push 失败 / 超时」→ 发送方 **已经 Success**；可能部分成员已收到；失败打 `kim_dispatch_fail_total`。**不要**写「漏 Push 靠 offline pull 所以 G-09 已关」。补一句：ACK 仍是 Snowflake 高水位（G-03）；Flutter 仍不拉（G-14）；G-09 条目保留。

`docs/reliable-delivery.md`：写扩散段落后加：落库即成功；在线 Push 尽力；duplicate 从 `message_content` + `message_index` 重建，不信本次请求。ACK 模型本页其余段落不动，并标明 G-03。

`docs/observability.md` 指标列表加上 `kim_dispatch_fail_total`。

合入后 **保留 G-09 条目**，在节首加一段（对齐 G-02/G-08）：

> **Chat 错误语义（本切片已落地）：** insert Ok 后发送方有界时间内 Success；identical clientId 从 index 重建并补投；改 payload → 111。**未关：** 漏 Push 的可靠补偿（G-03 高水位、G-14 Flutter 不 pull）。mailbox `try_send` 仍是 G-30；talk 路径已用预算包住 dispatch，其它 Push（群 Notify 等）未改。

同一提交还要：

- 建议修复顺序表第 8 行 **不要划掉**；改成「G-09 服务端错误语义已修；漏 Push 补偿见 G-03 / G-14」。
- 第 17 行「改掉 dispatch fail → 99 单测 | G-15, G-09」→ 去掉「改掉单测」，G-09 只留「漏 Push 补偿」。
- G-15 正文「G-09 的单测把错误语义写成契约」删掉；改成「`kim_dispatch_fail_total` 已有；缺口是 send→ack 延迟、Royal RPC、告警规则」。
- H0 persist-first 标「talk 已做（先 Success 再有界 dispatch）」；留下 try_send。

---

## Phase 1 — store + proto + Royal HTTP

可单独编译：talk 仍只用 `message_id` / `send_time` / `duplicate`，但所有 `InsertResult { ... }` 构造必须带上 `fanout`，否则 workspace 过不了。

**File: `crates/kim-protocol/proto/pkt.proto`**

- 新增 `InsertFanout`；`InsertMessageResp.fanout = 4`。`build.rs` 已 `compile_protos`，不必改 build。

**File: `sdk/web/src/proto/pkt.json`**

- `node sdk/web/scripts/gen-proto.mjs`。

**File: `services/chat/src/store/mod.rs`**

- 加 `Fanout`；扩展 `InsertResult`。
- `lookup_idempotency`。
- `fanout_from_write` / `fanout_from_index_rows` / `unique_accounts`。
- Memory first：`fanout_from_write`。
- Memory duplicate 与 lookup：content 取 body/type/extra；index 映射成元组后走 **同一** `fanout_from_index_rows`。缺 content → `StoreError`。
- 测试：
  - `insert_user_client_id_is_idempotent`：第二次改 `body`/`dest`，`duplicate=true`，`fanout.body` 仍是 `"hi"`，`fanout.dest=="bob"`，`recipients` 含 alice/bob。
  - `insert_group` 第二次换 members 列表，`recipients` 仍是第一次的三人。
  - 自聊 `recipients` 去重后长度为 1。
  - 现有 `empty_members_writes_content_without_index`：first `InsertResult.fanout` 为 `recipients=[]`、`kind=User`、`dest=""`（不是 Group / `req.dest`）。
  - `empty_members_duplicate_is_empty_recipients_not_error`：同一 `client_id` 再 insert → `duplicate=true`，fanout 同样是空 recipients / User / 空 dest，不是 `StoreError`。

**File: `services/chat/src/store/postgres.rs`**

- first：`fanout_from_write`，无额外 SELECT。
- `lookup_idempotency` + insert 的 early hit / ON CONFLICT 回读：共用 `load_fanout`。
- `#[cfg(test)]`：有 `DATABASE_URL` 时 `postgres_duplicate_reloads_fanout`（换 body/dest，断言 fanout）；可选同一幽灵 duplicate 断言 `recipients=[]`。

**File: `services/chat/src/royal.rs`**

- `HttpMessageStore::insert_user` / `insert_group` 映射 `resp.fanout`；`None` → 空 `Fanout` + warn。
- `lookup_idempotency` → `POST /api/v1/message/idempotency`；`Http { status: 404 }` → `Ok(None)`。

**File: `services/royal/src/lib.rs`**

- `insert_user` / `insert_group` 编码 `InsertMessageResp { fanout: Some(...) }`。
- `POST /api/v1/message/idempotency`：解码 `IdempotencyQuery`；空字段 400；lookup；未命中 404。
- `http_create_join_detail`（约 561 行）断言 first insert 的 `fanout.body` / `recipients`。
- **新增** `http_lookup_idempotency_hit_and_404`：insert 一次后 lookup 同一 clientId → `duplicate` + 原文 fanout；改 body 的冲突在 talk 层测，这里只测 store 返回原文。未知 clientId → 404 / `Ok(None)`。

本 phase **不**改 talk 行为：dispatch 仍 99。目的是类型与 HTTP 先对齐。

---

## Phase 2 — talk 行为、指标、单测

**File: `crates/kim-metrics/src/lib.rs`**

- `dispatch_fail_total`、`on_dispatch_fail`、register。

**File: `crates/kim-metrics/tests/scrape.rs`**

- `on_dispatch_fail("user")` 之后 scrape 含 `kim_dispatch_fail_total`。

**File: `services/chat/src/lib.rs`**

- `ChatSvc.metrics: Arc<Mutex<Option<Arc<KimMetrics>>>>`。
- **删除** `ChatHandler.metrics` 和 `svc` 上的 `#[allow(dead_code)]`。
- `with_metrics` / `metrics()` / `receive` 的 `on_talk` 只读写 `self.svc.metrics`。
- user/group handler 把 `metrics.as_deref()` 传进 talk。

**File: `services/chat/src/talk.rs`**

- `do_user_talk` / `do_group_talk` 增加 `metrics: Option<&KimMetrics>`；preflight lookup。
- 单聊删 insert 前 `get_locations`。
- `persist_then_push`：先 Success，再 `timeout(push_budget, locations+dispatch)`。
- `FailStore`：实现 `lookup_idempotency` → `Ok(None)`。
- `RecordingDispatcher::hang_on`。

**改写现有测试：**

| 测试 | 新契约 |
|---|---|
| `dispatch_fail_is_system_exception_without_success_resp`（约 813） | **改名** `dispatch_fail_is_success_with_message_resp`。`RecordingDispatcher::fail_on("wg-2")`。断言：一条 Flag=Push（尝试过）；一条 Success + 可解的 `MessageResp`；`message_id` 与 store 一致。带 `KimMetrics` 时 `kim_dispatch_fail_total{kind="user"}` = 1 |
| `same_client_id_does_not_insert_or_push_twice`（约 648） | **改名** `same_client_id_replays_push_from_store`。insert 仍 1；Push **2** 次；两次 Success 同一 id/time；两次 Push body 都是第一次 |
| `get_location_other_is_system_exception_without_insert_or_push`（约 844） | **改名** `get_location_other_is_success_after_insert_without_push`。库 1 行；Success；无 Push；`kim_dispatch_fail_total` +1 |
| `group_get_locations_other_is_system_exception_after_insert`（约 1237） | **改名** `group_get_location_other_is_success_after_insert_without_push`。insert 1；Success；无 Push；指标 +1 |

**新增测试：**

1. `same_client_id_changed_body_is_idempotency_conflict`：第一次 dest=bob、body=`hello`、`c1`。第二次同一 dest、body=`CHANGED`。bob 在线。断言：`IdempotencyConflict=111`；store 仍 1 条原文；Push 仍 1（只有第一次）。**不要**把原文再推一遍。
2. `same_client_id_changed_dest_is_idempotency_conflict`：alice 与 bob、carol 都是好友。第二次 dest=carol、同一 body。111；carol 从未收到；bob 只有第一次 Push。
3. `identical_retry_after_unfriend_still_replays`：第一次 dest=bob 成功。去掉好友。第二次 **完全相同** 的 clientId/dest/body → Success；第二次 Push 仍到 bob（跳过 NotFriends）。
4. `group_duplicate_uses_index_snapshot_not_current_members`：`g1` 先 `[alice,bob]`，talk 一次（bob 在线）。`seed` 成 `[alice,carol]`。identical 再 talk。第二次 Push 仍到 bob，**不到** carol。
5. `identical_retry_after_quit_still_replays`：alice 发群消息后 quit。identical clientId/dest/body 再发 → **不是** 107；按原 index 再 Push 给当时的成员。
6. `group_duplicate_changed_dest_is_idempotency_conflict`：alice 在 `g1` 与 `g2`。第一次 dest=`g1`。第二次 dest=`g2`、同一 body → 111；不把 `g1` 消息打到 `g2`。
7. `empty_client_id_inserts_and_pushes_twice`：两次空 clientId，两条 content、两次 Push、两个 id。
8. `self_chat_success_skips_own_channel`：dest=alice；session `ch-alice`。再 `cache.add` `ch-alice-web`。Success；无 Push 到 `ch-alice`；**有** Push 到 `ch-alice-web`。
9. `duplicate_dispatch_fail_still_success_and_metric`：第一次 `fail_on`（仍 Success）；identical 第二次再 fail；两次 Success；指标 ≥1。
10. `dispatch_hang_still_success_within_budget`：Bob 在 `wg-2`。`hang_on("wg-2")`（先记下 Push，再 `pending()`）。`push_budget=50ms`。断言：`store.recorded().len()==1`；Alice 的 Response 是 Success + 可解 `MessageResp`（`message_id` 对得上那一行）；`serve` 在 <200ms 返回；`kim_dispatch_fail_total{kind="user"}` = 1。
11. `get_locations_hang_still_success_within_budget`：`HangLocationStore`（`get_locations` 里 `pending()`）。insert 仍发生。`push_budget=50ms`。断言：库 1 行；Alice Success；无 Push；指标 +1。
12. `offline_receiver_success_without_dispatch_fail_metric`：Bob 不在线（Memory 空 loc → `NotFound`）。库 1 行；Success；无 Push；**不**增加 `kim_dispatch_fail_total`。

**File: `crates/kim-router` test_support**（或 chat 测试里的 dispatcher）

- `RecordingDispatcher::hang_on`：记下 Push 后 `std::future::pending().await`。不是生产 `Dispatcher` 语义变更。
- talk 测试里 `HangLocationStore`：`get_locations` 永不返回。

群 `group_two_gates_coalesce_skip_sender_and_omit_offline` 等 happy path 应继续绿：first insert 的 `Fanout.recipients` 等于当时 members。

**File: `services/chat/tests/e2e_talk.rs`**

- identical `clientId` 再发：bob 第二帧 Push 仍是原文（Success）。
- 改 body 的同一 `clientId`：响应 `IdempotencyConflict`，bob 不再多一帧。
- 用现有 `spawn_stack` / `become_friends`。HTTP lookup 见 Phase 1 Royal 测试。

---

## Phase 3 — 专题文档

代码 PR 的最后一组提交（仍同一 PR）。

**File: `docs/control-layer-chat.md`** — Status 表、duplicate 句、在线表。见 Design 文档节。

**File: `docs/reliable-delivery.md`** — persist-first 一段；ACK 标明 G-03。

**File: `docs/observability.md`** — `kim_dispatch_fail_total`。

**File: `docs/production-gaps.md`** — **保留 G-09**，节首注明错误语义已修、漏 Push 补偿未关；顺序表第 8 行改写不删除；G-15 去掉「单测把 99 锁死」；H0 talk persist-first 标已做，留下 try_send。

**File: `docs/impl/README.md`** — 切片 2 标已合入。

不改 `docs/architecture.md`、`docs/protocol-container.md`、`docs/web-sdk.md`（`isRetryable` 未改；可在 control-layer 提一句 99 不再表示「已落库」）。

---

## Phase Verify

```bash
cargo test -p kim-metrics
cargo test -p chat --lib talk
cargo test -p chat --lib store
cargo test -p royal
cargo test -p chat --test e2e_talk
# 有 Postgres 时：
DATABASE_URL=postgres://... cargo test -p chat --features postgres --lib store::postgres
node sdk/web/scripts/gen-proto.mjs
cargo fmt --all -- --check
cargo clippy -p chat -p royal -p kim-metrics -p kim-protocol -- -D warnings
```

人工扫：`talk.rs` 里 `MessagePush` 不得再用 `req.body`。`do_user_talk` 在 `insert_user` 之前不得 `get_locations`。`persist_then_push` 里 `ctx.resp(Success)` 的源码行号必须 **小于** `timeout(... dispatch)`。`lookup_idempotency` 在 `filter.check` / `is_friend` / `groups.members` 之前。`!inserted.duplicate` 不得挡住 dispatch。

验收（本 bug）：

- `dispatch_hang_still_success_within_budget`：库 1 行；Alice 短时限内 Success + `MessageResp`；指标 +1。
- `get_locations_hang_still_success_within_budget`：同上，无 Push，指标 +1。
- locations 非 NotFound / `dispatch` Err / 超时：指标 +1，Alice 仍 Success。
- Bob 离线（NotFound）：Success，**指标不 +1**。

---

## Architectural Notes

- **at-least-once 是故意的。** duplicate 总是再 dispatch。第一次已经 Push 成功、客户端只是没收到 Success 而重试，对端会再收到同一 `messageId`。SDK 去重（Web 对 Push 与 offline content 已按 id 去重）。进程内「是否 dispatch 过」以及 `dispatch` 的 Err 都不能当权威：部分网关可能已经成功。
- **persist-first 之后，发送方看到 Success 不再保证对端在线收到。** 补偿两条：(1) 发送方没收到 Resp → **identical** `clientId` 重试 → 从库重放 Push；(2) 已收到 Success → 客户端不再试。**(2) 不是可靠补偿：** G-03 高水位会吞漏 Push 的 id；G-14 Flutter 根本不 pull。因此 **不能删除 G-09**。
- **Young 连接 / 写队列满** 今天会 99，或在 `send().await` 上挂死（注释谎称会失败）。之后 Alice 在本次 Push **之前**收到 Success；Bob 可能仍没 Push。队列在本次 talk 之前已经满，是 G-30。
- **同一 `gate_id`。** Chat→该网关只有一个 `Channel`（`write_queue=64`）。先 `resp` 再 `dispatch`，是让本条 Success 抢在本条 Bob Push 前面进队。不把 Channel 改成 `try_send`。
- **G-03 仍然会丢。** 重放 Push 若晚于一个更大的 `messageId` 被 ACK，offline pull 仍可能跳过。
- **不改 `Context::dispatch` / `send_binary`。** 部分成功仍返回第一个 Err；handler 不把它变成 99。talk 用 `timeout` 包住这次 await，避免 Success 被永久卡住。G-30 再改 `try_send`。
- **lookup 多一次 Royal HTTP。** 带 `clientId` 的首次 talk：idempotency 404 + insert。这是在鉴权前识别 duplicate 的代价，不接受「条件式重放」。
- **H5 成员快照已经在 index 里。** 不要为 G-09 加 outbox。大群分批是 G-25。
- **Web `isRetryable`。** 99 离开 persist-success 路径之后，Web 不会把已落库当失败重试。剩下的 99 是真的 insert/目录故障，不应盲目重试。本切片不改 `status.ts`。
- **Royal 仍丢掉 `session.app`。** insert 仍写进程 `st.app`。G-05 / G-01。Fanout 不引入 app 字段。
- **混版本不是空 Push。** `persist_then_push` 在 `recipients=[]` 时根本不 `dispatch`。发送方 Success，对端零 Push，指标必须亮。发布：先 Royal / 共享镜像，再 Chat；回滚先 Chat。`chat-gray` 与 `chat` 共用一个 Royal（`compose.yml` `ROYAL_URL`），只滚灰色 Chat 会踩这个洞。

---

## File Change Summary

crate / 路径字母序，一行一个文件。实现 PR 相对 `main`。

| 文件 | 变更 |
|---|---|
| `crates/kim-metrics/src/lib.rs` | `kim_dispatch_fail_total` + `on_dispatch_fail(kind)` |
| `crates/kim-metrics/tests/scrape.rs` | scrape 含该计数器 |
| `crates/kim-protocol/proto/pkt.proto` | `InsertFanout`；`InsertMessageResp.fanout = 4`；`IdempotencyConflict=111`；`IdempotencyQuery` |
| `crates/kim-router/src/test_support.rs` | `RecordingDispatcher::hang_on` |
| `docs/control-layer-chat.md` | 99 不再含 dispatch fail；identical duplicate 重放；111 冲突 |
| `docs/impl/README.md` | 切片 2 标已合入；G-09 不删 |
| `docs/observability.md` | 列出 `kim_dispatch_fail_total` |
| `docs/production-gaps.md` | **保留 G-09** 并拆已修/未修；顺序表第 8 行改写；G-15 正文；H0 |
| `docs/reliable-delivery.md` | 落库真相、先 Success 再有界 Push、identical 才重放 |
| `sdk/web/src/proto/pkt.json` | gen-proto |
| `services/chat/src/lib.rs` | `ChatSvc.metrics` 唯一 mutex；删 `ChatHandler.metrics` |
| `services/chat/src/royal.rs` | 映射 `Option<InsertFanout>`；`lookup_idempotency` HTTP |
| `services/chat/src/store/mod.rs` | `Fanout` / `InsertResult` / `lookup_idempotency`；共用 helper |
| `services/chat/src/store/postgres.rs` | lookup + duplicate JOIN load |
| `services/chat/src/talk.rs` | preflight；`persist_then_push` 先 resp 再 timeout |
| `services/chat/tests/e2e_talk.rs` | identical 重放；改 body → 111 |
| `services/royal/src/lib.rs` | 编码 fanout；idempotency 路由 |

不改：`crates/kim-core`（`send_binary` 仍无超时）、生产 `Dispatcher` 语义、`sdk/web/src/status.ts`、`sdk/mobile`、`crates/kim-client`、migrations、`docs/architecture.md`、`docs/protocol-container.md`。

---

## Key Decisions

1. 方案 A：enrich `InsertResult` + 嵌套 `InsertFanout fanout = 4`。拒绝二次 `load_fanout` HTTP 当 insert 的第二枪，但 **接受** 鉴权前的 `lookup_idempotency`。
2. 先 `resp(Success)`，再 `timeout(TALK_PUSH_BUDGET, get_locations+dispatch)`。不改 `send_binary`。Chat→网关一条 Channel、`send().await` 无超时（注释与实现不符）。hang dispatch / hang locations 两条单测。离线 NotFound 不加失败指标。
3. 群收件人 = 发送时 index 行。identical 重试跳过 `members()`。
4. 空 `clientId` 不去重、不 lookup。
5. 自聊 recipients 去重；dispatch 仍跳过本 channel。
6. preflight：字段完全一致才重放；dest/body/kind 不一致 → `IdempotencyConflict=111`。拒绝条件式重放，拒绝静默打到原 dest。
7. `kim_dispatch_fail_total`：dispatch Err、locations 故障、空 recipients、**push 预算耗尽**。全员离线 NotFound 不加。
8. 新增 `lookup_idempotency`；无新表、无 migrate。不删 G-09。不做 connect-time sync，不改 ACK，不改 `isRetryable`。
9. 一个实现 PR。升级先 Royal 再 Chat。混版本 = Success + 无 Push + 指标。

---

## PR Plan

切片 1 是单 PR。本切片同样：**一个实现 PR**。Phase 1 的类型加宽会迫使所有 `InsertResult` 构造一起改，拆成「只合 proto」的 PR 会留下 talk 仍 99、HTTP 已带 fanout 的半成品，review 负担并不更小。

### PR 1: persist-first：insert 成功即 Success，duplicate 从落库重建 Push

- **Description:** G-09 的服务端错误语义 + identical clientId 重放，**不是整条关闭**。`lookup_idempotency` 在鉴权前；完全一致从 index 补投；dest/body 不一致 → 111。`persist_then_push` 先 Success，再有界 dispatch。改写把 99 锁死的单测。合入后 **保留** G-09，注明漏 Push 补偿仍依赖 G-03/G-14。
- **Deploy / rollback:** 升级先 Royal 或一次发 `KIM_IMAGE`，再 Chat（含 `chat-gray`）。回滚先 Chat 再 Royal。禁止新 Chat 对着旧 Royal。
- **Files/components affected:** 见 File Change Summary。
- **Dependencies:** 无。不把 G-03/G-14/G-30 整段纳入本 PR；文档写明这三项仍开。

若 review 强求拆分，唯一干净的切法是：(1) proto + `InsertResult` + Memory/Postgres/Http/Royal load（本文件 Phase 1）；(2) talk + metrics + 测试 + 文档（Phase 2–3）。(1) 必须独立可测 store 的 lookup/insert fanout；(2) 依赖 (1)。不要第三 PR。talk 层改 body → 111，store 层同一 clientId 仍返回原文 Fanout。
