# Local Goose agent in KIM

KIM does not ship a custom agent loop. Desktop IM talks to a **local Goose host**:

- Loop: [`goose-agent`](https://crates.io/crates/goose-agent) state machine, assembled per `AgentProfile` by `MachineFactory`
- Providers: [`goose-providers`](https://crates.io/crates/goose-providers) (OpenAI Completions/Responses, Anthropic Messages)
- Host: `crates/kim-agent-host` (profile, session map, @mention helper, system prompt)
- FFI: `sdk/mobile/rust_agent` (`kim_agent_ffi`) — still isolated from `kim_client_ffi`
- Dest: default persona is `goose` (alias of `agent:goose`). Extra personas use `agent:<profile_id>` (later).

## Product

1. Open **我 → Agent 设置**, pick OpenAI or Anthropic, save an API key.
2. **助手** is a local contact (通讯录 → 本地 Agent) and a session in 消息.
3. Tap it and chat like a friend. Messages stay on this device; they are not sent to WGateway.
4. In a human chat you can still `@助手` / `@goose` to pull the same local agent in.

## Layout

```
Flutter composer  --talk-->  kim_client_ffi (IM)
                 --@助手 / dest=goose-->  kim_agent_ffi
                      --> kim-agent-host
                           AgentProfile → MachineFactory → Goose
```

Dart orchestrates the two FFIs. Do not merge IM and agent Rust clients. `kim_agent_ffi` must not depend on `kim-client`.

## Build / test

```bash
cargo test -p kim-agent-host
cd sdk/mobile && flutter test test/agent_mention_test.dart test/agent_settings_test.dart
```
