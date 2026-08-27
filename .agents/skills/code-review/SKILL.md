---
name: code-review
description: >
  Code review combining language strictness rules, security auditing, and performance
  analysis. Use when a user says /code-review or asks to review a branch, PR, or set of
  changes. Auto-detects languages and applies the relevant rule sets from
  typescript-strict, rust-strict, swift-strict, go-strict, javascript-strict,
  security-audit-standard, performance-audit-standard, and github-standards.
---

# Code Review

Run a structured review of branch changes covering security, language strictness, performance, and conventions.

## Workflow

### Step 1: Detect changes

```bash
# Default base branch is main. User can override: /code-review --base staging
BASE=${1:-main}
git fetch origin "$BASE" --quiet 2>/dev/null || true
MERGE_BASE=$(git merge-base "origin/$BASE" HEAD)
BRANCH=$(git rev-parse --abbrev-ref HEAD)

# Changed files and stats
git diff --name-status "$MERGE_BASE"..HEAD
git diff --stat "$MERGE_BASE"..HEAD
git log --oneline "$MERGE_BASE"..HEAD
```

Use `$MERGE_BASE..HEAD` for ALL diffs. Never use `origin/main..HEAD` directly -- it includes changes from other merged branches.

### Step 2: Auto-detect languages

Map file extensions to rule sets:

| Extension | Rule set | Skill |
|---|---|---|
| `.ts`, `.tsx` | TypeScript | `typescript-strict` |
| `.rs` | Rust | `rust-strict` |
| `.swift` | Swift | `swift-strict` |
| `.go` | Go | `go-strict` |
| `.js`, `.mjs`, `.cjs` | JavaScript | `javascript-strict` |

Only apply rules for languages present in the diff.

### Step 3: Run checks

Run in this order. For each finding, record file, line number, description, and rule ID.

#### 3a. Security scan (security-audit-standard)

Scan the diff output (not the full repo) for:

- **Secrets**: API keys, tokens, passwords, connection strings (`sk-`, `ghp_`, `AKIA`, `-----BEGIN PRIVATE KEY`)
- **Input validation**: unvalidated user input reaching SQL, HTML, shell commands, file paths
- **Injection risks**: string concatenation in queries, `eval()`, `innerHTML`, unsanitized template literals
- **Auth issues**: missing auth checks on endpoints, weak JWT config, secrets in client-side code
- **File patterns**: `.env`, `*.pem`, `credentials.json` committed

#### 3b. Language strictness checks

Apply the top rules for each detected language (see Quick Reference Tables below). Read the changed lines and flag violations.

#### 3c. Performance scan (performance-audit-standard)

Look for these anti-patterns in the diff:

- O(n) membership checks where Set/Map should be used
- O(n^2) nested loops (`.filter()` inside `.map()`, `includes()` inside loop)
- Sort to find min/max (O(n log n) vs O(n) single pass)
- Sync file I/O in async context (`writeFileSync`, `std::fs::write` under async)
- Lock held across `.await` or disk I/O (Rust)
- Per-request client/regex creation instead of singleton
- Unbounded caches without capacity or TTL
- Multiple array passes where a single pass works

#### 3d. Style and conventions (github-standards)

- Commit messages follow `type(scope): subject` format
- No `console.log`/`print`/`println!` debugging left in code
- No commented-out code blocks
- No TODO/FIXME without a ticket reference
- Error handling present (no empty catch blocks, no swallowed errors)
- Naming conventions match language idioms

### Step 4: Generate review summary

## Output Format

```
## Code Review: [branch-name]
**Base**: [base-branch] | **Files changed**: N | **Commits**: N

### CRITICAL (must fix before merge)
- [file:line] Finding description (RULE-ID)

### HIGH (should fix)
- [file:line] Finding description (RULE-ID)

### MEDIUM (consider fixing)
- [file:line] Finding description (RULE-ID)

### LOW (nitpick)
- [file:line] Finding description (RULE-ID)

### Passed checks
- No secrets detected in diff
- Error handling present
- Input validated at boundaries
- Async operations properly awaited
- Resources cleaned up
- Commit messages follow conventions
```

