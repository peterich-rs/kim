# 服务端会话隐藏（Delete conversation for me）

| 字段 | 值 |
|---|---|
| 状态 | Draft |
| 作者 | — |
| 日期 | 2026-09-18 |
| 对照代码 | 工作树 HEAD `8c6d2d2`（用户分析基于 `dead0fc`；下文行号均已对当前树核验，不以旧分析为准） |
| 父规格 | [user-social-inbox.md](../user-social-inbox.md)、[control-layer-chat.md](../control-layer-chat.md)、[production-gaps.md](../production-gaps.md) |
| 范围 | v1：服务端 **按账号隐藏会话** + 客户端滑删不再被 `chat.inbox.list` 复活。不含清空聊天、撤回、人类双向物理 purge、账号注销。 |

---

## Overview

kim 首页滑删今天只做本地 SQLite 物理删除：`KimSdk::delete_thread` 在同一事务里清掉 `outbox` / `messages` / `threads` / `read_watermarks` / `timeline_meta`，**零服务端调用**。重连 `SyncEngine::run` 先 `chat.inbox.list` 再 `persist_inbox`，把同一 dest upsert 回 `threads`，会话格复活。对方副本、`message_content`、`message_index` 从未被触及。

生产 IM 把「删除」拆成一族正交操作。v1 只做微信/WhatsApp/Telegram 的 **滑删 = 对我隐藏 inbox 项**：服务端记下 per-account hide watermark，`inbox.list` 两端读路径（GROUP BY 与 `conversation_inbox` 物化）都过滤；新消息（物化行的 `last_message_id` 越过 `hidden_until_id`）自动重新出现；对方不受影响；历史仍可从通讯录点开。不把 `friend.remove` 接到消息存储上，不引入 `dm_purge_marks`，不把 bot 已有的 `purge_peer_dm` 复用到人类删好友。

---

## Background & Motivation

### 当前行为（已核验）

滑删链路：

1. Flutter `ConversationTile`（`sdk/mobile/lib/design/conversation_tile.dart:46-58`）`SlidableAction` → `chats_page.dart:229-231` `threadsProvider.deleteThread` → `inbox.dart:99-100` → FRB `KimUiHandle::delete_thread`。
2. `KimSdk::delete_thread`（`crates/kim-sdk/src/lib.rs:421-438`）只打本地 store：`cancel_outbox_run`、`store.delete_thread`、清 timeline 订阅窗口、`publish_timeline_resync(..., "deleted")`。**没有** `self.protocol()` 调用。对比同文件 `mark_read`（393-418 行）会 `tokio::spawn` `proto.mark_read`。它 **不** 从 `timelines` HashMap 摘掉订阅（只 reset 窗口）。
3. `outbox::delete_thread`（`crates/kim-sdk/src/store/outbox.rs:324-359`）单事务：

```text
DELETE outbox / messages / threads / read_watermarks / timeline_meta
  WHERE account = ? AND dest = ?
```

4. 重连复活：`crates/kim-client/src/sync.rs:160-168` `inbox_list(INBOX_LIMIT=200)` → `PersistHook::persist_inbox` → `persist_inbox_tx`（`store/mod.rs:1925-1949`）对每条 `InboxItem` 调 `threads::persist_inbox_item`（`threads.rs:107-154`）**UPSERT**。`persist_inbox` **不会**删除服务端列表里没有的本地 dest，但会把刚删掉的 dest 写回来——这就是复活。

补充（相对旧分析的修正）：复活的不只是「空会话格」。`persist_inbox` 只恢复 `threads` 行；随后同一轮 `offline_index` / `persist_talks`（`sync.rs:173-197`，`persist_talks_tx` → `apply_talk` + `threads::apply_incoming`，`store/mod.rs:1877-1918`）以及点开会话后的 `hydrate_latest_if_needed` → `chat.history` 会把消息也拉回来。Live persist 是 persist-then-ack（`supervisor.rs:143-164`：`persist_talks` Ok ⇒ `client.ack(id)`）。v1 堵住的是 **列表**（`persist_inbox` / `apply_incoming`）在 hide 未确认前把 dest 写回 `threads`。**不得** skip `apply_talk` 却仍 ACK——那会改 G-03 的 ACK 语义并吞掉「新消息 unhide」的那条 id。开着的 transcript 靠 **unsubscribe** 不刷新 UI，不靠丢弃落盘。

`inbox.list` 不是全量快照。服务端 `INBOX_MAX=100`（`services/chat/src/store/mod.rs:25-26`），`clamp_page` 把客户端 200 截成 100。因此 **不能**把「不在本次 list 里」当成隐藏——本地 SQLite 本来就允许比云端 top-100 更长的列表。

Web 同样有持久 dest 缓存，不是「每次信 `inbox()`」：`sdk/web/app/lib/threads.ts` 键 `kim.web.threads.${account}`；`ChatProvider.tsx:415-424` hydrate/save，`608-624` 对 `session.inbox()` **只 upsert、不删缺席 dest**。`decodeInboxResp`（`sdk/web/src/proto.ts:593`）只读 `items`。

### 社交与存储是两条线

| 操作 | 代码 | 对消息存储的影响 |
|---|---|---|
| `chat.friend.remove` | `do_friend_remove`（`friends.rs:233-269`）→ `PostgresSocialDirectory::remove`（`social.rs:434-443`）`DELETE FROM friendships` | **无**。不碰 `message_index` / `message_content` / `conversation_inbox` / `conversation_reads` |
| `chat.user.talk` | `talk.rs:104-129` | 非好友 `NotFriends=109`；拉黑 `Blocked=110` |
| `chat.history` / `chat.inbox.list` | `inbox.rs:26-117, 119-169` | **无好友检查** |
| `chat.bot.delete` | `bot.rs:109-146` | 成功后 `store.purge_peer_dm`：双向物理删除 index/content/inbox/reads/pending/idempotency/bot_turns |
| Flutter 删好友 | `peer_profile_page.dart:150-154` | 客户端组合：`friendRemove` **然后**本地 `deleteThread`。服务端仍无 hide。PR2 之后该组合会变成 unfriend → 服务端 hide；只调 `chat.friend.remove` 的客户端不会 hide。这是产品分叉，不是存储耦合（见 Key Decision 9）。 |

`docs/user-social-inbox.md:40` 已写明：人类 `friend.remove` 不清服务端历史；「双向人类 purge 另议」。这与微信（删好友保留本地/云历史、再加好友可续）一致，**不是缺口**。缺口是：没有服务端 hide API，本地滑删在 sync 之后是谎言。

### 双索引与 inbox 双读路径

```text
message_content (id PK, shared body)
       ▲
       │ message_id FK
message_index (per-account 行: account_a 视角, account_b/group_id, direction)
       │
       ├── 写路径始终 UPSERT conversation_inbox   (postgres.rs:258-268, 431-441)
       └── 读路径 KIM_INBOX_MATERIALIZED
             ├─ 0 (生产默认): GROUP BY message_index LEFT JOIN conversation_reads
             └─ 1: SELECT FROM conversation_inbox ORDER BY last_send_time
```

`conversation_inbox` PK `(app, account, dest, kind)`（`migrations/0010_conversation_inbox.sql`）。`last_message_id BIGINT NOT NULL REFERENCES message_content (id)`（`0010:7`），hide 补行必须用真实 content id，禁止 `last_message_id=0`。hide 是 **该账号的会话视图** 状态，自然落在这张表上，不必新建 mark 表。G-17 尚未在生产切物化读（`production-gaps.md`），v1 的过滤必须两条读路径都生效。

写 inbox 与 `mark_read` 已持账号级 `pg_advisory_xact_lock(hashtext(app), hashtext(account))`（`lock_inbox_accounts`，`postgres.rs:699-715`；`mark_read` 1220 行）。`insert_fanout_*` 锁 sender+dest（`postgres.rs:193-197, 332-335`）。hide 必须进同一把锁，避免与 in-flight talk 的 last 元组竞态。

