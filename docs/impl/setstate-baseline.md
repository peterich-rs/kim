# setState 基线

规则：`avoid_set_state`（`sdk/mobile/packages/kim_lints`），severity `error`。`lib/design/**` 放行。`lib/features/**` 禁止业务 `setState`。文件头 `// kim_lint: allow_ephemeral_set_state` 仅用于焦点、滚动、动画。

| 文件 | 次数 | 处置 |
|---|---:|---|
| `features/agent/agent_create_page.dart` | 0 | `agentCreateFormProvider` |
| `features/agent/agent_settings_page.dart` | 0 | `agentSettingsFormProvider` |
| `features/agent/provider_account_page.dart` | 0 | `providerAccountFormProvider` |
| `features/agent/agent_capabilities_page.dart` | 0 | `agentCapabilitiesFormProvider` |
| `features/settings/dev_panel.dart` | 0 | `devPanelProvider` |
| `features/contacts/peer_profile_page.dart` | 0 | `peerProfileProvider` |
| `features/contacts/contacts_page.dart` | 0 | `contactsSearchUiProvider` |
| `features/auth/auth_page.dart` | 0 | `authDraftProvider` |
| `features/agent/agent_runtime_switch.dart` | 0 | `runtimeSwitchBusyProvider` |
| `features/agent/agent_plaza_page.dart` | 0 | `plazaUiProvider` |
| `features/chats/chats_page.dart` | 0 | `chatsSearchUiProvider` |
| `features/auth/password_page.dart` | 0 | `passwordDraftProvider` |

`lib/design/**` 不计入：composer、image viewer、chat list、pet、bubble、text field。

`sdk/mobile/tools/check_page_size.sh` 在页面仍超过 400 行时不接入 CI。
