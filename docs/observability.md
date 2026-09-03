# 可观测（已落地）

对照小册第 31 章。`crates/kim-metrics` 只被 examples path-dep。不要加进 `kim-core` / `kim-tcp` / `kim-container`。

每个进程一个 `prometheus::Registry`（不是全局）。

| 进程 | 端口 |
|---|---|
| WGateway | `127.0.0.1:9001` |
| Chat | `127.0.0.1:9002` |
| TGateway | `127.0.0.1:9003` |
| Router | 复用 `:8088`（`.merge(kim_metrics::router)`，不第二绑定） |

指标：`kim_channel_total`、`kim_message_in_total`、`kim_message_in_flow_bytes`、`kim_message_out_flow_bytes`、`kim_no_server_found_error_total`（仅当 forward 错误正好是 `no adult instances`）、`kim_login_total`、`kim_handler_duration_seconds`（`COMMANDS` 29 条，其它 → `other`）、`kim_talk_total`、`kim_dispatch_fail_total`（talk 已落库但在线 Push 未完成：dispatch Err、locations 非 NotFound、空 recipients、push 预算耗尽；离线 NotFound 不加）、`kim_session_not_found_total`、`kim_heartbeat_revoke_error_total`（网关心跳吊销查询存储/传输错误；连续 3 次后断开，期内连接保持且不续签 JWT）、`kim_send_to_ack_seconds`（pending_delivery `created_at` → `acked_at`）、`kim_mailbox_full_total`。无 Royal RPC 直方图、无告警规则（见 [production-gaps.md](production-gaps.md) G-15）。

`metrics_listen` 为空则不起 metrics HTTP。CI 不跑 Prometheus 二进制。测试 scrape 文本。
