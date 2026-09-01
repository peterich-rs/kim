# 落库即成功并从持久化重建 Push

对应 [production-gaps.md](../production-gaps.md) **G-09**（H0 语义里「insert 成功 → `MessageResp`；duplicate 从落库 content/index 重建再 dispatch」那一段）。**不**把 H0 的 `try_send` / `kim_mailbox_full_total`（G-30）拉进本切片。

本切片合入后 **G-09 本身可以从 production-gaps 删除**（不像 G-02 / G-08 还要等 G-01）。本设计 PR 只在 gaps「实施节奏」挂一行指针；删条目等代码落地。

**明确不关：**

| 条目 | 本切片之后 |
|---|---|
| G-03 / G-04 / G-10 | ACK 仍是 Snowflake 高水位；`chat.talk.ack` / `offline.index` 不改 |
| G-14 | Flutter / `kim-client` 登录后仍不 pull；**不**在 connect 时服务端 sync inbox（G-09 原文「连接建立时服务端 sync」归 G-14） |
| G-30 | mailbox 仍 `send().await`；无 `kim_mailbox_full_total` |
| G-01 | Royal HTTP 仍裸；Compose Redis / Consul 不变 |
| G-05 | 寻址仍只有 account |
| H5 | 无 outbox 表、无分批 index；发送时成员快照 = 当时写下的 `message_index` 行 |

漏 Push 的设备仍靠 `chat.offline.index` / `content` 补洞。Web 会拉；Flutter 不会（G-14）。

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
| `do_user_talk`（`services/chat/src/talk.rs` 37–195） | dest 空 / decode / filter / exists / block / friend → **先** `get_locations(dest[+sender])` → `insert_user` → 用 **本次** `req` 组 `MessagePush` → `if !duplicate && !locs.is_empty() { dispatch }`；dispatch Err → `Status::SystemException`，**不**发 Success | locations 挪到 insert 之后；Push 与收件人只来自 `InsertResult.fanout`；dispatch / locations 失败 → Success + 指标 |
| `do_group_talk`（同文件 197–322） | filter → `members()` → `insert_group(members)` → `get_locations(members)` → 同上 duplicate 跳过、dispatch Err=99 | `members()` 仍服务 **新 insert** 与 NotGroupMember；dispatch 用 index 快照，不是当前 `group_members` |
| `InsertResult`（`store/mod.rs` 53–57） | `{ message_id, send_time, duplicate }` | 嵌 `Fanout` |
| Memory duplicate（`store/mod.rs` 304–313、369–376） | 幂等表命中只回 id/time | 命中后从 `contents` + `indexes` 填 `Fanout` |
| Postgres `insert_fanout`（`store/postgres.rs` 65–198） | `SELECT message_id, send_time FROM message_idempotency`；ON CONFLICT 回滚后再读同样两列 | 同上，再 JOIN content/index |
| `HttpMessageStore`（`royal.rs` 149–211） | 映射三字段 | 映射新 proto 字段 |
| Royal `insert_user` / `insert_group`（`services/royal/src/lib.rs` 208–283） | 编码三字段 | 编码 fanout |
| `InsertMessageResp`（`pkt.proto` 186–190） | 三个字段 | proto3 嵌套 `InsertFanout fanout = 4`（prost `Option`） |
| `Context::dispatch` | 跳过本 session `channel_id`；按 `gate_id` 合包；先记第一个 Err | **不改** |
| `KimMetrics`（`crates/kim-metrics/src/lib.rs`） | `kim_talk_total{kind}`；`on_talk` 在 `ChatHandler::receive`（`lib.rs` 518–524） | 新增 `kim_dispatch_fail_total`；talk handler 在 dispatch/locations 失败时 `inc` |
| `FailStore`（`talk.rs` 486–565） | 实现全部 `MessageStore` 方法 | 无新 trait 方法则不改签名 |
| `same_client_id_does_not_insert_or_push_twice`（约 648） | 两次 Success、同一 id；**断言 Push 恰好 1** | 改为两次 Push（at-least-once 重放） |
| Web `isRetryable`（`sdk/web/src/status.ts` 38–39） | 仅 300–399 | **不改**。persist 成功不再回 99，Web 不会把「已落库」当失败重试 |
| `docs/control-layer-chat.md` 48、58、75、91–96 | SystemException 含 dispatch 失败；duplicate「不再 Push」 | 代码落地时改文档 |