last 的权威序是 `(send_time, message_id)`，不是单独 `MAX(message_id)`：`upsert_inbox_rows`（`postgres.rs:601-622`）、Memory inbox（`store/mod.rs:1111-1113`）、G-17 / `deploy/backfill-inbox.sql` oracle 都是 `DISTINCT ON ... ORDER BY send_time DESC, message_id DESC`。GROUP BY list（`postgres.rs:1060-1079`）今天仍用独立 `MAX(i.message_id)` / `MAX(i.send_time)`——这是 G-17 要退役的 dual-MAX。hide 可见性 **不得**再拿 dual-MAX 去和 watermark 比。

### 生产 IM 怎么做（产品对照）

| 操作 | 微信 | WhatsApp | Telegram | Signal | iMessage | kim v1 |
|---|---|---|---|---|---|---|
| 滑删 / 删除会话 | 对我隐藏；新消息回来 | 对我隐藏；新消息回来 | 删对话（可清本地）；归档是另一套 | 删对话 for me | 本地删，iCloud 行为因版本而异 | **做：hide** |
| 清空聊天记录 | 对我清记录，会话格可留 | Clear chat | Clear history | 清本地 | 清本地 | 后期 |
| 撤回 | 2 分钟双方 tombstone | 对所有人删（窗口随版本） | 双方删 | 双方删 | 两分钟 undo | 后期 |
| 删好友 | **不清历史** | 不清 | 不清 | 不清 | N/A | **保持现状** |
| 互删物理抹掉 | 无 | 无 | 无 | 无 | 无 | **非目标** |
| 账号注销 / GDPR | 物理删该用户数据 | 同 | 同 | 同 | 同 | 后期，复用 `purge_peer_dm` |
| Bot 删除 | N/A | N/A | bot 退订 | N/A | N/A | **已有 purge，不动** |

---

## Goals & Non-Goals

### v1 Goals

1. 滑删后，**本设备重连**与**同账号其它设备**在约定的 unhide 事件之前，不得再从 `chat.inbox.list` / `persist_inbox` / `persist_talks` 的 **thread upsert** 把该 dest 填回会话列表。`inbox_hides.pending=1` 期间 **禁止** 因 `last_message_id` 更大而 unhide 列表。消息体仍落盘（见 Key Decision 7）。
2. 隐藏是 per-account：对方 inbox、对方 `message_index`、共享 `message_content` 不变。
3. Unhide 事件（须 `pending=0` 且本地 watermark 已换成服务端返回值之后）：该会话新消息（`conversation_inbox.last_message_id > hidden_until_id`），或用户从通讯录主动打开（显式 `chat.inbox.unhide`）。
4. `chat.history` 在隐藏期间仍可读（点开续聊）。**不**在 history 上隐式 unhide（避免 `hydrate_watched` 误伤）。
5. 群会话 hide ≠ `chat.group.quit`。拉黑 / 删好友 / 删 bot 保持正交。服务端 `do_friend_remove` 不加 hide。
6. 两条 inbox 读路径、Royal HTTP（含 `InboxResp.hidden` 与 hide 响应 `hidden_until_id`）、web/mobile 协议面对齐。旧客户端不调用 hide → 行为与今天相同（可接受）。
7. 滑删时若该 dest 的聊天页仍打开：unsubscribe timeline，Flutter **必须 pop**（或宽屏清空选中）。UI 不再收到 timeline 推送。SQLite `messages` 仍写入（unsubscribe ⇒ 无 UI）；**不**靠 skip+ACK 来空 transcript。

### Non-Goals（v1 明确不做）

- 清空聊天（删我的 `message_index`、GC `message_content`）。
- 撤回 / delete-for-everyone。
- 人类互删好友触发物理 purge；不新增 `dm_purge_marks`。
- 非好友时对 history/inbox 做可见性过滤（默认保持现状：仍可读）。可作为独立 flagged 产品，见后文。
- 账号注销 / GDPR 擦除（后期复用 `purge_peer_dm`）。
- 改 ACK / `pending_delivery` / G-03。
- 把 hide 塞进消息 `outbox` 表（列语义是 talk payload，不混）。
- 改变 `chat.bot.delete` → `purge_peer_dm`。
- 在 `do_friend_remove` 里「顺便」hide。

---

## Proposed Design

### 操作族与 v1 切片

```text
                    社交图                          消息存储
               ┌─────────────┐                 ┌──────────────────┐
 friend.remove │ friendships │                 │ message_index    │
 block.*       │ blocks      │                 │  (per-account)   │
               └─────────────┘                 │ message_content  │
                                               │  (shared body)   │
                                               │ conversation_*   │
                                               └──────────────────┘
  v1 ★ chat.inbox.hide / unhide ──► conversation_inbox.hidden_until_id
  later  clear-for-me            ──► drop MY index + GC content
  later  recall                  ──► tombstone content (both sides)
  later  account delete          ──► purge_peer_dm per peer
  now    chat.bot.delete         ──► purge_peer_dm (unchanged)
```

### 最小 schema：`hidden_until_id`，不加 mark 表

```sql
-- services/chat/migrations/0016_inbox_hide.sql
ALTER TABLE conversation_inbox
  ADD COLUMN hidden_until_id BIGINT NOT NULL DEFAULT 0;
```

语义：

- `hidden_until_id = 0`（默认）：未隐藏。雪花 `message_id > 0`，故物化行 `last_message_id > 0` 恒成立。
- hide：在账号 advisory lock 下把 `hidden_until_id` 设为 **当前物化行** 的 `last_message_id`（即 `(send_time, message_id)` 元组选出的那条，不是 `MAX(message_id)`）。
- **权威可见谓词**（物化行存在时）：`conversation_inbox.last_message_id > conversation_inbox.hidden_until_id`。
- 新消息 UPSERT 只推进 `last_message_id`（现有 `upsert_inbox_rows`，`postgres.rs:601-622`），**不必**把 `hidden_until_id` 清零；比较即自动 unhide。
- 再次滑删：watermark 跟到新的 last。

为什么不是：

| 方案 | 拒绝原因 |
|---|---|
| 独立 `dm_purge_marks` | 多一张表、多一次 join；把 hide 和 purge 绑在一起；探索稿已否决 |
| `hidden_at timestamptz` | unhide 要和 `send_time` 比，时钟/纳秒与雪花 id 两套序 |
| hide 时 DELETE `conversation_inbox` 行 | `KIM_INBOX_MATERIALIZED=0` 的 GROUP BY 仍从 `message_index` 聚出该项 |
| hide 时 DELETE 我的 `message_index` | 滑删变成清记录；新消息 unhide 无法保留历史 |
| 把标志放 `conversation_reads` | 从未 `inbox.read` 过的会话没有行；reads 语义是已读游标 |

`conversation_inbox` 在 **每次** `insert_user` / `insert_group` 已双写，与物化开关无关。G-17 回填未完成时，hide 对尚无物化行的 dest 做 UPSERT：last_* 用与 backfill 相同的 `DISTINCT ON` 序从 `message_index` 取；没有任何消息则 Success 空操作（不 insert 行，因为 FK 禁 `last_message_id=0`），但仍 Push（见写路径）。

不改 `deploy/backfill-inbox.sql` 的 SET 列表以外：回填 `ON CONFLICT DO UPDATE` 不得覆盖 `hidden_until_id`（该脚本目前只 SET last_* / unread）。新回填运行必须 `hidden_until_id = conversation_inbox.hidden_until_id`（保留已有 hide）。

### 可见性谓词（两条读路径）

物化（`inbox_materialized`，`postgres.rs:641-654`）在 WHERE 增加，LIMIT 之前过滤，隐藏行不占 100 cap：

```sql
SELECT dest, kind, last_message_id, last_send_time, last_sender,
       last_body, unread, last_msg_type
  FROM conversation_inbox
 WHERE app = $1 AND account = $2
   AND last_message_id > hidden_until_id   -- 隐藏行: last_id <= hidden_until_id
 ORDER BY last_send_time DESC, last_message_id DESC
 LIMIT $3
```

