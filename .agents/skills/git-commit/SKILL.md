---
name: git-commit
description: >
  Create Conventional Commits from the staged diff, then commit with user
  confirmation. Use when the user says commit, git commit, write a commit
  message, or asks for /git-commit or /commit. Complements github-standards
  for PR/branch rules.
license: MIT
metadata:
  author: kim
  adapted-from: https://github.com/0xMassi/claude-skills
---

# Git Commit

Write a Conventional Commits message that names the **landed code**. The staged
diff is the only source of truth. Commit only after the user confirms the
message and the file list.

## Hard rules

- Never add `Co-Authored-By`, `Generated-by`, or any mention of an AI tool.
- Never run `git add -A` or `git add .`. Stage only files that belong in this commit.
- Never `git push` unless the user explicitly asked to push.
- Do not commit secrets (`.env`, keys, tokens). Scan the staged diff first.
- Message describes the resulting code only. Drop session, plan, and journey text.

## Atomic grain

Each commit is one independently reviewable, independently revertable unit that
still builds.

**Too coarse** (split):

- Unrelated outcomes joined by "and" in the subject
- Feature mixed with an unrelated fix or refactor
- Behavior mixed with formatting-only edits
- Several crates changed for different reasons

**Too fine** (combine):

- One file per commit when those files are a single behavior
- Implementation split from the tests that lock that same behavior
- A slice that does not compile or pass on its own
- Drive-by import/rename leftovers from the same change

Default: implementation + its tests + docs that only describe that same code
change. If a leftover would still make sense as its own revert, it is a second
commit.

## Workflow

### 1. Gather context

```bash
git status
git diff
git diff --staged
git log --oneline -10
git branch --show-current
```

If nothing is staged, inspect the unstaged diff and stage the relevant files by
path. Leave generated noise (`target/`, `.DS_Store`) unstaged.

### 2. Read the landed code

Read the staged diff (and unstaged files you intend to include). Infer type and
subject from **what the tree is after this commit**, not from the conversation
or the path taken to get there.

Ask the user only when the diff itself is ambiguous (bug fix vs refactor vs
accidental leftover).

### 3. Pick type and scope

**Types**

| Type | When |
|------|------|
| `feat` | user-visible capability |
| `fix` | bug fix |
| `docs` | documentation only |
| `refactor` | restructure, no behavior change |
| `test` | tests only |
| `perf` | measured performance |
| `chore` | deps, build, lint, tooling |
| `ci` | CI config |
| `style` | formatting only |

**Scope** (this repo): crate or area, lowercase.

Examples: `core`, `tcp`, `ws`, `protocol`, `naming`, `container`, `echo`,
`gateway`, `deps`.

Omit scope only for repo-wide chores (`chore: bump edition`).

### 4. Write the message

Subject: imperative, no period, ≤72 characters. It states the code change.

Body: omit when the subject already names the landed code. When the subject
cannot hold the code-level facts, add a blank line then `-` bullets, one fact
per line. No headings. No paragraphs.

```
type(scope): concise subject

- code-level fact
- code-level fact
```

Body bullets are extra facts about the **resulting code** that a later reader
cannot get from the subject alone (constraint now enforced, API/compat note,
which existing type is reused). They are not a recap of the diff, not a file
list, and not a diary of how the code was written.

**Keep out of the message** (subject, body, footer):

- Implementation journey: tried X, then Y; after debugging; intermediate approaches
- Session text: as requested, per discussion, next we will, TODO, WIP plan
- Alternatives that did not land
- Agent/tool attribution
- `Why this change was needed` / `What changed` / `Problem solved` headings

Trivial one-liners are the default for small diffs:

```
fix(tcp): reject frames longer than max payload
docs: note ws-echo client args
chore(deps): bump thiserror to 2
```

Use a heredoc so the body keeps newlines:

```bash
git commit -m "$(cat <<'EOF'
type(scope): subject

- code-level fact
- code-level fact
EOF
)"
```

Optional footer, only if it already exists in the repo: `Fixes #N`. Do not
invent ticket IDs.

### 5. Confirm, then commit

Show the user the full message and the file list. Commit only after they
approve. Do not amend published history unless they asked.

## Examples

Good — subject names the code; body adds constraints the subject cannot hold:

```
feat(ws): accept HTTP Upgrade and speak RFC6455 frames

- wrap fastwebsockets Conn after hyper upgrade
- reuse TCP ChannelMap and EchoHandler
```

```
fix(core): drop Channel write lock before mailbox send

- clone Channel, release ChannelMap lock, then send
```

Good — subject is enough:

```
fix(tcp): reject frames longer than max payload
chore(deps): bump thiserror to 2
```

Bad — headings, journey, recap of the diff:

```
feat(ws): add websocket support

Why this change was needed:
The user asked for browser clients after we tried exposing TCP.

What changed:
- Added several files under crates/kim-ws
- Reworked the upgrade path twice

Problem solved:
ws-echo-client now works.
```