单聊今天 **insert 前进寻址**；群聊 **insert 后再寻址**。locations 非 `NotFound` 时：单聊 99 且 **未 insert**（`get_location_other_is_system_exception_without_insert_or_push`，约 844）；群聊 99 且 **已 insert**（`group_get_locations_other_is_system_exception_after_insert`，约 1237）。统一到 insert 之后后，两条都变成「已 insert + Success + 无 Push」。

幂等键仍是 `(app, sender, client_id)`（`0005_message_idempotency.sql`）。**不含 dest**。同一 `clientId` 改 dest 仍命中第一次的行。

---

## Design

落库是真相，在线 Push 是尽力。`insert_*` 返回 `Ok` 之后，发送方永远看到 `Status::Success` + `MessageResp{message_id, send_time}`。网关 push 失败、locations 存储故障、duplicate 重放失败，只 `warn` + `kim_dispatch_fail_total`。客户端按 `messageId` 去重。

`talk.rs` **禁止**再用 `req.r#type` / `req.body` / `req.extra` / `header.dest` / 当前 `members()` 组 `MessagePush` 或 dispatch 目标。唯一输入是 `InsertResult`（含第一次刚写入的同一结构）。

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

2. **`get_locations` 统一到 insert 之后，账号来自持久化收件人。**
   - **选定：** persist 成功后对 `Fanout.recipients` 去重再 `get_locations`。`NotFound` → 空 loc、Success、不 Push（离线）。其它 `SessionError` → warn + `kim_dispatch_fail_total` + Success，**不** 99。
   - **拒绝维持单聊「先寻址再 insert」：** 重试若改 dest，会按**新 dest** 寻址；且 locations 故障会挡住尚未发生的 insert（今天单聊就是这样）。统一之后，错误 dest 的重试只要通过 friend 检查，insert 仍可能 duplicate，寻址走**原 dest 的 index**。
   - 这会改写 `get_location_other_is_system_exception_without_insert_or_push`：insert **会**发生，响应是 Success。群路径 `group_get_locations_other_is_system_exception_after_insert` 同样改为 Success（insert 本来就会发生）。

3. **群 duplicate 的收件人 = 该 `message_id` 的 index `account_a`，不是现在的 `groups.members()`。**
   - 后加入者这次重试 **不会** 收到旧消息。已退出但仍留着 index 行的人 **会** 再收到一次 Push（at-least-once）。接受；H5 outbox 以后再说。
   - `members()` 仍在 insert **前**跑：未知群 / 发送方不在当前成员 → `NotGroupMember`，不 insert、不重放。发送方退群后再拿同一 `clientId` 重试，会 107、不会补 Push。这与「friend 检查打在请求 dest 上」对称。

4. **空 `clientId` 不去重。** 每次请求新 insert，dispatch 用该次写入的 `Fanout`。行为与今天一致，只是 Push 改从 store 结构来。

5. **自聊。** index 仍是 SEND+RECV 两行、同一 account。`recipients` 去重成一个账号。`dispatch` 仍跳过本 `channel_id`；发送方其它设备仍会收到。Success 不变。

6. **filter / exists / friend / block 仍打在本次请求 dest 上；一旦 `duplicate==true`，忽略本次 dest/body。**
   - 无法在 insert 前知道 duplicate（除非先查幂等表，多一次往返且与 `ON CONFLICT` 重复）。新产品消息仍必须先鉴权再落库，避免未好友/拉黑/不存在用户的正文进库。
   - **dest 不一致（必测）：** 第一次 dest=bob。第二次同一 `clientId`、dest=carol、body 改掉。
     - 与 carol 不是好友 → `NotFriends`，**不**重放（bob 那条仍在库里，没有额外 Push）。
     - 与 carol 是好友 → `insert_user` 返回 duplicate + **bob 的 Fanout** → Push 仍打给 bob，正文仍是第一次的。carol 不应收到。
   - filter 同样看本次 body：重试 body 命中词表 → `ContentBlocked`，不重放。接受，与 dest 鉴权同一类脚枪。
   - 群 dest 不一致（两条都必测）：
     - 发送方也在 `g2`：insert 仍可能 duplicate 第一群，fanout 用 `g1` 的 index，不是 `g2` 当前成员。
     - 发送方不是 `g2` 成员：`members()` 在 insert 前失败 → `NotGroupMember=107`，**不**重放。

