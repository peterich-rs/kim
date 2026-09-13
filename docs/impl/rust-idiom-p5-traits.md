# 第 5 刀：trait 现代化（有界）

父规格：[rust-idiom-upgrade.md](./rust-idiom-upgrade.md)。

## 必须（小而可编译）

1. **不要**一次删光 195 个 async_trait。只把 **没有作为 dyn 使用的、或仅单实现的** 先留着。
2. 本刀实际落地：
   - `Server` 拆成文档注释 + `ServerHandle` 类型别名不强制。若拆 builder 会爆，则：**edition 保持 2021**；给 `kim-core` / `kim-protocol` / `kim-router` 的 `lib.rs` 加 `//!` 已有模块文档即可。
   - workspace clippy 增加 `unwrap_used = "warn"`（不是 deny），避免测试爆炸。
   - 根目录加最小 `deny.toml`（licenses + advisories 占位，`cargo deny` 不作为 CI 门槛）。
   - `kim-core` 依赖 `kim-protocol` 的分层问题：在 architecture.md 加一句「ChannelId 来自 protocol，core 只为签名依赖；后续可抽 kim-ids」。不在本刀抽 crate。

## 明确不做

- 不 native async fn 全量替换
- 不 Chat 泛型 MessageStore
- 不 ServerBuilder typestate
- 不 edition 2024
- 不 panic=abort
- 不 missing_docs deny

## 验收

`cargo fmt && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace --offline`

deny.toml 语法正确即可，不强制安装 cargo-deny。
