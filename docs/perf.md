# 性能路径（已落地）

对照小册第 26–28 章。本文只记代码现在怎么写。

## 通信层

- TCP 生产写路径：`header_bytes` 栈上 5 字节头 + payload 两次 `write_all`。`encode_frame` 留给 codec 单测。
- 拆分后的 TCP 写半边是 1KiB `BufWriter`。`write_frame` 不 flush；Channel 一批 mailbox 只 flush 一次。`write_wait` 包住整批含 flush。Close 先 flush 再 shutdown。超时/写失败会 `shutdown`（BufWriter 在 shutdown 时 flush），不要指望 Drop flush。
- 未拆分的 `TcpConn` 仍然无缓冲，`write_frame` 仍然不 flush。
- WebSocket **写出仍拷贝**（fastwebsockets 0.10 的 `Payload::Bytes` 是 `BytesMut`）。读到 `BytesMut` 时 `.freeze()`。

CPU 分析用 `samply` / `cargo flamegraph`，不要加 pprof Cargo feature。`--all-features` 不能拉分析器 C 依赖。

## 存储寻址

- Redis `get_locations` 仍是一次 `MGET`。
- `CachedSessionStore` 包在 Redis 打开之后（`KIM_LOC_CACHE=0` 可关）。`Location` 没有 account，cache miss 走 N 次 `get_location`。
- `DualWriteStore` 始终编译。`REDIS_MIRROR_URL` 只在 Redis feature 运行时使用。Memory+Memory 单测不连 Redis。
- Postgres HASH SQL 在 `examples/fake-chat/scripts/hash_partition.sql`，**不会**被 `sqlx::migrate!` 执行。