7. **`kim_dispatch_fail_total` 在这些路径 +1（含 duplicate 重放）：`dispatch` 返回 Err；locations 非 NotFound；`fanout.recipients` 为空（混版本 / 幽灵 content）。**
   - label 对齐 `kim_talk_total`：`service_id`、`service_name`、`kind=user|group`。不加 `reason`（基数、H0 只要这一根计数器）。全员离线（`NotFound`）**不** +1。
   - 不改 `Context::dispatch` 的部分成功 Err。

8. **不新增 trait 方法，不新增表，不 sqlx migrate。** duplicate 的 load 收在各 `insert_*` 实现内部。`FailStore` 继续只返回 `Err`。空 members 幽灵 content（`empty_members_writes_content_without_index`）仍是 G-16/G-01 的直接 HTTP 问题；Chat talk 的 `members()` 必含发送方，不会走到这条。content 在、index 为空（first 与 duplicate 相同）：`recipients=[]`，`kind=User`，`dest=""`，**不是** `DecodeError` / `StoreError`。Memory 与 Postgres **共用** `fanout_from_index_rows`，禁止 Memory 走 `StoredMessage.kind/dest` 另写一套。

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

`MessageStore` trait **不**加方法。`insert_user` / `insert_group` 的返回值变宽。所有构造点（Memory、Postgres、`HttpMessageStore`）一起改，否则不能编译。

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

### talk 控制流

抽出 `persist_then_push`，user/group 共用。**去掉** `if !inserted.duplicate`。

```rust
async fn persist_then_push(
    ctx: &Context,
    inserted: &InsertResult,
    kind_label: &str, // "user" | "group"
    metrics: Option<&KimMetrics>,
) {
    let push = MessagePush {
        message_id: inserted.message_id,
        r#type: inserted.fanout.msg_type,
        body: inserted.fanout.body.clone(),
        extra: inserted.fanout.extra.clone(),
        sender: inserted.fanout.sender.clone(),
        send_time: inserted.send_time,
    };
    let accounts = unique_accounts(inserted.fanout.recipients.iter().cloned());
    if accounts.is_empty() {
        // 混版本（旧 Royal fanout=None）或幽灵 content：看起来像全员离线，必须打点。
        // 全员离线（get_locations NotFound）不走这里。
        warn!(
            message_id = inserted.message_id,
            "fanout recipients empty; skip push"
        );
        if let Some(m) = metrics {
            m.on_dispatch_fail(kind_label);
        }
    }
    let locs = if accounts.is_empty() {
        Vec::new()
    } else {
        match ctx.get_locations(&accounts).await {
            Ok(v) => v,
            Err(SessionError::NotFound) => Vec::new(),
            Err(err) => {
                warn!(%err, "get_locations failed");
                if let Some(m) = metrics {
                    m.on_dispatch_fail(kind_label);
                }
                Vec::new()
            }
        }
    };
    if !locs.is_empty() {
        if let Err(err) = ctx.dispatch(&push, &locs).await {
            warn!(%err, "dispatch failed");
            if let Some(m) = metrics {
                m.on_dispatch_fail(kind_label);
            }
        }
    }
    let resp = MessageResp {
        message_id: inserted.message_id,
        send_time: inserted.send_time,
    };
    info!(
        dest = %inserted.fanout.dest,
        message_id = inserted.message_id,
        send_time = inserted.send_time,
        duplicate = inserted.duplicate,
        recipients = accounts.len(),
        loc_count = locs.len(),
        msg_type = push.r#type,
        body_len = push.body.len(),
        "talk"
    );
    if let Err(err) = ctx.resp(Status::Success, Some(&resp)).await {
        warn!(%err, "resp failed");
    }
}
```