GROUP BY **禁止**在每条 `message_index` 上 LEFT JOIN `conversation_inbox`（`CASE dest` 不是 PK 查找，且 `HAVING MAX(i.message_id) > hidden` 与写路径元组序不一致，dual-MAX 会漏藏或与 Memory 测分叉）。正确做法：先走今天的聚合，**再**按 dest 点查物化行：

```sql
WITH agg AS (
  SELECT
      CASE WHEN i.group_id = '' THEN i.account_b ELSE i.group_id END AS dest,
      CASE WHEN i.group_id = '' THEN 0 ELSE 1 END AS kind,
      MAX(i.message_id) AS last_id,      -- 仅用于 ORDER/preview 回填；不用于 hide 比较
      MAX(i.send_time) AS last_at,
      COUNT(*) FILTER (
          WHERE i.direction = $3 AND i.message_id > COALESCE(r.last_read_id, 0)
      )::int AS unread
    FROM message_index i
    LEFT JOIN conversation_reads r ON ...   -- 与 postgres.rs:1070-1075 相同
   WHERE i.app = $1 AND i.account_a = $2
   GROUP BY 1, 2, r.last_read_id
)
SELECT a.*
  FROM agg a
  LEFT JOIN conversation_inbox ci
    ON ci.app = $1 AND ci.account = $2
   AND ci.dest = a.dest AND ci.kind = a.kind
 WHERE ci.account IS NULL
    OR ci.last_message_id > ci.hidden_until_id
 ORDER BY a.last_at DESC, a.last_id DESC
 LIMIT $4
```

规则：

- 有 `conversation_inbox` 行：用 **`ci.last_message_id > ci.hidden_until_id`**，不用 `MAX(i.message_id)`。
- 无行（从未 hide、回填未写到）：可见（`ci IS NULL`）。
- `WHERE` 在 `LIMIT` 之前，隐藏 dest 不吃掉 100 cap。禁止「先 LIMIT 再滤」。
- preview 的 last body 仍按今天方式用 `agg.last_id` 去查 `message_content`（dual-MAX preview 是 G-17 的既有债，本切片不修 oracle）。

`MemoryMessageStore::inbox`（`store/mod.rs:1061+`）对每 dest 若有 hide watermark，用 **自己维护的 last（send_time, message_id）** 与 watermark 比，与 postgres 物化谓词同构，不要用 dual-MAX。

`chat.history` **不加** hide 过滤。隐藏不是清记录。

`offline_index` **不加** hide 过滤。离线投递仍要到达并 **落盘 messages**；SDK 在 `pending=1` 或 `message_id <= hidden_until_id` 时不把 dest 写回 `threads`。ACK 仍是 persist-then-ack：写入 messages 成功才 ACK。漏投才是 G-03 的问题，hide 不掺进去。

### hide / unhide 写路径

与 `mark_read` 同锁：`lock_inbox_accounts` + `conversation_inbox ... FOR UPDATE`。

```text
hide(app, account, dest, kind):
  ① lock account
  ② SELECT last_message_id FROM conversation_inbox FOR UPDATE
     hit  → hidden_until_id := last_message_id; UPDATE; goto ④
     miss → DISTINCT ON 取 index 最新一行（序同 backfill）
            仍无 → hidden_until_id := 0; 不 INSERT; goto ④
            有   → UPSERT conversation_inbox
                   last_* 来自该行（id 必须存在于 message_content）
                   unread := COUNT index RECV AND message_id > COALESCE(reads,0)
                             （与 mark_read 相同重算，禁止「保持」）
                   hidden_until_id := last_message_id
  ③ commit
  ④ 始终 notify_account(self, CMD_INBOX_HIDE, {kind, hidden_until_id})
     含「无行空操作」：其它设备可能有 ensureThread 空格，要靠 Push prune
  ⑤ 返回 hidden_until_id（Response body / HTTP）
```

「最新一行」必须与 backfill 相同，禁止 `MAX(message_id)`：

```sql
SELECT DISTINCT ON (1)
       CASE WHEN group_id = '' THEN account_b ELSE group_id END,
       message_id, send_time, direction, account_b, group_id
  FROM message_index
 WHERE app = $1 AND account_a = $2 AND group_id = $3
   AND ($3 <> '' OR account_b = $4)
 ORDER BY 1, send_time DESC, message_id DESC
 LIMIT 1
```

私聊 `$3=''` `$4=dest`；群 `$3=dest`。再 JOIN `message_content` 取 body/type。

幂等：重复 hide 把 watermark 设成当前 last；已隐藏且无新消息 → 写相同值，Success。

unhide：`SET hidden_until_id = 0`。无行 → Success。Response 空 body。然后 Push `chat.inbox.unhide` 给其它 location。

竞态（hide vs in-flight talk）：双方都锁该账号。串行后两种结果都合理：

- insert 先：hide 看到新 last，连刚到达的消息一起藏（用户正在滑删）。
- hide 先：insert 推进 `last_message_id`，谓词自动 unhide（微信「新消息回来」）。

不在 `upsert_inbox_rows` 里显式 `hidden_until_id = 0`。

### 现状 vs 目标：滑删

```text
Flutter              kim-sdk                 Chat                  Postgres
   |                    |                      |                      |
   | deleteThread(dest) |                      |                      |
   |───────────────────►|                      |                      |
   |                    | DELETE sqlite        |                      |
   |                    | ✗ no RPC             |                      |
   |                    |                      |                      |
   |                    | reconnect            |                      |
   |                    |──► chat.inbox.list ─►|──► inbox() ─────────►|
   |                    |◄── items incl dest ──|◄─────────────────────|
   |                    | persist_inbox UPSERT dest   ← 复活
Legend: ──► request   ◄── response   ✗ missing call
```

```text
Flutter              kim-sdk                 Chat                  Postgres
   |                    |                      |                      |
   | deleteThread ★     |                      |                      |
   |───────────────────►| one tx: DELETE +     |                      |
   |                    |  INSERT inbox_hides  |                      |
   |                    |  pending=1 ★         |                      |
   |                    | unsub timeline ★     |                      |
   |                    |──► chat.inbox.hide ★►|──► hide() ★ ────────►|
   |                    |◄── {hidden_until_id} |◄─────────────────────|
   |                    | pending=0; local WM  |══► chat.inbox.hide ★  other locs
   |                    |  := server id ★      |    Header.dest = conv
   |                    |                      |                      |
   |                    | reconnect            |                      |
   |                    | flush pending ★      |                      |
   |                    |──► chat.inbox.list ─►|                      |
   |                    |◄── items omit dest ──|  hidden iff last_id <= WM
   |                    |    + hidden[] ★      |                      |
   |                    | persist skip/prune ★ |                      |
Legend: ──► request   ◄── response   ══► push   ★ new
        omit dest because the visible predicate last_id > hidden_until_id is false
```

### InboxResp 附带当前隐藏集

`persist_inbox` 不能靠「不在 items 里」删本地行（top-100 窗口）。proto3 向 `InboxResp` 加字段：

```protobuf
message InboxHiddenItem {
  string dest = 1;
  int32 kind = 2;
  int64 hiddenUntilId = 3;
}

message InboxResp {
  repeated InboxItem items = 1;
  repeated InboxHiddenItem hidden = 2; // * 该账号当前仍隐藏的 dest
}
```

`inbox_hidden` SQL（postgres 与 Memory 同构）：

```sql
SELECT dest, kind, hidden_until_id
  FROM conversation_inbox
 WHERE app = $1 AND account = $2
   AND last_message_id <= hidden_until_id
 ORDER BY last_send_time DESC, last_message_id DESC
 LIMIT 500
```

必须有 `ORDER BY`：截断时丢掉的是最旧隐藏 dest，可预测。命中 500 时：`kim_inbox_hidden_set_truncated_total` +1，warn log `account` + count。v1 不翻页、不加 `truncated` 字段（Q2）。旧客户端忽略 field 2。

