# Kill Inbox N+1 Then Materialize Conversation Inbox

| 字段 | 值 |
|---|---|
| 状态 | Draft（审查修订：unread 方向、乱序增量、mark_read 事务、锁序、回填、开关在 Royal） |
| 日期 | 2026-09-02 |
| 覆盖 | G-17（inbox 全量聚合 + do_inbox_list N+1 → additive `conversation_inbox` 物化双写切读） |
| 父规格 | [next-stage.md](./next-stage.md) B7、[production-gaps.md](../production-gaps.md) G-17 |

## Breaking Change Notice

无客户端可见变更：`chat.inbox.list` 命令与 `InboxResp` 形状不变（仍无游标——分页属后续）。DB 加 additive 表 `conversation_inbox` + 同事务双写，旧 `message_index` 聚合读路径保留为 fallback 开关，回滚 = 读切回 GROUP BY。

## Feasibility Assessment

- N+1 现状核实：`do_inbox_list`（`inbox.rs:47-64`）每行 `users.profile` 或 `groups.detail`。`UserDirectory::profiles` 与 `HttpUserDirectory::profiles`（`royal.rs:679`，已代理 `/api/v1/user/profiles`）已存在。群无批量 detail。父规格要求「去掉每行 profile/**detail**」——Phase 1 必须补批量 group detail，否则 G-17 只能部分关。
- PG 聚合现状：`store/postgres.rs:821-905` 单条 GROUP BY（含 `conversation_reads` LEFT JOIN + unread FILTER）+ 2 条补充查询（content 批查、sender 批查）——3 次往返、无 N+1；痛点是**每次 inbox.list 全量 GROUP BY `message_index`**：索引 `message_index_inbox (app, account_a, direction, send_time)` 与「GROUP BY dest」不匹配，扫描面随账号全部历史消息增长。
- 物化路径：`insert_fanout`（legacy 与 pending 两版）已单事务写 content + N index 行——同事务 UPSERT `conversation_inbox` 是纯加法；`mark_read` 已有更新点。
- Memory store（`store/mod.rs:969+`）持读锁扫全量 `indexes: Vec<InboxRow>`——按 `(app, account_a)` 分组后单账号扫描面从 O(全库) 降为 O(该账号)。
- 生产 Chat 配了 `ROYAL_URL`，`inbox()` 实际跑在 Royal 内的 `PostgresMessageStore`（`chat/src/main.rs` royal 分支 → `http_backends_with_hmac`）。切读开关必须给 **royal / royal-2**，设在 Chat 上无效。
- **Feasible with caveats: 物化 SQL/事务/回填按下列决策改过之后才能上线。**

## Current Surface Inventory

- `services/chat/src/inbox.rs:28-72` — `do_inbox_list`：逐行 `users.profile` / `groups.detail`（N+1）
- `services/chat/src/store/postgres.rs:821-905` — `inbox()`：GROUP BY + content 批查 + sender 批查
- `services/chat/src/store/mod.rs:326-331` — `MessageStore::inbox` trait 签名（不变）
- `services/chat/src/store/mod.rs:969-1030` — Memory `inbox()`：全 `indexes` 扫描；`Inner.indexes: Vec<InboxRow>`
- `services/chat/src/store/mod.rs:1827` — `inbox_history_and_read_cursor` 测试（回归基准）
- `services/chat/src/store/postgres.rs:84-100` — `insert_fanout` 分派（legacy/pending）
- `services/chat/src/store/postgres.rs:102-200` — `insert_fanout_legacy`：单事务 content + index 行
- `services/chat/src/store/postgres.rs:503+` — `insert_fanout_pending` 同形
- `services/chat/migrations/0001_messages.sql` — `message_index_inbox` 索引（与 GROUP BY 不匹配）
- `services/chat/migrations/0006_user_social_inbox.sql` — `conversation_reads (app, account, peer, group_id)` 主键
- `services/chat/src/users.rs:60-80` — `UserDirectory` trait：`profiles` 批量已存在
- `services/royal/src/lib.rs:228` — `/api/v1/user/profiles` 批量端点已存在
- `services/chat/src/royal.rs:679` — `HttpUserDirectory::profiles` **已代理** `/api/v1/user/profiles`
- `services/chat/src/directory.rs:47` — `GroupDirectory::detail` 无批量
- `services/chat/src/store/mod.rs:21-22` — `DIRECTION_RECV = 0`，`DIRECTION_SEND = 1`
- `services/chat/src/store/postgres.rs:146-172` — 私聊 index 行顺序：先 sender/SEND，再 dest/RECV
- `services/chat/src/main.rs:232` — 生产 `ROYAL_URL` → Http 后端；PG store 在 Royal `open_pg_backends`

## Design

### 决策

1. **Phase A 消灭每行 profile 与 detail**：`do_inbox_list` 一次 `users.profiles`（已有）。群：补 `GroupDirectory::details(app, ids)` + Royal `POST /api/v1/group/details`（id 列表），Chat HTTP 适配器代理。逐群 `detail` 只作缺 id 回退。`Ok(None)` 仍用 dest 当 title。不补批量则不得从 gaps 删 G-17。
2. **物化表形状（additive）**：

   ```sql
   CREATE TABLE conversation_inbox (
       app             TEXT NOT NULL,
       account         TEXT NOT NULL,  -- 视角所有者 = message_index.account_a
       dest            TEXT NOT NULL,  -- peer account 或 group_id
       kind            SMALLINT NOT NULL CHECK (kind IN (0, 1)), -- 0 user / 1 group
       last_message_id BIGINT NOT NULL REFERENCES message_content (id),
       last_send_time  BIGINT NOT NULL,
       last_sender     TEXT NOT NULL,
       last_body       TEXT NOT NULL,  -- 冗余，避免 inbox 读 join content
       last_msg_type   SMALLINT NOT NULL,
       unread          INT NOT NULL DEFAULT 0,
       PRIMARY KEY (app, account, dest, kind)
   );
   CREATE INDEX conversation_inbox_recent
       ON conversation_inbox (app, account, last_send_time DESC);
   ```

   - `unread` 物化：Rust 里算 `unread_delta = if direction == DIRECTION_RECV { 1 } else { 0 }`（`RECV=0` / `SEND=1`，**禁止** `CASE WHEN $10 = 1`——那会给发送者 +1）。
   - `last_*` 冗余进表，读路径单表查询。
3. **双写同事务；unread 无条件累加；last 按 (send_time, message_id) 决胜**（审查修订）。

   ```sql
   INSERT INTO conversation_inbox
       (app, account, dest, kind, last_message_id, last_send_time,
        last_sender, last_body, last_msg_type, unread)
   VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)  -- $10 = unread_delta，Rust 算好
   ON CONFLICT (app, account, dest, kind) DO UPDATE SET
       last_message_id = CASE WHEN (EXCLUDED.last_send_time, EXCLUDED.last_message_id)
                                 > (conversation_inbox.last_send_time, conversation_inbox.last_message_id)
                            THEN EXCLUDED.last_message_id ELSE conversation_inbox.last_message_id END,
       last_send_time  = CASE WHEN (EXCLUDED.last_send_time, EXCLUDED.last_message_id)
                                 > (conversation_inbox.last_send_time, conversation_inbox.last_message_id)
                            THEN EXCLUDED.last_send_time ELSE conversation_inbox.last_send_time END,
       last_sender     = CASE WHEN (EXCLUDED.last_send_time, EXCLUDED.last_message_id)
                                 > (conversation_inbox.last_send_time, conversation_inbox.last_message_id)
                            THEN EXCLUDED.last_sender ELSE conversation_inbox.last_sender END,
       last_body       = CASE WHEN (EXCLUDED.last_send_time, EXCLUDED.last_message_id)
                                 > (conversation_inbox.last_send_time, conversation_inbox.last_message_id)
                            THEN EXCLUDED.last_body ELSE conversation_inbox.last_body END,
       last_msg_type   = CASE WHEN (EXCLUDED.last_send_time, EXCLUDED.last_message_id)
                                 > (conversation_inbox.last_send_time, conversation_inbox.last_message_id)
                            THEN EXCLUDED.last_msg_type ELSE conversation_inbox.last_msg_type END,
       unread = conversation_inbox.unread + EXCLUDED.unread;
   ```

   **没有 WHERE。** 旧稿 `WHERE EXCLUDED.last_send_time >= last_send_time` 会让较旧消息晚到时整行 UPDATE 跳过，unread 增量丢失。相同 send_time 用 `message_id` 决胜，不能任意覆盖。

   **锁序**：事务内把待 UPSERT 行按主键 `(app, account, dest, kind)` **排序后再写**。私聊 index 现序是 sender 再 dest（`postgres.rs:146-160`），Alice→Bob 与 Bob→Alice 会交叉锁 inbox 行死锁。群成员同样排序。已有 `lock_recv_accounts` 按账号排序可复用思路。测试：双向并发私聊不死锁。
4. **回填与在线写同一套终态 UPSERT；切读开关在 Royal**（审查修订）。
   - `0010` 只建表。回填 **不** `ON CONFLICT DO NOTHING`：在线写可能已插入该会话的新行，DO NOTHING 会永久丢掉历史 unread/last。
   - 按账号 `pg_advisory_xact_lock(hashtext(app), hashtext(account))`（与 pending `lock_recv_accounts` 同函数），锁内从 `message_index` + `conversation_reads` **重算完整终态**，再 UPSERT（unread 用绝对值 SET，last 仍按元组决胜；回填语句用 `unread = EXCLUDED.unread` 覆盖，因为那是全量重算）。
   - 切读前逐行 diff（物化 vs 旧 GROUP BY），不是「行数约等于」。
   - env `KIM_INBOX_MATERIALIZED`：`open_pg_backends` / `PostgresMessageStore` 构造时读取。compose 传给 **royal 与 royal-2**。standalone Chat（无 `ROYAL_URL`）同名开关。`kim.env.example` + compose 都写。默认 0。
5. **Memory store 按 `(app, account_a)` 分组**：先 grep `indexes` 的全部用法（offline.content / history 若同按账号过滤则一并受益），把 `Vec<InboxRow>` 改为 `HashMap<(String, String), Vec<InboxRow>>`（或加行索引 map 保留 Vec）。inbox() 读锁内只扫本账号 Vec。
6. **`mark_read` 与 inbox 同一事务，不清零并发新消息**（审查修订）。当前接口带具体 `message_id`，`conversation_reads` 只 `GREATEST` 前移（`postgres.rs:988-993`）。禁止「UPDATE reads 后再 `SET unread = 0`」：较新消息先插入、较旧 read 后到会把新消息标成已读；两句不在一事务还会分叉。改为：
   - 同一事务，按主键锁 inbox 行（`FOR UPDATE`，锁序仍按 PK 排序）
   - reads `GREATEST` 前移，得到有效 `last_read_id`
   - `unread` **重算**：该会话 `message_index` 上 `direction = RECV` 且 `message_id > last_read_id` 的行数（或按区间精确扣减，重算更简单）
   - 测试：insert/read 可控交错（新消息先写、旧 read 后到，unread 仍 ≥ 1）
7. **不做**：inbox 游标分页、`conversation_reads` 表退役（G-27 已读回执仍依赖）。

### 用法示例（读路径，物化开关开）

```rust
// services/chat/src/store/postgres.rs
if self.inbox_materialized {
    let rows: Vec<(String, i16, i64, i64, String, String, i32, i16)> = sqlx::query_as(
        "SELECT dest, kind, last_message_id, last_send_time, last_sender,
                last_body, unread, last_msg_type
           FROM conversation_inbox
          WHERE app = $1 AND account = $2
          ORDER BY last_send_time DESC, last_message_id DESC
          LIMIT $3")
        .bind(app).bind(account).bind(cap)
        .fetch_all(&self.pool).await.map_err(pg_err)?;
    return Ok(rows.into_iter().map(InboxEntry::from_row).collect());
}
// 旧 GROUP BY 路径（观察期保留，else 分支原体）
```

## Phased Implementation

### Phase 1: 消灭 do_inbox_list N+1（独立可合）

- **File: `services/chat/src/inbox.rs`** — User dest → 一次 `users.profiles`；Group dest → 一次 `groups.details`；缺档回退 dest。
- **File: `services/chat/src/directory.rs`** + **`directory/postgres.rs`** + **`royal.rs`** — `details(app, ids)`；Royal `POST /api/v1/group/details`（`HttpGroupDirectory` 代理）。`HttpUserDirectory::profiles` 已有，不改。
- **File: `services/royal/src/lib.rs`** — 批量 group detail 端点。
- 测试：Fake 目录断言 `profiles`/`details` 各一次。
- 验证：`env -u REDIS_URL cargo test -p chat && cargo clippy -p chat -- -D warnings`。

### Phase 2: 物化表 + 同事务双写（读不切）

- **File: `services/chat/migrations/0010_conversation_inbox.sql`（新）** — 决策 2 DDL。
- **File: `services/chat/src/store/postgres.rs`**
  - `insert_fanout_legacy` / `insert_fanout_pending`：index 行写完后，按 PK 排序再 UPSERT（决策 3）。Rust 算 `unread_delta`。
  - `mark_read`：与 inbox 同一事务（决策 6），重算 unread，禁止 `SET unread = 0`。
  - `inbox()`：`inbox_materialized` 分支，默认 false。
- **File: `services/chat/src/store/mod.rs`** — 构造链读 `KIM_INBOX_MATERIALIZED`；`open_pg_backends` 透传（Royal 进程生效）。
- **File: tests** — RECV 才 +unread；乱序旧消息仍 +unread 且不回退 last；同 send_time 用更大 message_id；双向并发 insert 不死锁；insert/read 交错 unread 正确。
- 验证：`cargo test -p chat`；clippy。

### Phase 3: 回填脚本 + 读切换

- **File: `deploy/backfill-inbox.sql`（新）** — 按账号 advisory lock；锁内重算终态；UPSERT 写绝对值 unread + last 元组决胜。可重跑。禁止 `DO NOTHING`。
- **File: `deploy/backfill-inbox.sh`** — `psql` 包装。
- **File: `deploy/kim.env.example` + `deploy/compose.yml`** — `KIM_INBOX_MATERIALIZED=0` 传给 **royal、royal-2**（及无 ROYAL_URL 的 standalone chat）。
- **File: `docs/deploy.md`** — 回填 → **逐行 diff**（不是行数约等于）→ royal 置 1 → 观察 → 回滚=置 0。
- 验证：compose 回填 + 切读；Chat 进程置 1、Royal 置 0 时读路径仍走旧 GROUP BY（证明开关在写库进程）。

### Phase 4: Memory store 按账号分组

- **File: `services/chat/src/store/mod.rs`**
  - 先 grep `\.indexes` 全部用法定形：offline/content/history 若按 `(app, account)` 过滤则 `HashMap<(String,String>, Vec<InboxRow>>` 全体受益；若有全表迭代（如 GC/统计）则保留 Vec + 加 map 索引。
  - `insert_*` 写点同步维护分组；`inbox()` 读锁内只扫本账号 Vec。
- **File: `services/chat/src/store/mod.rs`（tests）** — `inbox_history_and_read_cursor` 回归 + 账号隔离断言（B 的 insert 不影响 A 的 inbox 结果）。
- 验证：`env -u REDIS_URL cargo test -p chat`。

### Phase 5: 文档

- **File: `docs/user-social-inbox.md`** — 物化表形状、双写点、unread 语义、乱序守卫、回填与切换 runbook。
- **File: `docs/control-layer-chat.md`** — inbox 读路径两模式。
- **File: `docs/production-gaps.md`** — G-17 关闭（GROUP BY 退役时点 = 观察期后另 PR）。
- **File: `docs/impl/README.md`** — B7 记录。
- 验证：全量 `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && env -u REDIS_URL cargo test --workspace`。

## Architectural Notes

- **同事务双写代价**：群 500 成员 ≈ 500 index + 500 inbox UPSERT。G-25 正交。
- **死锁**：inbox UPSERT 与 mark_read 都按 `(app, account, dest, kind)` 排序后取锁。私聊交叉发送必须先排序。reads 与 inbox 在同一事务内，先锁 inbox 再更新 reads，或先锁 reads 再 inbox——**全仓库统一一种顺序**（推荐 PK 序锁 inbox，再 GREATEST reads）。
- **unread 重算 vs 置零**：重算不怕乱序 read；代价是 mark_read 多一次 index 计数，inbox 读频远高于 mark_read，可接受。
- **`last_body` 冗余的下游义务**：撤回（G-27）改写 content.body 时须同步 UPDATE inbox 行——记入 G-27 依赖清单。
- **明确不改**：`chat.inbox.list` 协议形状、`MessageStore::inbox` trait 签名、客户端、history/offline 语义（Memory 分组只改扫描面）、`conversation_reads` 表（保留双写）。
- **回滚**：`KIM_INBOX_MATERIALIZED=0` 读切回旧路径；物化表留存无害（双写继续，回到开关即回到读）。
- **新依赖**：无。

## File Change Summary

- `services/chat/src/inbox.rs` -- 批量 profiles + details
- `services/chat/src/directory.rs` / `directory/postgres.rs` -- `GroupDirectory::details`
- `services/chat/src/royal.rs` -- HttpGroupDirectory::details（profiles 已有）
- `services/royal/src/lib.rs` -- `POST /api/v1/group/details`
- `services/chat/migrations/0010_conversation_inbox.sql` -- 物化表 DDL
- `services/chat/src/store/postgres.rs` -- 排序后 UPSERT、unread_delta、mark_read 同事务重算、物化读
- `services/chat/src/store/mod.rs` -- 开关（Royal 构造）+ Memory 按账号分组
- `deploy/backfill-inbox.sql` -- advisory lock + 终态 UPSERT
- `deploy/backfill-inbox.sh` -- 回填入口
- `deploy/kim.env.example` / `deploy/compose.yml` -- `KIM_INBOX_MATERIALIZED` 给 royal / royal-2
- `docs/deploy.md` -- 回填/切换 runbook
- `docs/user-social-inbox.md` -- 物化形状
- `docs/control-layer-chat.md` -- 读路径两模式
- `docs/production-gaps.md` -- G-17 关闭
- `docs/impl/README.md` -- B7 记录