`do_user_talk` 在 friend/block/exists 成功之后立刻 `insert_user`（删掉 insert 前的 `get_locations`）。`do_group_talk` 仍先 `members()` 再 `insert_group`，然后只调 `persist_then_push`，不要对 `members` 寻址。

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
- 约 75 行 duplicate：「命中则返回第一次的 `messageId` / `sendTime`，不再 insert；**仍从落库重建 `MessagePush` 并尽力 dispatch**。忽略本次 body/extra/type/dest。」
- 「在线不是确定值」表（约 91–96）：「在线但网关 push 失败」→ 发送方 **Success**；可能部分成员已收到；失败打 `kim_dispatch_fail_total`。补一句：ACK 仍是 Snowflake 高水位（G-03 未改）；漏 Push 靠 offline pull（Flutter 仍不拉，G-14）。

`docs/reliable-delivery.md`：写扩散段落后加：落库即成功；在线 Push 尽力；duplicate 从 `message_content` + `message_index` 重建，不信本次请求。ACK 模型本页其余段落不动，并标明 G-03。

`docs/observability.md` 指标列表加上 `kim_dispatch_fail_total`。

合入后从 `docs/production-gaps.md` **删除 G-09 整节**，建议修复顺序表第 8 行划掉。同一提交还要：

- 顺序表第 17 行现在是「投递指标；改掉 dispatch fail → 99 单测 | G-15, G-09」→ 去掉 G-09，改成只留 G-15（例如「send→ack / Royal RPC / 告警规则」）。
- G-15 正文（约 494 行）「更糟的是 G-09 的单测把错误语义写成契约」删掉；改成「`kim_dispatch_fail_total` 已有；缺口是 send→ack 延迟、Royal RPC、告警规则」。
- H0 三条里 persist-first 标已做，留下 try_send。

---

## Phase 1 — store + proto + Royal HTTP

可单独编译：talk 仍只用 `message_id` / `send_time` / `duplicate`，但所有 `InsertResult { ... }` 构造必须带上 `fanout`，否则 workspace 过不了。

**File: `crates/kim-protocol/proto/pkt.proto`**

- 新增 `InsertFanout`；`InsertMessageResp.fanout = 4`。`build.rs` 已 `compile_protos`，不必改 build。

**File: `sdk/web/src/proto/pkt.json`**

- `node sdk/web/scripts/gen-proto.mjs`。

**File: `services/chat/src/store/mod.rs`**

- 加 `Fanout`；扩展 `InsertResult`。
- `fanout_from_write` / `fanout_from_index_rows` / `unique_accounts`。
- Memory first：`fanout_from_write`。
- Memory duplicate（读锁命中与写锁二次检查）：content 取 body/type/extra；index 映射成元组后走 **同一** `fanout_from_index_rows`。缺 content → `StoreError`。
- 测试：
  - `insert_user_client_id_is_idempotent`：第二次改 `body`/`dest`，`duplicate=true`，`fanout.body` 仍是 `"hi"`，`fanout.dest=="bob"`，`recipients` 含 alice/bob。
  - `insert_group` 第二次换 members 列表，`recipients` 仍是第一次的三人。
  - 自聊 `recipients` 去重后长度为 1。
  - 现有 `empty_members_writes_content_without_index`：first `InsertResult.fanout` 为 `recipients=[]`、`kind=User`、`dest=""`（不是 Group / `req.dest`）。
  - `empty_members_duplicate_is_empty_recipients_not_error`：同一 `client_id` 再 insert → `duplicate=true`，fanout 同样是空 recipients / User / 空 dest，不是 `StoreError`。

**File: `services/chat/src/store/postgres.rs`**

- first：`fanout_from_write`，无额外 SELECT。
- early idempotency hit 与 ON CONFLICT 回读：`load_fanout(pool, app, message_id)`（签名、`FanoutSqlRow`、`ORDER BY i.id` 见 SQL 节）。
- `#[cfg(test)]`：有 `DATABASE_URL` 时 `postgres_duplicate_reloads_fanout`（换 body/dest，断言 fanout）；可选同一幽灵 duplicate 断言 `recipients=[]`。

**File: `services/chat/src/royal.rs`**

- `HttpMessageStore::insert_user` / `insert_group` 映射 `resp.fanout`；`None` → 空 `Fanout` + warn。