`do_inbox_list` 今日只建 `InboxResp { items }`（`inbox.rs:114`）。改为填 `hidden`。生产路径见 Royal：`POST /api/v1/inbox` **同一** `InboxResp` 带回 `hidden`，Chat HTTP 适配器不得丢 field 2。

---

## API / Interface Changes

### 长连接命令

沿用 `chat.inbox.*` 与 `ConversationReadReq` 形状（dest 在 Header，kind 在 body）。

| command | Flag | dest | body | 谁可调 |
|---|---|---|---|---|
| `chat.inbox.hide` | Request | peer 账号或 group id | `ConversationHideReq { kind }` | 已登录 session |
| `chat.inbox.hide` | Push | **被隐藏的会话 dest**（`dispatch_cmd` 复制 request header，`context.rs:104-106`） | `ConversationHideReq { kind, hidden_until_id }` | Chat → 同账号其它 location（skip 本 channel） |
| `chat.inbox.unhide` | Request | 同上 | `ConversationHideReq { kind }` | 已登录 session |
| `chat.inbox.unhide` | Push | 同上（会话 dest） | `ConversationHideReq { kind }` | Chat → 同账号其它 location |

与 `chat.friend.request` 相同：**Push 复用 command**，不另开 `chat.inbox.hidden`。`Command` 枚举今日 44 条（`command.rs:154` `CONSTANTS` 与 `Command::ALL` 等长），+2 → 46。`crates/kim-protocol/src/lib.rs` 的 `CMD_*` re-export 同步加。`kim-metrics` `COMMANDS` 今日 **34** 条（3 login + 31 `chat.*`，`lib.rs:17-52`）；文档 `observability.md` 写 34、`production-gaps.md` 写 29，以源码 34 为准。加入 hide/unhide，否则进 `other`。

```protobuf
message ConversationHideReq {
  int32 kind = 1;            // 0 user, 1 group
  int64 hidden_until_id = 2; // hide Success / Push 必填；Request 可 0，服务端填
}
```

**hide Success body** = `ConversationHideReq { kind, hidden_until_id }`（服务端写入值）。客户端 ack 后用该值覆盖本地 watermark。**unhide Success** 空 body。

不新增 Status。错误：

| 条件 | Status |
|---|---|
| `Header.dest` 空 | `NoDestination=300` |
| kind 不是 0/1 | `InvalidPacketBody=101` |
| 存储失败 | `SystemException=99` |
| 无会话 / 重复 hide / 重复 unhide | `Success=0`（幂等；hide 仍带 body，`hidden_until_id` 可能为 0） |
| 服务端尚未识别该 command（混部） | `CommandNotFound=2` |

**不**查好友、不查群成员、不查 bot owner。隐藏自己的 inbox 项与是否还能 talk 无关。非好友 hide 合法。群 hide 不是退群。

幂等：hide/unhide 都是状态收敛，不需要 client_id。

### Royal HTTP（生产 inbox 读/写都在 Royal）

现有：`POST /api/v1/inbox`、`/history`、`/inbox/read`（`royal/src/lib.rs:295-297`，`product.rs:307-367`）。Chat `HttpMessageStore::inbox`（`royal.rs:449-479`）今日只 map `InboxResp.items`，**丢弃其它字段**。`mark_read` 走 `post_maybe_empty`（`royal.rs:520-540`），hide **不能**走空 body，因为 Push 需要 `hidden_until_id`。

选定方案（不做独立 `/hidden` 二次 RPC）：

| 路径 | 方向 | body |
|---|---|---|
| `POST /api/v1/inbox` | 入 `InboxQuery { account, limit }` | 出 **同一** `InboxResp { items, hidden }` |
| `POST /api/v1/inbox/hide` | 入 `ConversationHide { account, dest, kind }` | 出 `ConversationHide { account, dest, kind, hidden_until_id }` |
| `POST /api/v1/inbox/unhide` | 入 `ConversationHide { account, dest, kind }` | 空 |

`royal_pool.rs:337-346` `classify` 前缀 `/api/v1/inbox` 已覆盖 `/hide` `/unhide` `/read`，path_group=`inbox`，不必加条目。内部 HMAC 与其它 product 路由相同。

Chat `do_inbox_list` 生产路径：一次 `store.inbox()` 得到 items **和** hidden，填 `InboxResp`。禁止 Chat 再调一个 `inbox_hidden()` HTTP。`do_inbox_hide` 用返回的 `hidden_until_id` 填 Push body。Push **不**从 Royal 发（与 `do_inbox_read` → `notify_account` 相同，`inbox.rs:196-207`）。

测试：Royal HMAC 200/401；**Chat HTTP 模式** `do_inbox_list` 的 `InboxResp.hidden` 非空（hide 之后）。

### `MessageStore` trait

```rust
pub struct InboxPage {
    pub items: Vec<InboxEntry>,
    pub hidden: Vec<HiddenDest>, // dest, kind, hidden_until_id
}

async fn inbox(&self, app: &str, account: &str, limit: i32)
    -> Result<InboxPage, StoreError>;

async fn hide_conversation(
    &self, app: &str, account: &str, dest: &str, kind: MessageKind,
) -> Result<i64, StoreError>; // hidden_until_id；无行时 0

async fn unhide_conversation(
    &self, app: &str, account: &str, dest: &str, kind: MessageKind,
) -> Result<(), StoreError>;
```

`inbox()` 签名从 `Vec<InboxEntry>` 改为 `InboxPage`，强迫 `HttpMessageStore` / Memory / Postgres / Counting stub **全部**实现 hidden，编译器挡住「只 map items」。`talk.rs` / `login.rs` stub 返回空 `hidden`。不另设 `inbox_hidden()` 以免 HTTP 双调用。

### 客户端：`delete_thread` = 本地优先 + 同事务 watermark + 上线先 flush 再 list

`mark_read` 的 spawn-and-forget 对未读角标可接受，对 hide **不可接受**。must-succeed 则离线滑删失败。消息 `outbox` 列是 talk payload，**不**把 hide 塞进去。

本地表（`SCHEMA_VERSION` 7 → 8）：

```sql
CREATE TABLE IF NOT EXISTS inbox_hides (
  account TEXT NOT NULL,
  dest TEXT NOT NULL,
  kind INTEGER NOT NULL,          -- 0 user / 1 group；由 thread_kind_from_name 写入
  hidden_until_id INTEGER NOT NULL,
  pending INTEGER NOT NULL DEFAULT 1, -- 1 = 服务端尚未确认
  PRIMARY KEY (account, dest, kind)
);
```

`threads.kind` 是 TEXT `'user'|'group'`（`schema.rs:15`），**不是** INTEGER。读取必须 `thread_kind_from_name`（`timeline.rs:233-238`）。

**一条 IMMEDIATE 事务**（禁止「紧随」第二次 commit）。`delete_thread_tx`（`store/mod.rs:1579-1590`）改为：

1. `SELECT kind, last_message_id FROM threads WHERE account=? AND id=?`（在 DELETE 之前）。无行：`kind=0`，`last=0`，仍 INSERT hide（空本地滑删 / ensureThread）。
2. 现有五条 DELETE（outbox / messages / threads / read_watermarks / timeline_meta）。
3. `INSERT INTO inbox_hides ... pending=1, hidden_until_id=<step 1>`（`ON CONFLICT` 覆盖为 pending=1 和新 last）。
4. commit。

进程若在 commit 前崩溃：threads 还在，用户再滑一次。commit 后崩溃：有 watermark，Goal 1 成立。

`migrate.rs` 今日只升到 v7（`migrate.rs:64-67`）。必须加 `if version < 8 { migrate_v8; set 8 }`。`schema.rs` 的 `CREATE TABLE IF NOT EXISTS` 每次 open 会建表，但 `prepare.rs:31` `v <= SCHEMA_VERSION` 的 Keep 路径要 `SCHEMA_VERSION = 8` 才诚实。

`KimSdk::delete_thread` 事务之后：

