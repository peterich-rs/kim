---
name: github-standards
description: >
  GitHub repository standards and workflow rules. Use when creating repos, opening PRs,
  pushing code, setting up CI, creating branches, writing READMEs, or reviewing repo
  hygiene. Covers commit format (complements git-commit skill), PR message template,
  sensitive data pre-commit checks, repo naming, README requirements, branch naming,
  .gitignore baseline, release tagging, and CI/CD baseline. Derived from conventions
  across 22+ repositories.
---

# GitHub Standards

Repository and workflow conventions from 22+ production repositories.

## CRITICAL: Sensitive Data: Pre-Commit Checks

Before EVERY commit, scan staged files for secrets. Never commit these:

### Automatic scan patterns

```bash
# Run before committing: catches most leaks
git diff --cached --diff-filter=ACM | grep -iE \
  'sk[-_]live|sk[-_]test|api[-_]?key|secret[-_]?key|password\s*=|token\s*=|ghp_|gho_|github_pat_|ntn_|xoxb-|xoxp-|AKIA[A-Z0-9]{16}|-----BEGIN (RSA |EC )?PRIVATE KEY|mongodb\+srv://|postgres(ql)?://[^@]+@|redis://:[^@]+@'
```

### Files that MUST NEVER be committed

| Pattern | Why |
|---|---|
| `.env`, `.env.*` | API keys, DB URLs, secrets |
| `*.pem`, `*.key`, `*.p12` | Private keys, certificates |
| `credentials.json`, `auth*.json` | OAuth tokens, service accounts |
| `config.json` (if contains secrets) | Runtime config with keys |
| `*.keystore`, `*.jks` | Java/Android signing keys |
| `id_rsa`, `id_ed25519` | SSH private keys |
| `*.sqlite`, `*.db` (user data) | Local databases with PII |
| `sessions.json` | Login sessions, cookies |

### What to do if secrets ARE committed

```bash
# 1. Remove from tracking (doesn't remove from history)
git rm --cached .env
echo ".env" >> .gitignore
git commit -m "chore: remove tracked secrets, add to gitignore"

# 2. Rotate the exposed secret IMMEDIATELY
# Old key is in git history forever (unless you rewrite)

# 3. If the repo is public, consider history rewrite
git filter-branch --force --index-filter \
  'git rm --cached --ignore-unmatch .env' HEAD
# Or use BFG Repo-Cleaner (faster)
```

### Safe alternatives for secrets in code

| Instead of | Use |
|---|---|
| Hardcoded API key | `process.env.API_KEY` or `std::env::var("API_KEY")` |
| Hardcoded URL with token | `obfstr!()` macro (Rust) for compile-time obfuscation |
| Config file with secrets | `.env` file (gitignored) + `.env.example` (committed, no values) |
| Inline DB connection string | Environment variable + validation at startup |

## CRITICAL: Commit Message Format

**Complements the `/git-commit` skill**: these are the structural rules.

### Format

```
type(scope): concise subject line (imperative mood, no period)

Why this change was needed:
[1-3 sentences: business/technical motivation]

What changed:
[Bullet list of technical modifications]

Problem solved:
[What's different for the user/system after this commit]

Refs: TICKET-123
```

### Rules

1. **Subject line**: imperative mood ("add feature" not "added feature"), max 72 chars
2. **Type**: `feat`, `fix`, `docs`, `chore`, `refactor`, `ci`, `style`, `test`, `perf`
3. **Scope**: module or area affected (`auth`, `editor`, `gateway`, `deps`)
4. **Body**: wrap at 72 characters per line
5. **No co-authors**: Never add `Co-Authored-By` or mention AI tools
6. **No `git add -A` or `git add .`**: Stage specific files only
7. **Heredoc format**: Always use heredoc for multi-line messages

### Quick commits (small changes)

For trivial changes, a single-line message is acceptable:

```
fix(editor): correct off-by-one in line numbering
chore(deps): bump rollup in the npm_and_yarn group
docs: update API endpoint documentation
```

### Version bumps

```
chore: bump version to 0.4.0
```

## CRITICAL: Pull Request Format

### PR Title

```
type(scope): concise description
```

Same Conventional Commits format as commit messages. Under 72 characters.

### PR Body Template

Every repo should have `.github/PULL_REQUEST_TEMPLATE.md`:

```markdown
## Summary
<!-- What does this PR do? 1-3 bullet points. -->

## Related issue
<!-- Link the issue. E.g. "Closes #42" or "Refs: PROJ-123" -->

## Changes
<!-- Brief description of what changed and why. -->

## Test plan
- [ ] Type check passes (`tsc --noEmit` / `cargo check` / `swift build`)
- [ ] Linter passes (`clippy` / `eslint` / `swiftlint`)
- [ ] Existing tests pass
- [ ] New functionality manually tested
- [ ] Edge cases considered

## Screenshots
<!-- If visual change, add before/after. Delete section if not applicable. -->
```

### PR rules

1. **One concern per PR**: don't mix features with refactors
2. **Draft PRs** for work-in-progress: prefix title with `[WIP]` or use GitHub draft
3. **Self-review before requesting**: read your own diff first
4. **Link related issues**: `Closes #N` auto-closes on merge
5. **Small PRs preferred**: under 400 lines changed when possible
6. **No merge commits**: squash merge or rebase, keep history linear

## HIGH: Branch Naming

### Format

```
type/description-in-kebab-case
```

### Types

| Prefix | When |
|---|---|
| `feat/` or `feature/` | New feature |
| `fix/` | Bug fix |
| `refactor/` | Code restructuring |
| `chore/` | Maintenance, deps, config |
| `docs/` | Documentation |
| `ci/` | CI/CD changes |
| `release/` | Release preparation |
| `hotfix/` | Urgent production fix |

### Examples

```
feat/github-oauth-flow
fix/pty-output-encoding
refactor/extract-validation-module
chore/bump-dependencies
release/0.4.0
hotfix/auth-token-expiry
```

### Rules

1. **Lowercase only**, kebab-case
2. **Include ticket ID** if using issue tracker: `feat/PROJ-123-oauth-flow`
3. **Short but descriptive**: someone should understand what it's about
4. **Delete after merge**: don't accumulate stale branches
5. **`main`** is the primary branch (not `master`)

## HIGH: Repository Setup

### Required files for every repo

| File | Purpose | When to create |
|---|---|---|
| `README.md` | Project overview, setup, usage | At repo creation |
| `.gitignore` | Exclude build artifacts, secrets, OS files | At repo creation |
| `CLAUDE.md` | AI assistant context (see `/claude-md-template`) | At repo creation |
| `.env.example` | Template for required env vars (no values) | When first env var added |
| `LICENSE` | Usage terms | At repo creation (for public repos) |

### Repo naming

| Project type | Convention | Examples |
|---|---|---|
| Apps | `kebab-case` | `my-app`, `bot-rs` |
| Libraries/packages | `kebab-case` | `engine-rs`, `cli-tools` |
| APIs/services | `snake_case` or `kebab-case` | `service_api`, `auth-api` |
| iOS apps | `PascalCase` | `MyApp`, `Companion` |
| Landing pages | `kebab-case` or `snake_case` | `app_landing`, `product.io` |

Suffix with tech when ambiguous: `-rs` (Rust), `-api` (backend), `-app` (frontend).

### Repo description

Always set a GitHub description. Format:

```
[Emoji] One-line description of what it does and key tech
```

Examples:
- `AI coding workspace: Tauri v2, React, TypeScript`
- `iOS journaling app: SwiftUI, CloudKit`
- `Multi-protocol AI gateway: Rust, axum, OpenAI-compatible`

## HIGH: .gitignore Baseline

Every repo starts with this minimum, then adds language-specific patterns:

```gitignore
# OS
.DS_Store
Thumbs.db

# IDE
.vscode/
.idea/
*.swp
*.swo

# Secrets: NEVER COMMIT
.env
.env.*
!.env.example
*.pem
*.key
*.p12
credentials*.json
auth*.json
secrets*.json

# Build artifacts
dist/
build/
*.log
```

### Language-specific additions

**Node.js/TypeScript:**
```gitignore
node_modules/
*.local
.next/
.turbo/
```

**Rust:**
```gitignore
/target/
src-tauri/target/
Cargo.lock  # Only for libraries, not binaries
```

**Swift/iOS:**
```gitignore
xcuserdata/
*.xcuserdatad/
DerivedData/
Pods/
*.dSYM.zip
```

**Go:**
```gitignore
/bin/
/vendor/
*.exe
```

## HIGH: README Requirements

### Minimum structure

```markdown
# Project Name

One paragraph: what it is, who it's for, key technology.

## Setup

Exact commands to get running from a fresh clone.

## Development

Commands for daily development workflow.

## Architecture

Brief overview of how the codebase is organized.
```

### README update triggers

Update README when:
- [ ] New setup steps added (dependency, env var, tool)
- [ ] Project architecture changes (new module, service, API)
- [ ] Build/deploy process changes
- [ ] Minimum system requirements change