**File: `services/royal/src/lib.rs`**

- `insert_user` / `insert_group` 编码 `InsertMessageResp { fanout: Some(...) }`。
- `http_create_join_detail`（约 561 行）断言 first insert 的 `fanout.body` / `recipients`。
- **新增** `http_insert_user_duplicate_returns_original_fanout`：经 `http_backends` 同一 `client_id` insert 两次，第二次改 body/dest；断言 `duplicate`、`fanout.body` 是原文、`recipients` 是原来的 `[alice, bob]`。这是生产适配器（Chat HTTP → Royal）的钉；`e2e_talk` / talk 单测走 Memory，盖不住映射丢字段。

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

- `do_user_talk` / `do_group_talk` 增加 `metrics: Option<&KimMetrics>`。
- 单聊删 insert 前 `get_locations`。
- `persist_then_push`；永远 Success（insert Ok 之后）。
- `FailStore`：无新 trait 方法则不动。

**改写现有测试：**

| 测试 | 新契约 |
|---|---|
| `dispatch_fail_is_system_exception_without_success_resp`（约 813） | **改名** `dispatch_fail_is_success_with_message_resp`。`RecordingDispatcher::fail_on("wg-2")`。断言：一条 Flag=Push（尝试过）；一条 Success + 可解的 `MessageResp`；`message_id` 与 store 一致。带 `KimMetrics` 时 `kim_dispatch_fail_total{kind="user"}` = 1 |
| `same_client_id_does_not_insert_or_push_twice`（约 648） | **改名** `same_client_id_replays_push_from_store`。insert 仍 1；Push **2** 次；两次 Success 同一 id/time；两次 Push body 都是第一次 |
| `get_location_other_is_system_exception_without_insert_or_push`（约 844） | **改名** `get_location_other_is_success_after_insert_without_push`。`store.recorded().len()==1`；Success；无 Push |
| `group_get_locations_other_is_system_exception_after_insert`（约 1237） | **改名** `group_get_location_other_is_success_after_insert_without_push`。insert 1；Success；无 Push |

**新增测试：**

1. `same_client_id_changed_body_and_dest_pushes_first_persisted`：alice 与 bob、carol 都是好友。第一次 dest=bob、body=`hello`、`clientId=c1`。第二次 dest=carol、body=`CHANGED`、同一 clientId。bob 在线。断言：content 1 条且 body=`hello`、dest=bob；Push 的 `MessagePush.body=="hello"` 且 channel 只有 `ch-bob`；carol 的 channel 从未出现；两次 Success 同一 `message_id`。
2. `same_client_id_wrong_dest_not_friends_does_not_replay`：只与 bob 好友。第二次 dest=carol → `NotFriends`；Push 仍 1；store 1。
3. `group_duplicate_uses_index_snapshot_not_current_members`：`g1` 先 `[alice,bob]`，talk 一次（bob 在线）。`seed` 成 `[alice,carol]`（carol 在线、bob 仍在线）。同一 `clientId` 再 talk。第二次 Push 仍到 bob，**不到** carol。
4. `group_duplicate_changed_dest_replays_original_group`：alice 在 `g1` 与 `g2`。第一次 dest=`g1`。第二次 dest=`g2`、body 改掉。fanout/Push 仍是 `g1` 成员与原文。
5. `group_duplicate_changed_dest_not_member_does_not_replay`：alice 只在 `g1`。第一次 dest=`g1`（bob 在线，已 Push）。第二次 dest=`g2`（alice 不是成员）→ `NotGroupMember`；Push 仍 1（只有第一次）；store 1。与用户路径「非好友不重放」对称。
6. `empty_client_id_inserts_and_pushes_twice`：两次空 clientId，两条 content、两次 Push、两个 id。
7. `self_chat_success_skips_own_channel`：dest=alice；session channel `ch-alice`。再 `cache.add` alice 第二台（`ch-alice-web`，另一 `gate_id`）。Success；无 Push 到 `ch-alice`；**有** Push 到 `ch-alice-web`。不冻结 1:1 网关顺序。
8. `duplicate_dispatch_fail_still_success_and_metric`：第一次 dispatch fail（仍 Success）；第二次再 fail_on；两次 Success；指标 ≥1（duplicate 重放也 `inc`）。