1. **从 `timelines` HashMap 摘掉该 dest 的订阅**（不只 reset 窗口）。publish `Resync { reason: "deleted" }`。开着的 watch 结束；后续 persist 不再向已死的 sender 推 snapshot。
2. 若 `protocol()` 可用：`spawn` hide RPC（见重试）。成功：`pending=0` 且 **`hidden_until_id := 响应的服务端值`**，然后 **`maybe_unhide_from_stored_messages(dest)`**（见下）。失败：按重试表（1xx 删 hide 行；2/3 保持 pending）。
3. Flutter：若当前路由就是该 dest，**必须 pop**（宽屏则清空 selected）。这不是可选 UX。

#### pending=1 的硬规则（只挡列表，不挡 messages / ACK）

`persist_inbox` / `apply_incoming`：**只要 `pending=1`，绝不 upsert `threads`**，不论 inbox `last_message_id` 或 talk `message_id` 是否更大。本地 last=100、服务端 last=105、hide RPC 尚未落地时，reconnect 不得把格写回。

`apply_talk`（写 `messages`）**始终执行**（与是否 hide 无关）。Live persist 仍是 `supervisor.rs:143-164` persist-then-ack：messages 写入成功才 `ack`。禁止 skip `apply_talk` 却 ACK——那是新的 ACK 语义，且会让 `pending_delivery` 不再投递这条「unhide 触发 id」。

`pending=0` 之后：`message_id > hidden_until_id`（或 inbox `last_message_id > WM`）→ 删 hide 行并 upsert thread（真 unhide）。`InboxResp.hidden` 中的 dest：删 `threads`，写入/更新 `inbox_hides pending=0` 且 watermark=服务端值。

**hide ack 之后**（本进程、不必等下次 `SyncEngine::run`）：

```text
pending = 0
hidden_until_id := server WM
maybe_unhide_from_stored_messages(dest):
  SELECT MAX(message_id) FROM messages WHERE account=? AND dest=?
  if max_id > server WM:
    DELETE inbox_hides
    upsert thread from that latest local message
  refresh_session_snapshot
```

这样：pending 窗口落入的 id=106 已在 SQLite；hide ack 返回 WM=105 时 dest **立刻**回列表，不等下一次进程启动。不为此再打一次 `inbox_list`（connect 时已经 list；in-session ack 扫本地 id 更便宜，且不改 ACK）。

#### 上线顺序：先 **有界** flush hide，再 `inbox_list`

`SyncEngine::run`（`sync.rs:160`）今天第一句就是 `inbox_list`。`kick_outbox`（`lib.rs:1510-1513`）只叫醒消息 outbox，与 sync **无顺序**。今日 `persist_inbox` Err 才中止 sync（`sync.rs:163-166` `SyncFailed` + `return Ok(0)`）。

`PersistHook`（`persist.rs:22-29`）增加：

```rust
/// Best-effort. Must not fail the sync: timeout/transport → Ok(()).
async fn before_inbox_sync(&self) -> Result<(), PersistError> { Ok(()) }
async fn persist_inbox(
    &self,
    items: &[InboxItem],
    hidden: &[InboxHiddenItem],
) -> Result<(), PersistError>;
```

`SyncEngine::run`：

```text
hook.before_inbox_sync().await   -- 忽略返回值（磁盘灾难除外可 Ok）
                                 -- 预算: 2s 墙钟 或 16 个 pending dest，先到为准
                                 -- timeout / 传输 / Status=3: 行保持 pending=1，**继续**
client.inbox_list()              -- 返回 items + hidden
hook.persist_inbox(items, hidden)
  Err => SyncFailed; return Ok(0)   -- 只有 list 的 persist 失败才中止 sync
... offline pages / persist_talks
```

`SdkPersistHook::before_inbox_sync`：扫 `pending=1`，在预算内发 hide。**不得**把 flush 失败变成跳过 `inbox_list`。pending 守卫仍然挡住 thread upsert。

`KimClient::inbox_list`（`client.rs:230`）改为返回 `(Vec<InboxItem>, Vec<InboxHiddenItem>)`。`Event::Inbox` 带 `hidden`。

#### persist_talks 与开着的 transcript

选定：**unsubscribe timeline + 仍写 messages + 只跳过 thread upsert**。

Goal 7 的「transcript 不填回」靠摘订阅，不靠丢弃落盘。开着的 watch 已死，`bump_timeline_version` 对已摘 dest 无 UI。用户稍后从通讯录打开走 `unhide` + `chat.history`（本地 messages 若已有则 hydrate 更快）。

对 `dest ∈ inbox_hides`：

| 条件 | `apply_talk`（messages） | `apply_incoming`（threads） |
|---|---|---|
| `pending=1` | **写入**（然后 ACK） | skip |
| `pending=0` 且 `message_id <= hidden_until_id` | **写入**（然后 ACK） | skip |
| `pending=0` 且 `message_id > hidden_until_id` | 写入 | upsert，删 hide 行 |

`upsert_on_send`（本地发消息）：视为用户主动续聊 → 删 hide 行、upsert thread、发 `chat.inbox.unhide`（best-effort）。服务端 insert 也会推进 last。

#### hide RPC 重试

无 `last_error` 列，不发明该字段。

| Status / 错误 | 行为 |
|---|---|
| Success=0 | `pending=0`，本地 WM := body.`hidden_until_id`，然后 `maybe_unhide_from_stored_messages` |
| 1xx（含 101）/ 4xx / `NoDestination=300` | **不重试**。**删除 `inbox_hides` 行**（`pending` 不再存在）。服务端拒绝 ⇒ 本机不得继续假装已 hide，否则 `persist_inbox` 永远跳过该 dest。用户可再滑一次 |
| `CommandNotFound=2` | **保持 pending=1**。本连接不再 flush 该 dest；**下次新 session** 的 `before_inbox_sync` 再试一次。混部升完服务端后 hide 会落地 |
| `ServiceUnavailable=3`、传输错误 | 本轮 `before_inbox_sync` 预算内可再试；超时则 pending=1，**继续** `inbox_list`。本设备因 pending 守卫不复活列表 |
| 无行空操作 Success，`hidden_until_id=0` | `pending=0`。服务端仍 Push，其它设备 prune 空格 |

#### Push 解码（PR2，不是 PR3）

`kim-client` `decode_logic` 对其它 Push 落到 `Event::Status`（`wire.rs:804-808`），**不会**变成 session 事件。必须新增：

```rust
Event::InboxHidden { dest, kind, hidden_until_id }
Event::InboxUnhidden { dest, kind }
```

解码 `Flag=Push` 且 command 为 `chat.inbox.hide` / `unhide`。**Push `Header.dest` 就是会话 id**。

- Hidden：本地删除（与 `delete_thread_tx` 相同五行 + `inbox_hides pending=0`、WM=Push 值），**禁止**再打 hide RPC。然后 `refresh_session_snapshot`。
- **Unhidden（Goal 3 账号级）：** 删 hide 行；**`ensure_thread(dest, kind)`** 插入空 `threads` 行（title=dest，last 空即可；打开再 hydrate history）；`refresh_session_snapshot`。禁止「只删 hide、等下次 reconnect 的 inbox_list」——B 设备列表会空到重连。

PR3 不再做第二条解码路径；Flutter 列表来自 SDK snapshot（`inbox.dart:77-86`）。

#### `openKimChat` / FFI

`openKimChat`（`open_chat.dart:11-27`）今日只 `ensureThread`。PR3 调 SDK `unhide_thread`（best-effort）。**FFI `unhideThread` + `ProtocolClient::unhide_conversation` + `KimSdk::unhide_thread` 在 PR2 落地**，PR3 只接线。不在 `chat.history` 隐式 unhide。

#### Web（v1 **只收** hide/unhide，不做滑删手势）

`sdk/web/src/store.ts` 确实只存消息 ACK。**app 不是**：`threads.ts` `kim.web.threads.${account}` + `ChatProvider` upsert-only。Q4 已关闭。

