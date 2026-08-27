---
name: rust-database
description: >
  Rust database access with SQLx, Diesel, and SeaORM. Use when writing queries,
  designing repositories, handling transactions, running migrations, tuning
  connection pools, or adding Postgres/MySQL/SQLite to a Rust service. Invoke
  with /rust-database.
license: MIT
metadata:
  author: huiali
  source: https://github.com/huiali/rust-skills
---

# Rust Database Skill

## Core Question

**How do we guarantee data correctness while keeping queries and migrations operationally safe?**

## Persistence Architecture

- Keep repository/data-access layer separate from business logic.
- Define explicit transaction boundaries around business invariants.
- Choose stack by need:
  - `sqlx`: explicit SQL and compile-time query checks (default for new KIM storage).
  - `diesel`: typed query builder with strong schema coupling.
  - `sea-orm`: async ORM convenience with rapid CRUD iteration.

### SQLx defaults (async Tokio)

```toml
sqlx = { version = "0.8", default-features = false, features = ["runtime-tokio", "tls-rustls", "postgres", "macros", "chrono", "uuid"] }
```

- Share one `PgPool` (or `SqlitePool`) via `Arc`; do not open a connection per request.
- Set `max_connections`, `acquire_timeout`, and `idle_timeout` from config, not hardcoded defaults.
- Prefer `query_as!` / `query!` in CI with `SQLX_OFFLINE=true` after `cargo sqlx prepare`.
- Map DB errors with `thiserror` in library crates; do not `.unwrap()` pool or query results.

## Transaction and Consistency Rules

- Keep transactions short.
- Do not perform network calls inside transactions.
- Handle deadlock/serialization retries with bounded policy.

## Query Performance

- Use indexes based on real access patterns.
- Detect and remove N+1 query behavior.
- Inspect query plans for slow paths.

## Migration Safety

- Prefer additive migrations for rolling deployments.
- Separate destructive changes into phased rollouts.
- Verify backward/forward compatibility windows.

## Common Pitfalls

- Long-lived transactions causing lock contention.
- Schema changes incompatible with old app versions.
- Inconsistent timezone/nullability handling across layers.
- Pool exhaustion under burst traffic.

## Review Checklist

- [ ] Transaction boundaries align with domain invariants.
- [ ] Retry/timeout policy is explicit for DB operations.
- [ ] Migrations are safe for rolling deploy.
- [ ] Query plans and indexes are validated.
- [ ] Metrics cover pool usage, latency, and error rates.

## Verification Commands

```bash
cargo check
cargo test
cargo clippy
cargo sqlx prepare
cargo sqlx migrate run
```

## Related Skills

- `rust-cache` for Redis
- `postgres-strict` and `postgresql-table-design` for schema
- `sql-optimization-patterns` for EXPLAIN / indexes