群 `group_two_gates_coalesce_skip_sender_and_omit_offline` 等 happy path 应继续绿：first insert 的 `Fanout.recipients` 等于当时 members。

**File: `services/chat/tests/e2e_talk.rs`**

- 加一条：alice→bob 带 `clientId`；再发改 body 的同一 id；bob 第二帧 Push 仍是原文。用现有 `spawn_stack` / `become_friends`。这是 talk 层的进程级钉（`ChatHandler::new` → Memory），**不是** `HttpMessageStore` 覆盖；HTTP duplicate 见 Phase 1 Royal 测试。

---

## Phase 3 — 专题文档

代码 PR 的最后一组提交（仍同一 PR）。

**File: `docs/control-layer-chat.md`** — Status 表、duplicate 句、在线表。见 Design 文档节。

**File: `docs/reliable-delivery.md`** — persist-first 一段；ACK 标明 G-03。

**File: `docs/observability.md`** — `kim_dispatch_fail_total`。

**File: `docs/production-gaps.md`** — **删除 G-09**；顺序表第 8 行去掉；第 17 行去掉 G-09；G-15 正文改为「`kim_dispatch_fail_total` 已有；缺口是 send→ack / Royal RPC / 告警规则」；H0 persist-first 标已做。

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

人工扫：`talk.rs` 里 `MessagePush` 构造不得再出现 `req.body` / `req.r#type` / `req.extra`。`do_user_talk` 在 `insert_user` 之前不得 `get_locations`。`!inserted.duplicate` 不得再挡住 dispatch。

---

## Architectural Notes

- **at-least-once 是故意的。** duplicate 总是再 dispatch。第一次已经 Push 成功、客户端只是没收到 Success 而重试，对端会再收到同一 `messageId`。SDK 去重（Web 对 Push 与 offline content 已按 id 去重）。进程内「是否 dispatch 过」以及 `dispatch` 的 Err 都不能当权威：部分网关可能已经成功。
- **persist-first 之后，发送方看到 Success 不再保证对端在线收到。** 补偿两条：(1) 发送方没收到 Resp → 同一 `clientId` 重试 → 从库重放 Push；(2) 已收到 Success → 不再试，对端靠 offline pull。Flutter 没有 (2)，这是 G-14，不是本切片能修的。G-09 建议里的「连接建立时服务端 sync」不要做。
- **Young 连接 / 写队列满**（`adult_delay` 默认 10s，`write_queue=64`）今天会 99。之后发送方 Success，接收方可能仍没 Push。这是本切片要修的产品谎言。
- **G-03 仍然会丢。** 本切片多出来的重放 Push 若晚于一个更大的 `messageId` 被 ACK，offline pull 仍可能跳过。不在这里改游标。文档必须写明。
- **不改 `Context::dispatch`。** 部分成功仍返回第一个 Err；handler 不把它变成 99。G-30 再改网关下行 `try_send`。
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
| `crates/kim-protocol/proto/pkt.proto` | `InsertFanout` + `InsertMessageResp.fanout = 4` |
| `docs/control-layer-chat.md` | 99 不再含 dispatch fail；duplicate 会重放 Push |
| `docs/impl/README.md` | 切片 2 标已合入 |
| `docs/observability.md` | 列出 `kim_dispatch_fail_total` |
| `docs/production-gaps.md` | **删除 G-09**；顺序表第 8、17 行；G-15 正文；H0 留下 try_send |
| `docs/reliable-delivery.md` | 落库真相、在线尽力、duplicate 从库重建 |
| `sdk/web/src/proto/pkt.json` | gen-proto |
| `services/chat/src/lib.rs` | `ChatSvc.metrics` 唯一 mutex；删 `ChatHandler.metrics`；talk handler 传入 |
| `services/chat/src/royal.rs` | HTTP 映射 `Option<InsertFanout>`；`None` → 空 Fanout |
| `services/chat/src/store/mod.rs` | `Fanout` / `InsertResult`；共用 `fanout_from_index_rows`；幽灵 duplicate 单测 |
| `services/chat/src/store/postgres.rs` | duplicate JOIN load；可选 PG 单测 |
| `services/chat/src/talk.rs` | `persist_then_push`；改写/新增测试 |
| `services/chat/tests/e2e_talk.rs` | clientId 改 body 仍推原文 |
| `services/royal/src/lib.rs` | 编码 `fanout: Some(...)`；HTTP duplicate 改 body/dest 单测 |

