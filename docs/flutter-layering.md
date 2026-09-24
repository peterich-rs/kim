# Flutter 分层与 Riverpod

依据：[Consumers](https://riverpod.dev/docs/concepts2/consumers)、[Refs](https://riverpod.dev/docs/concepts2/refs)、[select](https://riverpod.dev/docs/how_to/select)。状态以 Riverpod 为核心。`setState` 只允许 `lib/design/**` 的瞬时 UI，或文件头标记 `// kim_lint: allow_ephemeral_set_state` 的焦点/动画。

## 三种 Consumer

| API | 何时 |
|---|---|
| `ConsumerWidget` | 默认可复用区块 |
| `ConsumerStatefulWidget` | 仅 `FocusNode` / `TextEditingController` / `AnimationController` |
| `Consumer` + `child` | 同文件一次性局部订阅；`child` 冻结不变子树 |

谁 `ref.watch`，谁重建。组装壳少 watch。禁止用 `ref.read` 假装优化。

## 细粒度

1. 拆 `ConsumerWidget`（主）
2. `Provider` / `family` 派生投影（如 `peerTypingProvider`、`chatChromeProvider`）
3. `select` 字段。返回值不可变。`selectAsync` 只在 async provider 内 `await`，不要写进 `build`。

| API | 位置 |
|---|---|
| `ref.watch` | `build` / `Notifier.build` |
| `ref.read` | 点击与异步回调 |
| `ref.listen` | `build` 里的 toast / 跳转 |
| `ref.listenManual` | `initState` |

## 状态放哪

| 类别 | 机制 |
|---|---|
| 域状态 | `Notifier` / `AsyncNotifier` |
| 向导草稿 | autoDispose `Notifier`（`agentCreateFormProvider`） |
| 局部投影 | `family` + 可选 `select` |
| 文本输入 | `TextEditingController` 留在 State；提交或离焦再写入 notifier |
| 业务字段 | 禁止 `setState` 改 `_step` / `_accountId` / 搜索 query |

## 聊天页

`ChatPage` 只 `listen` toast / redirect。`ChatMessageList`、`ChatTypingHost`、`ChatPermissionStrip`、`ChatHeader`、`ChatComposerBar` 各自订阅。Chrome 用 `chatChromeProvider`，子组件 `select` 字段。

## 表单

`AgentCreatePage` `select((d) => d.step)`。各 step 再 `select` 自己的字段。

## Lint

`sdk/mobile/packages/kim_lints` 规则 `avoid_set_state`（`custom_lint`，severity `error`）。`lib/features/**` 零 `ignore_for_file`。`riverpod_lint` 3.1 与 `flutter_riverpod` 3.4 / `custom_lint` 0.8 的 analyzer 约束冲突，暂不启用，等上游对齐后再加。

```bash
cd sdk/mobile && dart run custom_lint
cd sdk/mobile && flutter analyze
```

页面软上限：`features/**/*_page.dart` 超过 400 行应继续拆。`tools/check_page_size.sh`。
