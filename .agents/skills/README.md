# Project skills

Grok loads these from `.agents/skills/` (repo scope). Slash: `/<name>`.

| Skill | Use when |
|-------|----------|
| `rust-best-practice` | 写或审 Rust：先做设计判断（所有权、类型、trait、错误、并发、长连接），再按需查 `rules/` 和安全约束 |
| `postgres-strict` | Postgres schema, RLS, migration safety |
| `postgresql-table-design` | Types, indexes, constraints, partitioning |
| `sql-optimization-patterns` | EXPLAIN, slow queries, N+1 |
| `git-commit` | Conventional Commits + confirm before `git commit` |
| `git-advanced-workflows` | rebase, bisect, worktree, reflog |
| `github-standards` | Branch/PR format, secrets scan, CI hygiene |
| `code-review` | Structured review of a branch or PR |
| `ascii-diagram` | Any logic spanning 2+ hops / 3+ participants: sequence, call-chain, dataflow, pipeline, layered, state diagrams -- diagram first, prose second |
| `docs-maintenance` | 整理 `docs/`：长期合同留在根上，`docs/impl/` 只放未合入切片，合入时删切片稿 |

Official Dart (`dart-*`) and Flutter (`flutter-*`) skills: slash `/<name>`, e.g. `/flutter-add-widget-test`. Update with `npx skills update`.

### Dart (dart-lang/skills)

| Skill | Use when |
|-------|----------|
| `dart-add-unit-test` | Unit tests with `package:test` |
| `dart-build-cli-app` | CLI entrypoint, exit codes, scripts |
| `dart-collect-coverage` | Coverage + LCOV |
| `dart-fix-runtime-errors` | Stack traces, LSP, hot reload |
| `dart-generate-test-mocks` | Mockito + build_runner |
| `dart-migrate-to-checks-package` | `expect` → `package:checks` |
| `dart-resolve-package-conflicts` | `pub get` version conflicts |
| `dart-run-static-analysis` | `dart analyze` / `dart fix` |
| `dart-setup-ffi-assets` | Native assets, C/C++ hooks |
| `dart-use-doc-examples` | `{@example}` in dartdoc |
| `dart-use-ffigen` | Generate FFI bindings |
| `dart-use-pattern-matching` | Switch expressions / patterns |
| `dart-use-primary-constructors` | Dart primary constructors |
| `dart-write-documentation` | Effective Dart `///` docs |

### Flutter (flutter/agent-plugins)

| Skill | Use when |
|-------|----------|
| `flutter-add-integration-test` | `integration_test` / driver flows |
| `flutter-add-widget-preview` | Widget previews |
| `flutter-add-widget-test` | WidgetTester UI tests |
| `flutter-apply-architecture-best-practices` | UI / Logic / Data layers |
| `flutter-build-responsive-layout` | Phone vs tablet/desktop layout |
| `flutter-fix-layout-issues` | Overflow / unbounded constraints |
| `flutter-implement-json-serialization` | `fromJson` / `toJson` |
| `flutter-setup-declarative-routing` | `go_router` |
| `flutter-setup-localization` | l10n / intl |
| `flutter-use-http-package` | REST with `package:http` |

## Sources

| Skill | Upstream |
|-------|----------|
| `rust-best-practice` | Design text and `references/tokio-net.md` local; `rules/` from [leonardomso/rust-skills](https://github.com/leonardomso/rust-skills); `references/strict.md` from [0xMassi/claude-skills](https://github.com/0xMassi/claude-skills) |
| `postgres-strict`, `github-standards`, `code-review` | [0xMassi/claude-skills](https://github.com/0xMassi/claude-skills) |
| `postgresql-table-design`, `sql-optimization-patterns`, `git-advanced-workflows` | [wshobson/agents](https://github.com/wshobson/agents) |
| `git-commit` | Local; adapted from 0xMassi (Zapier/Jira MCP removed) |
| `docs-maintenance` | Local |
| `dart-*` | [dart-lang/skills](https://github.com/dart-lang/skills) (BSD-3-Clause) |
| `flutter-*` | [flutter/agent-plugins](https://github.com/flutter/agent-plugins) (BSD-3-Clause) |

Each copied skill keeps its upstream `LICENSE`. Lockfile: repo-root `skills-lock.json` (`npx skills update`).