### Anti-patterns

- Don't put secrets or tokens in README
- Don't document aspirational features as existing
- Don't let README rot: if setup instructions are wrong, they're worse than none

## MEDIUM: Release & Tagging

### Semantic versioning

```
v{major}.{minor}.{patch}

major: breaking changes
minor: new features (backwards compatible)
patch: bug fixes
```

### Tag format

```bash
git tag -a v0.4.0 -m "Release v0.4.0: OAuth flow and PTY improvements"
git push origin v0.4.0
```

### Changelog

For projects with releases, maintain `CHANGELOG.md`:

```markdown
## [0.4.0] - 2026-03-15

### Added
- GitHub OAuth Device Flow authentication
- PTY session management with auto-reconnect

### Fixed
- Terminal encoding issue on non-ASCII characters

### Changed
- Migrated secrets from plugin-store to macOS Keychain
```

## MEDIUM: CI/CD Baseline

### Minimum CI for every repo

```yaml
# .github/workflows/ci.yml
name: CI
on: [push, pull_request]

jobs:
  check:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      # TypeScript/Node
      - run: pnpm install --frozen-lockfile
      - run: pnpm exec tsc --noEmit
      - run: pnpm test  # if tests exist

      # Rust (add if applicable)
      - run: cargo check
      - run: cargo clippy -- -D warnings
      - run: cargo test

      # Go (add if applicable)
      - run: go vet ./...
      - run: go test ./...
```

### Release CI (desktop apps)

```yaml
# .github/workflows/release.yml
name: Release
on:
  push:
    tags: ['v*']

jobs:
  build:
    strategy:
      matrix:
        include:
          - os: macos-latest
            target: aarch64-apple-darwin
          - os: macos-latest
            target: x86_64-apple-darwin
          - os: windows-latest
            target: x86_64-pc-windows-msvc
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - run: pnpm install --frozen-lockfile
      - run: pnpm tauri build --target ${{ matrix.target }}
      # Upload artifacts, create release, etc.
```

## MEDIUM: Code Review Checklist

When reviewing PRs, check:

- [ ] **No secrets** in diff (API keys, tokens, passwords)
- [ ] **Tests** exist for new functionality
- [ ] **Types** are correct (no new `any`, `as`, forced unwraps)
- [ ] **Error handling** is present (no swallowed errors)
- [ ] **README updated** if setup/architecture changed
- [ ] **CLAUDE.md updated** if commands/modules changed
- [ ] **Commit messages** follow Conventional Commits format
- [ ] **Branch** will be deleted after merge
- [ ] **No console.log/print debugging** left in code
- [ ] **No commented-out code** (delete it, git has history)

## HIGH: Supply Chain Hardening

### Pin third-party Actions to a SHA, not a tag

```yaml
# BAD: tag can be moved by an attacker who compromises the action
- uses: actions/checkout@v4

# GOOD: SHA cannot be moved
- uses: actions/checkout@b4ffde65f46336ab88eb53be808477a3936bae11 # v4.1.1
```

Apply to every `uses:` from outside your org. Use Dependabot to keep SHAs current.

### Required `permissions:` block

GitHub-issued `GITHUB_TOKEN` defaults to read+write on most events. Lock it down:

```yaml
permissions:
  contents: read
# Then opt-in per job:
jobs:
  release:
    permissions:
      contents: write   # to create release
      id-token: write   # for OIDC publishing
```

### Dependabot baseline

```yaml
# .github/dependabot.yml
version: 2
updates:
  - package-ecosystem: github-actions
    directory: "/"
    schedule: { interval: weekly }
  - package-ecosystem: npm        # or cargo, gomod, swift
    directory: "/"
    schedule: { interval: weekly }
    open-pull-requests-limit: 5
```

### CODEOWNERS for review routing

```
# .github/CODEOWNERS
*                       @org/maintainers
/api/                   @org/backend
/ui/                    @org/frontend
/.github/workflows/     @org/infra
```

Combine with branch protection ("Require review from Code Owners") to enforce.

## LOW: GitHub Settings

### Recommended repo settings

- **Default branch**: `main`
- **Branch protection on `main`**:
  - Require PR reviews before merging (for team repos)
  - Require status checks to pass
  - Require linear history (no merge commits)
  - Require signed commits (for security-sensitive repos)
- **Auto-delete head branches**: Enabled
- **Squash merging**: Default merge strategy
- **Issues**: Enabled
- **Discussions**: Disabled (unless community project)
- **Secret scanning + push protection**: Enabled (free for public repos)
