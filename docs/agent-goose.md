# Local Goose agent in KIM

Goose runs on the **desktop IM** (macOS / Windows / Linux). iOS and Android
are IM clients only: they do not compile or ship `kim_agent_ffi`. Mobile and
Web can view and send in a registered 1:1; only desktop runs Goose.

KIM does not ship a custom agent loop. Desktop IM talks to a **local Goose host**:

- Loop: [`goose-agent`](https://crates.io/crates/goose-agent) state machine, assembled per `AgentProfile` by `MachineFactory`
- Providers: [`goose-providers`](https://crates.io/crates/goose-providers) (OpenAI Completions/Responses, Anthropic Messages)
- Host: `crates/kim-agent-host` (profile, session map, @mention helper, system prompt)
- FFI: `sdk/mobile/rust_agent` (`kim_agent_ffi`) — still isolated from `kim_client_ffi`
- Dest: default persona id is `goose` (alias of `agent:goose`). Extra personas use `agent:<profile_id>` as the **local** dest / mention key. After `chat.bot.create`, the IM dest is the server-assigned `serverAccount` (`b_…`).

The client flag `agent.server_identity` **defaults false**. When off, 1:1 stays
on-device (`dest=goose` / `agent:<id>`, `_appendLocal`, no WGateway). When on
and logged in, registered 1:1 user↔agent traffic goes through WGateway
(`chat.user.talk` / `chat.bot.reply`); group `@mention` stays local.

## Product

1. Open **我 → Agent 设置**, pick OpenAI or Anthropic, save an API key.
2. **助手** is a contact (通讯录) and a session in 消息. With the flag off it is a local-only dest. With the flag on, a new profile registers immediately; an existing profile (including 助手) registers on first 1:1 open.
3. Tap it and chat like a friend. Registered 1:1 messages are stored on the server so phone / web can read and send. Goose still runs only on this desktop; the owner session posts `chat.bot.reply`.
4. In a human chat you can still `@助手` / `@goose` to pull the same local agent in. Those replies stay on this device.

## Layout

```
Flutter composer  --talk-->  kim_client_ffi (IM / WGateway)
                 --@助手 / unregistered dest=goose-->  kim_agent_ffi
                      --> kim-agent-host
                           AgentProfile → MachineFactory → Goose
                 --registered 1:1 TalkResp-->  kim_agent_ffi
                      --> assistant_finished → chat.bot.reply
```

Dart orchestrates the two FFIs. Do not merge IM and agent Rust clients. `kim_agent_ffi` must not depend on `kim-client`.

## Build / test

```bash
cargo test -p kim-agent-host
cd sdk/mobile && flutter test test/agent_mention_test.dart test/agent_settings_test.dart test/state/chat_agent_test.dart
```
