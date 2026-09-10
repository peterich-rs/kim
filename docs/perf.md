# 性能路径（已落地）

对照小册第 26–28 章。本文只记代码现在怎么写。

## 通信层

- TCP 生产写路径：`header_bytes` 栈上 5 字节头 + payload 两次 `write_all`。`encode_frame` 留给 codec 单测。
- 拆分后的 TCP 写半边是 8KiB `BufWriter`。`write_frame` 不 flush；Channel 一批 mailbox 只 flush 一次。`write_wait` 包住整批含 flush。Close 先 flush 再 shutdown。超时/写失败会 `shutdown`（BufWriter 在 shutdown 时 flush），不要指望 Drop flush。
- 未拆分的 `TcpConn` 仍然无缓冲，`write_frame` 仍然不 flush。
- WebSocket 写出用 `Payload::Borrowed`（`Bytes` 活过 `write_frame`）。服务端 writev 零拷贝。客户端 RFC6455 mask 仍可能经 `to_mut` 拷一次。读到 `BytesMut` 时 `.freeze()`。

CPU 分析用 `samply` / `cargo flamegraph`，不要加 pprof Cargo feature。`--all-features` 不能拉分析器 C 依赖。

## 存储寻址

- Redis `get_locations`：单账号 `HVALS`；多账号 `pipe().cmd("HVALS")` 一次往返，失败再逐个回退。不是 `MGET`。
- Session / ACK / nonce / 吊销 / device hot 六个 `ConnectionManager` 共用 `open_connection_manager`（连接/响应超时 3s）。
- `CachedSessionStore` 包在 Redis 打开之后，且仅当 `KIM_LOC_CACHE=1` / `true`（opt-in；compose 默认 `0`）。内部是 moka（session 20 万 / loc 50 万、TTL 60s、NotFound 负缓存 5s）。`add`/`delete` 本进程 write-through 并 `invalidate` loc。跨实例：Redis 写入成功后 `PUBLISH kim:loc:inv {account}`（Chat 与 Royal 的 `RedisSessionStore` 都会发）；开缓存的 Chat 启动时必须 `SUBSCRIBE` 成功，否则拒绝启动。订阅断开会清空本地缓存再重连。TTL 60s 只覆盖「漏掉一条 PUBLISH」的尾巴。多账号 miss 走一次 `get_locations`（Redis pipeline HVALS），再按账号 `list_locations` 填缓存（`Location` 没有 account，无法从扁平列表拆回）。
- `DualWriteStore` 始终编译。`REDIS_MIRROR_URL` 只在 Redis feature 运行时使用。Memory+Memory 单测不连 Redis。
- Postgres 业务池 `statement_timeout=5s`；migrate 走独立连接（不带 5s）。`min_connections` 默认 0，`max_lifetime` 默认 30min。
- Chat→Royal HTTP 重试带退避；私聊 exists/block/friend 共享 800ms 目录预算。
- Postgres HASH SQL 在 `services/chat/scripts/hash_partition.sql`，**不会**被 `sqlx::migrate!` 执行。
