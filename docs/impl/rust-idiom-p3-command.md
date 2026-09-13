# 第 3 刀：Command 枚举（最小可编译）

父规格：[rust-idiom-upgrade.md](./rust-idiom-upgrade.md)。在第 1–2 刀之后继续。

本刀**不做**完整 Plug 流水线（talk 长函数拆 Plug 放到本刀若时间不够可只做枚举）。优先：

## 必须

1. `kim-protocol` 增加 `enum Command` 覆盖全部 `CMD_*` 常量（约 40 个）。`Command::parse(&str) -> Result<Self, ProtocolError>`（未知 → 新变体 `UnknownCommand` 或让 Router 继续 CommandNotFound）。
2. `Command` 实现 `as_str()` 回到线格式字符串。
3. `Router` 内部可仍用 HashMap<String,_> **或** HashMap<Command,_>。推荐 `handle` 接受 `Command`（`router.handle(Command::UserTalk, do_user_talk)`），lookup 用 `Command::parse(&packet.header.command)`。未知 command 保持 `Status::CommandNotFound`。
4. `CMD_*` 常量保留给编解码/客户端；Chat 注册处改用枚举。

## 可选（有时间再做）

把 `do_user_talk` / `do_group_talk` 抽 `ParseDest` / `SocialGuard` 两个函数（不是完整 Plug trait）。不要重写整个 talk.rs。

## 不做

- 不改 protobuf Header.command 类型（仍是 string）
- 不改 ACK
- 不拆 MessageStore
- 不用 PlugResult::NewPipe

## 验收

`cargo fmt && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace --offline`