Web 没有 SQLite `inbox_hides.pending`。若 PR3 做滑删，RPC 失败 + reconnect 会从 `items` 复活（与 mobile 同一洞）。**v1 不在 PR3 加 web 滑删**，因此不需要 `pendingHide`。Web 只消费：

PR2：

- `decodeInboxResp` 返回 `{ items, hidden }`（`proto.ts:593` 今日只 items）。
- `client.inbox()` 返回 hidden（`client.ts:562-568`）。
- `Command.InboxHide` / `InboxUnhide`；`inboxHide` / `inboxUnhide`（给以后手势用，PR3 不接线）。
- Push `chat.inbox.hide|unhide` 进 client 事件。

PR3：

- connect 后用 `items` + `hidden[]` **prune** 内存和 `localStorage`（对 `hidden` 的 dest `removeThread`，禁止只 upsert）。
- Push hide：从 cache 去掉 dest；若 `activeId===dest` 则离开聊天页。
- Push unhide / `hidden[]` 不再包含该 dest：若 `items` 有则 upsert；否则 **插占位 thread**（与 SDK `ensure_thread` 同）。不得等重连。
- **不做** ConversationList 滑删。以后若做，必须加 in-memory `pendingHide`：connect 的 `session.inbox()` 之前 flush，且 pending 集合内禁止 `upsertThread`（含 `"message"` reducer，`ChatProvider.tsx:186-208`）。
- 测试：`sdk/web/tests` decode hidden；ChatProvider prune **和** unhide 占位。

#### Agent extras vs hide

`withLocalThreads`（`inbox.dart:51-71`）会把 enabled agent dest 重新插进 Flutter 列表，即使 SDK 已 hid。规则：

- **有 `serverAccount` 的已注册 bot**：dest 与云 inbox 相同。若在 SDK `inbox_hides` / snapshot 隐藏集中，**禁止** extras 插回。
- **无 server 身份的 host-local agent**：不是云会话，不在 `inbox_hides`，extras 可以保留（桌面 Goose 快捷入口）。

PR3：`withLocalThreads` 接收 SDK 暴露的 hidden dest 集合（snapshot 增 `hidden_dests: Vec<String>`，PR2 填）。

### 群 / bot / 自己

- 群：`kind=1`，dest=group id。成员仍在，后续群消息 unhide。不是 `chat.group.quit`。
- Bot 1:1：hide 与人类相同。`chat.bot.delete` 仍 `purge_peer_dm`（物理、双向）。owner 误走 `friend.remove` 仍 `BotSocialDenied=113`。
- dest=自己：talk 允许（`control-layer-chat.md`）；hide 同样允许。

---

## Data Model Changes

### Postgres

`conversation_inbox` 新增 `hidden_until_id BIGINT NOT NULL DEFAULT 0`。无需部分索引：hide 写是点查 PK；list 已按 `(app, account, last_send_time DESC)`。v1 不做部分索引。

`purge_peer_dm`（`postgres.rs:1511-1597`）已 `DELETE FROM conversation_inbox` 该 pair 两侧，新列随行走。bot 删除后 hide 状态一并消失，正确。

后期 clear-for-me：另加 `cleared_before_id`，**不要**复用 `hidden_until_id`。GC 必须 refcount，不能照抄 `purge_peer_dm` 直接删 content。

### SQLite（kim-sdk）

`inbox_hides` 必须与五条 DELETE 同事务。`SCHEMA_VERSION = 8` + `migrate_v8`。`delete_thread` 继续物理删本地消息；服务端 history 仍在。

### 存储量

每行 +8 字节。`InboxResp.hidden` 上限 500，`ORDER BY last_send_time DESC, last_message_id DESC`。

---

## Alternatives Considered

### A. 滑删 = 删我的 `message_index`（拒绝作为 v1）

新消息 unhide 需要历史还在；与「清空聊天」混为一谈。

### B. `friend.remove` 时（互删才）`purge_peer_dm`（拒绝）

非消费级 IM 默认。bot 删除已经走这条。

### C. 仅本地 watermark、无服务端列（拒绝）

其它设备无法一致；重装丢失。

### D. hide 必须成功才删本地（拒绝）

离线滑删失败。`pending=1` 硬守卫 + 上线先 flush 给出正确性。

### E. 新消息不 unhide，必须手动（拒绝）

与微信/WhatsApp/Telegram 默认相反。

### F. `chat.history` 即 unhide（拒绝）

`hydrate_watched`（`lib.rs:1125-1131`）+ 未摘订阅会把「聊天页还开着时滑删」立刻弄回来。显式 `unhide` 绑在 `openKimChat`。v1 改为 **unsubscribe**，从根上拆掉这条隐式路径。

### G. skip `apply_talk` + 仍 ACK（拒绝）

会改 persist-then-ack（`supervisor.rs:157-162`）的含义：没落盘却 ACK，G-03/`pending_delivery` 不再重投。pending 窗口里的 id 正是「新消息 unhide」触发器。v1 改为 **仍写 messages、只 skip thread upsert**；UI 靠 unsubscribe。上一轮因「开着的 timeline 会填满」拒绝「写 messages」——unsubscribe 之后该理由不成立。

---

## Security & Privacy Considerations

- hide 只改调用者的 `conversation_inbox` 行。伪造 dest 最多藏自己的视图。
- 不新增越权面：`Header.dest` + session.account 已是 inbox.read 模型。
- history 仍返回隐藏会话的消息。hide 不是保密边界，也不是 GDPR。
- Royal hide / inbox HTTP 必须 HMAC。`classify` 已归 inbox 组。
- 群 hide 不泄露成员、不触发 quit。
- 空操作仍 Push：只向 **本账号** 其它 location 发，不向 peer 发。

威胁：恶意客户端对自己每个 dest 调 hide——廉价 UPDATE，账号锁串行，可接受。

---

## Observability

- `kim_handler_duration_seconds`：`COMMANDS` 现 34 条（3 login + 31 `chat.*`）。加入 `chat.inbox.hide` / `unhide`。
- counter `kim_inbox_hide_total{op="hide|unhide"}`。
- gauge/counter：`kim_inbox_hidden_set_size`（本次 list 的 hidden 行数）；`kim_inbox_hidden_set_truncated_total`（命中 LIMIT 500）。
- hide 走 Royal：`kim_royal_rpc_seconds{path_group="inbox"}`（前缀匹配，无需改 classify）。
- 日志：`account` + `dest` + `kind` + `hidden_until_id`，不打 body。`CommandNotFound=2` flush skip 打 debug。`before_inbox_sync` 超时打 warn，**不**升错误。
- 告警：非必须。

---

## Rollout Plan

1. **迁移先行**：`0016` additive，`DEFAULT 0`。回填必须保留 `hidden_until_id`。
2. **服务端先于客户端**：Royal `InboxResp.hidden` + hide 响应 + 两条 list 过滤。旧客户端忽略 field 2，从不调 hide → 滑删仍复活。可接受。
3. **混部**：新客户端打到旧 Chat → Status 2。pending=1 保持本机隐藏，**下次新 session** 再试，不热循环重打。1xx/4xx/300 则删 hide 行（服务端拒绝，本机不再撒谎）。
4. **新客户端**：同事务 `inbox_hides`；`before_inbox_sync` flush；persist 尊重 pending 与 `hidden[]`；Push 解码。其它设备在线靠 Push，离线靠 list 的 `hidden[]`（生产 HTTP 必须带上）。
5. **无 feature flag**。
6. **回滚**：停新客户端 hide RPC；列留下无害。`UPDATE conversation_inbox SET hidden_until_id=0 WHERE hidden_until_id>0`。
7. 生产默认 `KIM_INBOX_MATERIALIZED=0`；GROUP BY 后聚合再 join 的谓词必须在 PR1 落地。

---

## Tests

### Store（postgres `DATABASE_URL` 门控 + Memory 同构）

