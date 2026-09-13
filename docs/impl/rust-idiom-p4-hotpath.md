# 第 4 刀：热路径所有权（有界）

父规格：[rust-idiom-upgrade.md](./rust-idiom-upgrade.md)。

## 必须

1. `LaneKeyFn` 改为 `Arc<dyn Fn(&[u8]) -> Option<Arc<str>> + Send + Sync>`。`logic_channel_id` 返回 `Option<Arc<str>>`（从 header.channel_id 做 `Arc::from`，避免再 `String` 再 `Arc::from`）。`channel.rs` `lane_id` 不再二次包装。
2. `Context::dispatch_cmd` 已用 `HashMap<GatewayId, Vec<ChannelId>>`（第 1 刀）。确认没有多余 `gate_id.clone()` 成 String。`packet.clone()` 可留（body 已是 Bytes）。
3. 不要改 InsertMessage.body 为 Bytes（protobuf MessageReq.body 仍是 string，收益小、面大）。

## 不做

- reuseport / vectored write / jemalloc / io_uring
- sdk/*
- ChannelMap / WriteShared（已完成）
- ahash 换 hasher（无 bench 数字不换）

## 验收

`cargo fmt && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace --offline`
