---
name: git-commit
description: >
  Create Conventional Commits from the current diff, then commit with user
  confirmation. Use when the user says commit, git commit, write a commit
  message, or asks for /git-commit or /commit. Complements github-standards
  for PR/branch rules.
license: MIT
metadata:
  author: kim
  adapted-from: https://github.com/0xMassi/claude-skills
---

# Git Commit

Write a Conventional Commits message that future readers can use to reconstruct *why* the change landed. Commit only after the user confirms the message.

## Hard rules

- Never add `Co-Authored-By`, `Generated-by`, or any mention of an AI tool.
- Never run `git add -A` or `git add .`. Stage only files that belong in this commit.
- Never `git push` unless the user explicitly asked to push.
- Do not commit secrets (`.env`, keys, tokens). Scan the staged diff first.
- Prefer one logical change per commit. Split unrelated edits.

## Workflow

### 1. Gather context

```bash
git status
git diff
git diff --staged
git log --oneline -10
git branch --show-current
```

If nothing is staged, inspect the unstaged diff and stage the relevant files by path. Leave generated noise (`target/`, `.DS_Store`) unstaged.

### 2. Infer why from the diff

Read the diff and recent commits. Infer the motivation from the code change; only ask the user if the why is ambiguous (bug fix vs refactor vs accidental leftover).

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

Examples: `core`, `tcp`, `ws`, `protocol`, `naming`, `container`, `echo`, `gateway`, `deps`.

Omit scope only for repo-wide chores (`chore: bump edition`).

### 4. Write the message

Subject: imperative, no period, ≤72 characters.

```
type(scope): concise subject

Why this change was needed:
[1-3 sentences]

What changed:
- [bullet]

Problem solved:
[observable result]
```

Trivial one-liners are OK for tiny changes:

```
fix(tcp): reject frames longer than max payload
docs: note ws-echo client args
chore(deps): bump thiserror to 2
```

Use a heredoc so the body keeps newlines:

```bash
git commit -m "$(cat <<'EOF'
type(scope): subject

Why this change was needed:
...

What changed:
- ...

Problem solved:
...
EOF
)"
```

### 5. Confirm, then commit

Show the user the full message and the file list. Commit only after they approve. Do not amend published history unless they asked.

## Examples

```
feat(ws): accept HTTP Upgrade and speak RFC6455 frames

Why this change was needed:
Browser clients cannot speak the length-prefixed TCP codec. The gateway
needs the same Conn/Channel abstraction over WebSocket.

What changed:
- Added kim-ws Conn wrapping fastwebsockets after hyper upgrade
- Wired EchoHandler through the same ChannelMap as TCP

Problem solved:
ws-echo-client can round-trip payloads through the existing handler.
```

```
fix(core): drop Channel write lock before awaiting the mailbox

Why this change was needed:
Holding the ChannelMap lock across the mpsc send stalled other pushes
when a slow writer filled the mailbox.

What changed:
- Clone the Channel handle, release the map lock, then send

Problem solved:
Push no longer serializes unrelated channels behind one lock.
```