不改：`crates/kim-router`（dispatch 语义）、`crates/kim-core`、`sdk/web/src/status.ts`、`sdk/mobile`、`crates/kim-client`、migrations、`docs/architecture.md`、`docs/protocol-container.md`。

---

## Key Decisions

1. 方案 A：enrich `InsertResult` + 嵌套 `InsertFanout fanout = 4`（prost `Option`）。拒绝 B、C，拒绝扁平字段 4–10。
2. `get_locations` 全部在 insert 之后，账号 = 持久化 `recipients` 去重。locations 非 NotFound 不得 99。
3. 群收件人 = 发送时 index 行，不是当前成员。后加入者不因这次重试收到旧消息；已退出者可能再收到 Push。
4. 空 `clientId` 不去重。
5. 自聊 recipients 去重；dispatch 仍跳过本 channel；测第二台设备。
6. exists/friend/block/filter/`members()` 仍看**本次请求**。duplicate 之后只信 store。dest 不一致：非好友 109 / 非群成员 107 不重放；否则重放到原来的 dest。必测。
7. `kim_dispatch_fail_total{service_id,service_name,kind}`：`dispatch` Err、locations 故障、空 `recipients` 都 +1，含 duplicate 重放。全员离线 NotFound 不加。
8. 无新表、无新 trait 方法、不改 `dispatch` 部分成功、不做 connect-time sync、不改 ACK、不改 `isRetryable`。Memory/Postgres first 与 duplicate 都走 `fanout_from_index_rows`；LEFT JOIN 用 `Option` + `ORDER BY i.id`。空 members 幽灵 first/duplicate 都是 `kind=User`、空 dest。
9. 一个实现 PR。升级先 Royal（或共享镜像）再 Chat；回滚先 Chat 再 Royal。混版本 = Success + 无 Push + 指标，不是空 body Push，也不是 99。

---

## PR Plan

切片 1 是单 PR。本切片同样：**一个实现 PR**。Phase 1 的类型加宽会迫使所有 `InsertResult` 构造一起改，拆成「只合 proto」的 PR 会留下 talk 仍 99、HTTP 已带 fanout 的半成品，review 负担并不更小。

### PR 1: persist-first：insert 成功即 Success，duplicate 从落库重建 Push

- **Description:** G-09 / H0（不含 G-30）。`insert_*` 返回 `Fanout`；`InsertMessageResp.fanout` 为嵌套 message；talk 在 persist 之后一律 `MessageResp`；dispatch / locations 失败 / 空 recipients 只打 `kim_dispatch_fail_total`；duplicate 忽略本次 body/dest，从 content+index 重放。改写把 99 锁死的单测。文档：落库真相、在线尽力。合入后删除 production-gaps G-09，并改 G-15 / 顺序表第 17 行。
- **Deploy / rollback:** 升级先 Royal 或一次发 `KIM_IMAGE`，再 Chat（含 `chat-gray`）。回滚先 Chat 再 Royal。禁止新 Chat 对着旧 Royal。混版本客户端看见 Success、零 Push；靠 `kim_dispatch_fail_total` 发现，不要回退到 `req.body`。
- **Files/components affected:** 见 File Change Summary。热路径：`talk.rs`、`store/{mod,postgres}.rs`、`royal.rs`、`services/royal` insert HTTP、`kim-metrics`、`pkt.proto`。
- **Dependencies:** 无。可在 G-01 之前合。不依赖 G-03（但文档必须写明游标仍会丢）。不依赖 G-14（文档写明 Flutter 仍不拉）。

若 review 强求拆分，唯一干净的切法是：(1) proto + `InsertResult` + Memory/Postgres/Http/Royal load（本文件 Phase 1）；(2) talk + metrics + 测试 + 文档（Phase 2–3）。(1) 必须独立可测 store 的「换 body 仍回原文」；(2) 依赖 (1)。不要第三 PR。
