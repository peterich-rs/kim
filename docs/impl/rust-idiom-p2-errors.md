# 第 2 刀：错误收口

父规格：[rust-idiom-upgrade.md](./rust-idiom-upgrade.md)。在第 1 刀之后的 worktree 上继续。

## 范围（必须整仓编译）

1. workspace 依赖加 `anyhow`。五个服务 `main` / `load_config` / `load_tls` / `open_*` 改 `anyhow::Result`，用 `.context("...")`。examples 可一起改。
2. `HandlerFn` 改为 `Future<Output = Result<(), RouterError>>`。`Router::serve` 传播 handler 错误。`do_*` 仍可在内部 `resp` 后 `return Ok(())`；把现在的 `warn`+吞掉改成 `?` 或 `return Err`。最小改动：handler 闭包末尾 `Ok(())`，已有 `ctx.resp` 失败处 `return Err(err)`。
3. `MessageListener::receive` → `Result<(), Error>`。Chat/Gateway 实现把内部失败变成 `Ok(())` 并 warn **或** `Err`。读循环对 `Err` 记 debug/warn，**不要**默认拆连接（行为变化太大）；先传播到现有 read_loop 的 error 路径（若 receive 失败则当该帧处理失败，继续读）。
4. `SessionError`：Location decode 的截断/UTF-8 改成独立变体 `Truncated` / `InvalidUtf8`，不再 `Other("truncated location")`。测试 `matches!(..., SessionError::Other(_))` 改为新变体。
5. `Naming::Error`：至少拆 `Http { status: u16 }` / `Transport(String)` / `FeatureDisabled`，consul.rs 对号入座。不必一次删光所有 `kim_core::Error::Other`（第 2 刀后半可留 `Other` 但新代码禁止新增；本刀重点是应用 anyhow + handler Result + Location/Naming 类型化）。

## 不做

- 不 `unwrap_used = deny`（测试 unwrap 太多，单独一刀）。
- 不改 ACK、线格式、ID newtype。
- 不把 `StoreError::Backend` 一次拆完（Royal/sqlx 映射面太大）；可加 `Timeout` 若已有字面量 `"list_locations timeout"`。
- 不改 Dart 错误字符串匹配。

## 验收

```
cargo fmt
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --offline
```