- `hide_filters_inbox_group_by` / `hide_filters_inbox_materialized`：talk 后 hide；`inbox().items` 不含 dest；`inbox().hidden` 含 dest；`history()` 仍有行。
- `new_message_unhides`：hide 后对端 insert；items 再出现；`hidden_until_id` 仍为旧值但 `ci.last_message_id` 更大。
- `hide_idempotent_and_missing_dest`：两次 hide Success；无 index 无 inbox 行 → Success 且 `hidden_until_id=0`、不 INSERT 行。
- `hide_lock_serializes_with_insert`。
- `unhide_clears_watermark`。
- `purge_peer_dm_still_clears_hidden_row`（`store/mod.rs:2357`）。
- `dual_max_skew_does_not_leak_hidden`：构造 `send_time` 更大但 `message_id` 更小的后到消息，hide 以元组 last 为 WM；GROUP BY 路径不得因 `MAX(message_id)` 把 dest 漏回 items。
- `last_equals_hidden_until_id_is_hidden`。
- `hide_filter_before_limit`：100 可见 + N 隐藏 → page 仍 100 可见，不是 100-N。
- `hide_upsert_from_index_uses_distinct_on`：无 inbox 行、有 index；UPSERT 的 `last_message_id` 等于 backfill oracle，unread 等于 mark_read 重算。
- 回填不得把 `hidden_until_id` 打回 0。

### Handler / e2e / Royal

- `e2e_inbox.rs`：hide → list items 空 + hidden 含 dest → history 1 条 → 对端 talk → list 1 条。
- 空 dest → 300；坏 kind → 101。
- 群 hide：成员仍可 talk；发送方 list 暂无该项直到下一条。
- `do_friend_remove` 后 history/list 仍在（**不要**顺便 hide/purge）。
- Royal：`POST /api/v1/inbox/hide` HMAC 200，body 含 `hidden_until_id`；缺 HMAC 401。
- **Chat HTTP 模式**：hide 后 `do_inbox_list` 的 proto `hidden` 非空（回归 Issue 2：适配器不得丢字段）。
- 空操作 hide 仍向其它 location Push。

### SDK / kim-client

- 现有 `delete_thread_drops_outbox_so_restart_does_not_send`、`delete_thread_resync_clears_older` 仍过。
- `delete_thread_writes_inbox_hides_same_tx`：mock 在 DELETE 后、commit 前 panic 的路径不可在生产测试，用 SQL 查同一连接；至少断言函数返回后二者同时存在或同时不存在。
- **复活回归** `delete_thread_persist_inbox_does_not_resurrect`：同 last → 仍无 dest。
- `pending_hide_blocks_unhide_on_stale_local_last`：本地 WM=100、`pending=1`、`persist_inbox` last=105 → dest **不**回来（thread skip）；messages 若随 talk 到达仍写入。
- `pending_talk_persisted_unhides_after_hide_ack`：`pending=1`、live/offline 写入 id=106（大于本地 last=100）且 ACK；hide ack 返回 WM=105 → **不必重连**，`load_threads` 含 dest。
- `before_inbox_sync_flushes_before_list`：hide RPC **启动**早于 `inbox_list`。超时 / Status=3：**仍调用** `inbox_list`（断言 list 被调用）；pending 仍为 1。
- `offline_ids_between_local_and_server_last_persist_messages_skip_thread`：pending 或 `id <= WM` 的 persist_talks **写 messages、ACK、不 upsert thread**。
- `inbox_resp_hidden_prunes_local_thread`。
- `push_inbox_hide_decodes_and_persists`：`Event::InboxHidden` → 本地删 + pending=0，无二次 RPC。
- `push_inbox_unhide_restores_thread_without_reconnect`：B 已 hide；Unhide Push 后 `load_threads` 含 dest（空行即可）。
- `command_not_found_keeps_pending`：Status 2 不在本连接热循环重打。
- `invalid_packet_body_deletes_hide_row`：Status 101 → 无 `inbox_hides` 行，随后 `persist_inbox` 可 upsert。
- `delete_thread_unsubscribes_timeline`：之后 persist_talks 不向该 dest 的旧 watch 推消息（messages 仍落盘）。

### Flutter / web

- tile 仍调 `deleteThread`。PR3：`openKimChat` unhide；滑删当前 dest **pop**；`withLocalThreads` 不插回 hidden dest。
- web：`decodeInboxResp` 含 hidden；ChatProvider prune + unhide 占位；**无滑删手势**。

---

## Open Questions

1. **从通讯录打开是否 unhide（默认：是）**。微信如此。若产品要「点开也不回列表、只等新消息」，PR3 可去掉 `openKimChat` 的 unhide，协议已具备。
2. **隐藏集 cap 500**。超活跃隐藏是否要分页 / `truncated` 字段；v1 只 metric + 稳定 ORDER BY。
3. **非好友 history 可见性**。默认保持可读。独立 flag，默认关。不在 v1。

已关闭：

- ~~web 是否缓存 dest~~：缓存，`kim.web.threads.${account}`，upsert-only。PR2 解码 hidden，PR3 prune + unhide 占位。
- ~~滑删时聊天页是否 pop~~：必须 pop（或宽屏清空选中），与 unsubscribe 配套。不是可选 UX。
- ~~web 是否在 PR3 做滑删~~：否。v1 web 只收 Push/`hidden[]`。滑删以后要 `pendingHide`。

---

## References

- `docs/user-social-inbox.md` — 好友与 inbox 已落地形状；人类 remove 不清历史。
- `docs/control-layer-chat.md` — 命令、1xx、dest 规则。
- `docs/production-gaps.md` — G-17 物化读；inbox 非全量。
- `docs/mobile-client.md` — Dart 不持业务；threads 来自 SDK snapshot。
- `services/chat/migrations/0010_conversation_inbox.sql`、`0006_user_social_inbox.sql`、`0001_messages.sql`。
- `services/chat/src/store/postgres.rs` — insert/list/history/mark_read/purge。
- `services/chat/src/royal.rs` `HttpMessageStore::inbox` 只 map items。
- `crates/kim-sdk/src/store/outbox.rs` `delete_thread`；`threads.rs` `persist_inbox_item`；`migrate.rs` v7；`schema.rs` `threads.kind` TEXT。
- `crates/kim-client/src/sync.rs` — inbox then offline pages；`persist.rs` PersistHook；`wire.rs:804` 未知 Push → Status。
- `sdk/web/app/lib/threads.ts`、`app/state/ChatProvider.tsx:415-424, 608-624`、`src/proto.ts:593`。
- 探索稿 `dm_purge_marks`：仅作反例，不采用。

---

## Key Decisions

1. **v1 只做「对我隐藏会话」**，不做清记录 / 撤回 / 人类 purge。拒绝：一个「server delete」打穿存储。
2. **状态放 `conversation_inbox.hidden_until_id`**。有物化行时可见谓词是 **`ci.last_message_id > ci.hidden_until_id`**，不是 `MAX(message_id)`。GROUP BY 先聚合再点查 ci。拒绝：mark 表、`hidden_at`、删 index、每行 CASE join。
3. **GROUP BY 与物化路径都过滤**。生产默认 `KIM_INBOX_MATERIALIZED=0`。拒绝：只改物化表。
4. **服务端 `friend.remove` 不碰消息存储、也不 hide**。拒绝：互删物理抹除；拒绝 PR1「顺便」hide。
5. **客户端本地优先 + 同事务 `inbox_hides` + `pending=1` 硬守卫（只挡 thread upsert）+ 上线有界 `before_inbox_sync` 再 list**。ack 后本地 WM := 服务端值，并扫本地 `messages.message_id > WM` 决定是否立刻 unhide。拒绝：must-succeed；拒绝抄 `mark_read` spawn；拒绝「紧随」第二次事务。
6. **`POST /api/v1/inbox` 返回 `InboxResp { items, hidden }`**（一次 RPC）。hide HTTP 出 `{ hidden_until_id }`。`HttpMessageStore.inbox` 改为 `InboxPage`。同账号 Push。Unhide Push 必须 `ensure_thread`。拒绝：独立 `/hidden` 二次调用；拒绝适配器只 map items。
7. **history 不隐式 unhide；打开会话显式 unhide。`delete_thread` unsubscribe；滑删当前页必须 pop。隐藏期间仍 `apply_talk` + persist-then-ack，只 skip thread upsert。** 拒绝：skip messages 却 ACK（新 ACK 语义 / 吞 unhide id）。
8. **人类物理 purge 非 v1**；注销/GDPR 再复用 `purge_peer_dm`。
9. **Flutter 删好友继续组合 `deleteThread`（因而 hide）；其它只调 `chat.friend.remove` 的客户端不 hide。** 这是产品分叉，写明以免有人「修」进 `do_friend_remove`。
10. **Status 2 保持 pending，仅下次新 session 再 flush。1xx/4xx/300 删除 hide 行（服务端拒绝，本机不再撒谎）。** 3 与传输：预算内可试，超时继续 list。空操作 hide 仍 Push。无 `last_error` 列。
11. **Push 解码在 PR2**（`Event::InboxHidden/Unhidden`）。Unhidden → `ensure_thread` + snapshot。PR3 只做 Flutter/web UX。Header.dest = 会话 id。
12. **已注册 bot extras 不得插回 hidden dest**；无 server 身份的 host-local agent 可以。
13. **v1 web 只收 hide/unhide（Push + `hidden[]`），PR3 不加滑删手势。** 以后手势必须有 `pendingHide`。

