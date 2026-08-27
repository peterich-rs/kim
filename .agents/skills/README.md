# Project skills

Grok loads these from `.agents/skills/` (repo scope). Slash: `/<name>`.

| Skill | Use when |
|-------|----------|
| `rust-skills` | Writing or reviewing Rust (265 rules; open `rules/` as needed) |
| `rust-strict` | Unwrap/unsafe/secret/lock-across-await audit |
| `rust-async-patterns` | Tokio tasks, channels, `select!`, cancellation |
| `rust-tokio-net` | TCP / WebSocket framing, Conn, backpressure |
| `rust-distributed` | Raft, 2PC, consistency, distributed locks |
| `rust-database` | SQLx / Diesel / SeaORM, transactions, migrations |
| `rust-cache` | Redis, TTL, Cache-Aside, invalidation |
| `postgres-strict` | Postgres schema, RLS, migration safety |
| `postgresql-table-design` | Types, indexes, constraints, partitioning |
| `sql-optimization-patterns` | EXPLAIN, slow queries, N+1 |
| `git-commit` | Conventional Commits + confirm before `git commit` |
| `git-advanced-workflows` | rebase, bisect, worktree, reflog |
| `github-standards` | Branch/PR format, secrets scan, CI hygiene |
| `code-review` | Structured review of a branch or PR |

## Sources (MIT)

| Skill | Upstream |
|-------|----------|
| `rust-skills` | [leonardomso/rust-skills](https://github.com/leonardomso/rust-skills) |
| `rust-strict`, `postgres-strict`, `github-standards`, `code-review` | [0xMassi/claude-skills](https://github.com/0xMassi/claude-skills) |
| `rust-async-patterns`, `postgresql-table-design`, `sql-optimization-patterns`, `git-advanced-workflows` | [wshobson/agents](https://github.com/wshobson/agents) |
| `rust-database`, `rust-cache`, `rust-distributed` | [huiali/rust-skills](https://github.com/huiali/rust-skills) |
| `git-commit`, `rust-tokio-net` | Local; `git-commit` adapted from 0xMassi (Zapier/Jira MCP removed) |

Each copied skill keeps its upstream `LICENSE`.