Omit empty severity sections. Always show "Passed checks" to confirm what was verified.

## Quick Reference: Top 5 Rules Per Language

### TypeScript

| ID | Rule | What to flag |
|---|---|---|
| TS-01 | No `any` | `any` type annotation without justification comment |
| TS-02 | No `as` assertions | `as Type` without preceding type guard |
| TS-04 | No `@ts-ignore` | Use `@ts-expect-error` with explanation instead |
| TS-12 | Narrow catch errors | `catch (err)` without `instanceof Error` check |
| TS-13 | No silent catch | Empty `catch {}` or catch without logging |

### Rust

| ID | Rule | What to flag |
|---|---|---|
| -- | No `.unwrap()` | `.unwrap()` in non-test code without `// BUG IF:` comment |
| -- | No `.expect()` | `.expect()` outside static init (`LazyLock`, `OnceLock`) |
| -- | SAFETY comments | `unsafe` block without `// SAFETY:` comment |
| -- | Lock across await | `RwLock`/`Mutex` guard held across `.await` |
| -- | Bounded caches | `HashMap` used as cache without capacity limit |

### Swift

| ID | Rule | What to flag |
|---|---|---|
| SW-01 | No force unwrap | `!` on optionals in production code |
| SW-03 | No silent `try?` | `try?` on critical operations (file creation, auth, data save) |
| SW-09 | `@MainActor` on VMs | ViewModel missing `@MainActor` annotation |
| SW-12 | Task cancellation | `Task { }` without `[weak self]` or cancellation check |
| SW-15 | `[weak self]` | Async closures with strong `self` capture |

### Go

| ID | Rule | What to flag |
|---|---|---|
| GO-01 | Wrap errors | `return err` without `fmt.Errorf("context: %w", err)` |
| GO-02 | Check errors | Ignored error return value |
| GO-03 | Drain body | `resp.Body.Close()` without `io.Copy(io.Discard, ...)` on error paths |
| GO-07 | RWMutex | Write lock where read lock suffices, or missing double-check |
| GO-17 | No hardcoded secrets | `const apiKey = "..."` or similar |

### JavaScript

| ID | Rule | What to flag |
|---|---|---|
| JS-01 | No `var` | `var` declaration anywhere |
| JS-03 | No silent catch | Empty catch block or catch without action |
| JS-05 | Bounded retries | Recursive retry without max attempt limit |
| JS-08 | Async file I/O | `writeFileSync`, `readFileSync` in server code |
| JS-17 | No `eval()` | `eval()` or `new Function()` with dynamic input |

## Common Anti-Patterns Checklist

Quick-scan checklist applied to ALL languages:

- [ ] No hardcoded secrets or API keys in diff
- [ ] All errors handled (not swallowed with empty catch)
- [ ] No `console.log`/`print`/`println!`/`NSLog` debugging left
- [ ] No commented-out code blocks
- [ ] No TODO/FIXME without ticket reference
- [ ] Input validated at system boundaries
- [ ] No force-unwrap / force-cast / `any` type escape
- [ ] Async operations properly awaited (no fire-and-forget)
- [ ] Resources cleaned up (listeners, subscriptions, file handles, DB connections)
- [ ] No unbounded collections in hot paths (use bounded cache, Set, or Map)

## Branch Comparison Options

### Review against a different base branch

```
/code-review --base staging
/code-review --base develop
```

Override the default `main` base branch.

### Review specific files only

```
/code-review --files src/auth.ts src/middleware.ts
```

Limits the review to listed files. Use `git diff $MERGE_BASE..HEAD -- <file>` per file.

### Review a PR by number

```bash
# Fetch PR metadata and diff
gh pr view <NUMBER> --json title,body,headRefName,baseRefName,files
gh pr diff <NUMBER>
```

Use `gh pr diff` output as the diff source instead of `git diff`.

### Review a specific commit range

```
/code-review --range abc123..def456
```

Uses the provided range instead of computing merge-base.