---

## PR Plan

每 PR 可独立审查、独立合入。

### PR1 — server hide conversation

- **标题：** `chat: per-account inbox hide (hidden_until_id + chat.inbox.hide)`
- **依赖：** 无
- **文件：**
  - `services/chat/migrations/0016_inbox_hide.sql`
  - `services/chat/src/store/mod.rs`（`InboxPage` + Memory 三方法）
  - `services/chat/src/store/postgres.rs`（hide/unhide、物化 WHERE、GROUP BY 后聚合 join、DISTINCT ON miss-fill、unread 重算）
  - `services/chat/src/inbox.rs`（`do_inbox_hide` / `do_inbox_unhide`；list 填 items+hidden；hide Success body；空操作仍 Push）
  - `services/chat/src/lib.rs`（router.handle）
  - `services/chat/src/royal.rs`（`HttpMessageStore::inbox` → `InboxPage` 含 hidden；hide 解码 `hidden_until_id`，禁止 `post_maybe_empty`；unhide）
  - `services/chat/src/login.rs` / `talk.rs` Counting stub
  - `services/chat/tests/e2e_inbox.rs` 及 store 单测（含 dual-MAX、filter-before-LIMIT、HTTP list hidden）
  - `crates/kim-protocol/proto/pkt.proto`、`src/wire.rs`、`src/command.rs`（ALL/CONSTANTS 46）、`src/lib.rs` CMD re-export
  - `crates/kim-metrics/src/lib.rs` `COMMANDS`
  - `services/royal/src/lib.rs`、`product.rs`（`/api/v1/inbox` 出 hidden；`/inbox/hide|unhide`）
  - `deploy/backfill-inbox.sql`（ON CONFLICT 保留 `hidden_until_id`）
  - `docs/user-social-inbox.md`、`docs/control-layer-chat.md`
- **内容：** schema、两条谓词、命令、Royal 一次 inbox RPC 带回 hidden、hide 响应 id、Push（Header.dest=会话）、e2e。不改 SDK/Flutter。不在 `do_friend_remove` 调 hide。

### PR2 — kim-sdk / kim-client：滑删打 hide，persist 不再复活

- **标题：** `sdk: delete_thread hides on server; persist skips hidden dests`
- **依赖：** PR1
- **文件：**
  - `crates/kim-sdk/src/store/schema.rs`（`SCHEMA_VERSION=8` + `CREATE inbox_hides`）
  - `crates/kim-sdk/src/store/migrate.rs`（`migrate_v8`）
  - `crates/kim-sdk/src/store/mod.rs` `delete_thread_tx`（同事务 SELECT→DELETE→INSERT hide）
  - 新 `inbox_hides.rs`；`persist_inbox_tx` / `persist_talks_tx` pending 守卫
  - `crates/kim-sdk/src/lib.rs` `delete_thread`（unsubscribe）、`unhide_thread`、pending flush
  - `crates/kim-sdk/src/proto.rs` `ProtocolClient` hide/unhide
  - `crates/kim-client/src/persist.rs` `before_inbox_sync` + `persist_inbox(..., hidden)`
  - `crates/kim-client/src/sync.rs` 先 flush 再 list
  - `crates/kim-client/src/client.rs` `inbox_list` 返回 hidden
  - `crates/kim-client/src/wire.rs` / `events.rs`：`Event::InboxHidden` / `InboxUnhidden`；Inbox 事件带 hidden
  - `sdk/mobile/rust` FRB：`unhideThread`、session snapshot `hidden_dests`
  - `sdk/web/src/command.ts`、`client.ts`、`proto.ts` `decodeInboxResp` 返回 hidden；Push 解码
  - 测试：复活、pending 挡 thread、hide ack 后扫本地 id unhide、flush 超时仍 list、offline 写 messages skip thread、Push hide/unhide、Status 2、Status 101 删 hide 行、unsubscribe
- **内容：** 同事务 watermark、hide RPC、ack 换 WM + `maybe_unhide_from_stored_messages`、pending 只挡 thread、有界 flush、Push 解码（Unhidden `ensure_thread`）、web proto。无 Flutter `openKimChat`、无 ChatProvider prune UI、**无 web 滑删**。

### PR3 — Flutter / web UX

- **标题：** `mobile+web: unhide on open; pop on swipe; prune thread cache`
- **依赖：** PR2（含 FRB `unhideThread` 与 web `hidden[]`）
- **文件：**
  - `sdk/mobile/lib/router/open_chat.dart`（打开 → `unhideThread`）
  - `sdk/mobile/lib/features/chats/chats_page.dart` / 路由：滑删当前 dest **pop**
  - `sdk/mobile/lib/features/chats/inbox.dart` `withLocalThreads` 跳过 hidden dests
  - `sdk/mobile/test/support/fake_kim.dart`
  - `sdk/web/app/state/ChatProvider.tsx`：connect 用 items+hidden prune；Push hide 掉格；Push unhide 占位；`activeId` 离开
  - `sdk/web/app/lib/threads.ts`；`sdk/web/tests`
- **内容：** UX 接线。不第二次实现 Push 解码。**不加 web 滑删。**

### 后期（不在 v1 实施，仅挂号）

| PR | 标题 | 说明 |
|---|---|---|
| PR4 | `chat: clear conversation for me` | `cleared_before_id`；history 下界；删我的 index；content refcount GC；**不**改 hide |
| PR5 | `chat: recall / delete for everyone` | 时间窗；双方 tombstone |
| PR6 | `chat: account deletion purge` | GDPR/注销；按 peer 循环 `purge_peer_dm` |
| PR7 | `chat: hide history while not friends` | 可选读路径 filter；**默认关** |
| — | 人类互删物理 purge | **不做**，除非产品书面推翻 Decision 4/8 |

---

## v1 / later 分界

```text
v1 (PR1-PR3)
  * chat.inbox.hide / unhide
  * conversation_inbox.hidden_until_id
  * inbox.list 双路径过滤 + InboxResp.hidden (Royal 同包)
  * SDK 同事务 inbox_hides; pending 只挡 thread; 有界 flush-before-list
  * apply_talk + ACK 在 hide 期间仍走; hide ack 扫本地 id unhide
  * Push decode in PR2 (Unhidden ensure_thread); openKimChat unhide + pop in PR3
  * web receive-only (no swipe)
  (existing) chat.bot.delete -> purge_peer_dm
  (existing) friend.remove -> DELETE friendships only
  (existing) Flutter unfriend composes deleteThread

later
  clear-for-me / recall / account purge / optional not-friend visibility
  x mutual-unfriend physical purge as default
```
