# Introduce Capability Blocks as the Agent Assembly Unit

| Field | Value |
|---|---|
| Author | KIM Agent Working Group |
| Date | 2026-09-14 |
| Status | Draft |
| Audience | Engineers implementing Goose × KIM assembly layer (`kim-agent-host` + `sdk/mobile`) |
| Goose pin | `goose-agent` / `goose-provider-types` / `goose-providers` **0.1.0-alpha.9** |
| Extends | `docs/impl/goose-personalized-agents.md`, `docs/impl/agent-provider-persona.md` (P-KD), `docs/impl/agent-productivity.md` (S-KD) |
| Numbering | **B-KD *n*** for decisions in this doc. Revises S-KD / prior assembly notes when stated. |
| Principle | **Goal-first.** Prefer the correct assembly model over minimal diff. Legacy `ToolSet` is a read-migration surface, not the long-term API. |

---

## Breaking Change Notice

This changes the **authoritative** on-disk / `profile_json` shape for agent capabilities.

- **Write path (new):** `capabilities: [{ "kind", "id"?, "params" }]` is authoritative.
- **Read path (compat):** missing `capabilities` → derive from legacy `tools` + `extensions` + `permissions.tools` via `CapabilityRef::from_legacy_tools`.
- **Dart prefs:** after first save under this design, rows may omit bool `tools` or keep a **derived projection** for old readers; host must not require `tools` when `capabilities` is present.
- **FFI:** `SessionOpenOpts.profile_json` schema widens; no new required FRB fields in Phase 1 if preview is a separate top-level fn. Phase 2 adds `preview_assembled` / optional `capability_catalog_json`.
- **Consumers:** only `kim-agent-host` + `sdk/mobile` (desktop agent host). No backend / WGateway change.

Migration for an old profile row:

1. Load JSON; if `capabilities` absent, run `from_legacy`.
2. On next `saveEditor` / `saveProfile`, persist `capabilities` (and optionally keep `tools` as projection for one release).
3. Host `session_open` accepts either shape forever for read; new writes prefer `capabilities` only after Dart cutover.

---

## Feasibility Assessment

**Fully feasible on published goose 0.1.0-alpha.9.** Evidence:

- `StateMachine` already rebuilds tools + `prompt_parts` every inference (`goose-agent` machine loop). KIM only underuses this.
- `MachineFactory::assemble` already maps `AgentProfile` → `Vec<Step>` (`crates/kim-agent-host/src/machine.rs`). Replacing if-else with a registry interpreter is a local refactor of that function plus new modules.
- Dual executors already exist: Rust `ToolProvider` (fs/bash/MCP/subagent/skill) vs Dart yield (`DeferredKimToolOp` + `KimCapabilityHost`). Blocks classify into the same two buckets.
- Skill layer (`skills.rs`, `SkillOp`, `activate_skill`) and Workspace (`WorkspaceSpec`, `project_root`) stay; blocks **reference** them instead of re-implementing.
- `ScriptedProvider` exists for dry-run / host tests.
- Product constraints preserved: two FFIs never merge; no unpublished `block/goose` Recipe/Doctor; P-KD create stays short form; capability bundles exclude vendor/key (bot-KD 12 / S-KD 11 / S-KD 12).

Caveats (not blockers):

- Dart editor IA must move with host (otherwise users write bools while host reads capabilities).
- macOS repo bookmark / `user_agents_skills` injection stays in Dart (`toHostJson` / `ChatAgent._ensureSession`); blocks must not assume absolute paths in persisted profile beyond today's `workspace.path` display field.
- MCP remains stdio-only until a later transport PR; block params may reserve `url` but `check()` rejects non-stdio in v1.

---

## Current Surface Inventory

### Host — assembly and profile

| Symbol | Path | Role today |
|---|---|---|
| `AgentProfile` | `crates/kim-agent-host/src/profile.rs` | Persona + `ToolSet` + `permissions` + `extensions` + `workspace` + `skills` + `steer` |
| `ToolSet` | same | 10 bools; `kim_world_names()` / `has_any()` |
| `PermissionConfig` | same | `BTreeMap<String, PermissionDefault>` by tool name |
| `DEFAULT_SYSTEM_PROMPT` | `crates/kim-agent-host/src/lib.rs` | Hard-codes six IM tool names; lies when ToolSet differs |
| `effective_system_prompt()` | `profile.rs` | Empty → `DEFAULT_SYSTEM_PROMPT` |
| `MachineFactory::assemble` | `crates/kim-agent-host/src/machine.rs` | Concatenates AGENTS.md + skill catalog into one string; if-else ToolProviders |
| `SystemPromptOp` | `ops/system_prompt.rs` | Single static `prompt: String` via `prompt_parts` |
| `SteerOp` | `ops/steer.rs` | Extra system part |
| `PermissionOp` | `ops/permission.rs` | Name-level resolve; `force_ask` for `bash` / `*__*` |
| `DeferredKimToolOp` | `ops/deferred_kim.rs` | Schema for named IM tools; `yielded()` |
| `FsToolProvider` / `BashToolProvider` | `ops/fs.rs`, `ops/bash.rs` | Root = `project_root` |
| `McpToolProvider` / `McpHub` | `ops/mcp.rs` | stdio only |
| `SubagentOp` | `ops/subagent.rs` | `delegate` → `builtin_templates()` chat-only child |
| `SkillOp` | `ops/skill.rs` | `activate_skill` |
| `build_registry` / `catalog_prompt_block` | `skills.rs` | Portable scan + app refs |
| `AgentHost::from_resolved` / `run_loop` | `lib.rs` | Calls `assemble` each turn |

### FFI / Dart

| Symbol | Path | Role |
|---|---|---|
| `SessionOpenOpts.profile_json` | `sdk/mobile/rust_agent/src/api/session.rs` | Serde into `AgentProfile` |
| `resolved_from_opts` | same | `from_legacy` or parse JSON; `normalize_mode`; `apply_reasoning` |
| `AgentProfile` / `AgentToolSet` / `toHostJson` | `sdk/mobile/lib/state/agent_profiles.dart` | Prefs + inject provider + `user_agents_skills` |
| `ChatAgent._ensureSession` | `sdk/mobile/lib/state/chat_agent.dart` | Resolves cwd, builds opts + `profile_json` |
| `KimCapabilityHost` | `sdk/mobile/lib/agent/capability_host.dart` | Executes deferred IM tools |
| Editor routes | `app_router.dart` | `/agent/:id`, `/workspace`, `/skills`, `/tools`, `/plaza` |
| Pages | `agent_settings_page.dart`, `agent_workspace_page.dart`, `agent_skills_page.dart`, `agent_tools_page.dart`, `agent_plaza_page.dart` | Split IA; inconsistent save |

### Explicitly out of scope for this plan

- Merging `kim_agent_ffi` and `kim_client_ffi`
- Server-side Goose / storing API keys in Royal
- OS seatbelt / Docker
- Goose Recipe as persona (S-KD 12)
- Hooks (S-KD 17)
- Arbitrary remote skill CDN (beyond future KIM first-party catalog)
- Group @mention fan-out

---

## Design

### Goals

- One **Capability Block** is the unit of assembly: tools (+ optional deferred names), prompt fragments, default permission rules, and `check()` against workspace/skills.
- `MachineFactory` becomes an **interpreter** over resolved parts — not a hand-written switch on ten bools.
- System prompt layers: **identity (user)** + **capability digest (generated)** + **workspace AGENTS.md** + **skill catalog** + **steer** + optional per-turn dynamic parts. Tool lists in prose always match advertised schemas (fixes DEFAULT_SYSTEM_PROMPT lie).
- `preview_assembled` and real `assemble` share one resolve path.
- UX: create stays short; details assemble via capability cards; one “what this agent gets” preview; high-risk configs get dry-run.
- Capability **bundle** export/import = blocks + params + skills + identity — no vendor/key.

### Non-goals

- Rewriting goose-agent
- Replacing Skill portable/app split
- Shipping a template gallery that binds vendors (P-KD 1)
- Making every MCP transport work in v1

### Key Decisions

1. **B-KD 1 — `capabilities[]` is authoritative; `ToolSet` is legacy projection only.**  
   Rejected: keep bools forever and only generate prompts from them (does not fix G2).  
   Rejected: dual-write forever as equal sources of truth (drift).  
   Write: capabilities. Read: capabilities or `from_legacy(tools, extensions)`.

2. **B-KD 2 — Block kind is registered; instance id is runtime.**  
   Kinds: `im.send_message`, `im.search_contacts`, `im.search_messages`, `im.get_conversation_context`, `im.read_clipboard`, `im.list_profiles`, `fs`, `bash`, `mcp`, `subagent`, `skill` (optional sugar — skills may stay on `profile.skills` and only contribute catalog via existing Skill path).  
   MCP instance id e.g. `mcp:github`. Params carry command/transport.  
   Rejected: `'static str` Capability trait id for every instance.

3. **B-KD 3 — Blocks emit `CapabilityPart`, not one Step per block.**  
   Parts fold into: `PromptComposeOp` (or multi-op `prompt_parts`), one `DeferredKimToolOp`, one `ToolOperation` with many providers, shared `PermissionOp` rules, existing `SkillOp` when registry non-empty.  
   Rejected: N× `ToolOperation` steps (name collisions / order chaos).

4. **B-KD 4 — Identity prompt never lists tools.**  
   Replace `DEFAULT_SYSTEM_PROMPT` tool enumeration with a short identity-only fallback (`DEFAULT_IDENTITY_PROMPT`). Capability digest is always generated from resolved parts.  
   Revises P-KD 3/14 wording: empty user prompt → identity fallback **plus** generated layers, not the old English tool laundry list.

5. **B-KD 5 — Permission upgrades to ordered rules; tool-name map remains a sugar.**  
   `PermissionRule` matches tool name, optional fs path prefix, bash argv prefix, or `ext__` MCP tool. First match wins; else GooseMode defaults; `force_ask` becomes default rules for bash/MCP unless `NeverAllow`.  
   Rejected: only renaming `PermissionConfig.tools` without patterns (leaves G4).

6. **B-KD 6 — Preview and dry-run are first-class host APIs.**  
   `preview_assembled(profile_json, project_root) -> AssembledPreview`.  
   Dry-run: ScriptedProvider + fixed user turn → list of tool names that would be advertised + which would AskBefore — no network.  
   Rejected: UI-only string guess of tools.

7. **B-KD 7 — Subagent children assemble via the same Factory.**  
   `delegate` params: `profile_id` or inline capability subset + `max_turns`. Child must not get `im.send_message` unless explicitly granted.  
   Revises hard-coded `builtin_templates()` chat-only machine in `subagent.rs`.

8. **B-KD 8 — Skill `requires_tools` enforced at assign UI and at assemble.**  
   Missing caps → structured warning; assemble may omit skill from catalog or fail closed for app skills that declare hard requires (prefer warn + skip catalog entry for v1).  
   Implements S-KD 7 for real.

9. **B-KD 9 — UX: one mental model “Persona + Capabilities”; storage layers may stay split.**  
   Revises S-KD 30 presentation: user should not need three saves across workspace/tools/skills to understand “what can it do”. Implementation may keep routes but must unify save semantics and surface a single capability sheet + preview.  
   Plaza remains discovery; assign always scoped to a persona.

10. **B-KD 10 — Capability bundle ≠ Recipe.**  
    Export JSON: identity, capabilities, skills, workspace kind (not absolute secret paths by default), permission rules. No `api_key`, no provider account. Aligns S-KD 12 / bot-KD 12.

11. **B-KD 11 — Goal-first phasing.**  
    Schema + registry + interpreter land together as the host truth (Phase 1–2). Do not ship a long-lived “prompt-only fake layer” that still reads only bools as the product API.

### Target types (host)

New module `crates/kim-agent-host/src/capability/` (suggested files: `mod.rs`, `registry.rs`, `part.rs`, `legacy.rs`, `preview.rs`, blocks under `capability/blocks/`).

```rust
/// Persisted reference. `kind` selects the registered block.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CapabilityRef {
    pub kind: String,
    /// Optional instance id (`mcp:github`). Empty → kind is enough for singletons.
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub params: serde_json::Value,
    #[serde(default = "enabled_true")]
    pub enabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiskTier {
    Read,
    Write,
    External,
    Destructive,
}

/// What one enabled block contributes after `build`.
pub struct CapabilityPart {
    pub kind: String,
    pub instance_id: String,
    pub risk: RiskTier,
    /// Injected as system (or tagged) prompt_parts; must stay in sync with tools.
    pub prompt_parts: Vec<(String, String)>,
    pub deferred_tool_names: Vec<String>,
    pub in_process: Option<Arc<dyn ToolProvider<HostSession>>>,
    pub permission_defaults: Vec<PermissionRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "match", rename_all = "snake_case")]
pub enum PermissionMatch {
    Tool { name: String },
    FsPath { prefix: String },
    BashArgv { prefix: String },
    McpTool { extension: String, name: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PermissionRule {
    pub r#match: PermissionMatch,
    pub effect: PermissionDefault,
}

pub struct AssembledPreview {
    pub identity: String,
    pub prompt_layers: Vec<(String, String)>, // label → text
    pub full_system_prompt: String,
    pub tools: Vec<PreviewTool>,
    pub permission_matrix: Vec<(String, String)>, // tool → effect
    pub warnings: Vec<String>,
    pub step_names: Vec<String>,
}

pub struct PreviewTool {
    pub name: String,
    pub source: String, // kind or "skill" / "mcp:…"
    pub executor: String, // "rust" | "dart"
}
```

Block registration (compile-time table + MCP/skill instance factory):

```rust
pub trait CapabilityBlock: Send + Sync {
    fn kind(&self) -> &'static str;
    fn risk(&self) -> RiskTier;
    fn param_schema(&self) -> serde_json::Value;
    fn check(&self, params: &Value, ctx: &AssembleCtx) -> Result<(), HostError>;
    fn build(&self, params: &Value, ctx: &AssembleCtx) -> Result<CapabilityPart, HostError>;
}

pub struct AssembleCtx<'a> {
    pub profile: &'a AgentProfile,
    pub project_root: &'a Path,
    pub mcp: Arc<McpHub>,
    pub provider: Arc<dyn Provider>,
    pub model: ModelConfig,
}
```

`AgentProfile` additions (serde):

```rust
// in AgentProfile
#[serde(default)]
pub capabilities: Vec<CapabilityRef>,

#[serde(default)]
pub permission_rules: Vec<PermissionRule>,

// Keep tools / permissions.tools for read+projection:
pub fn project_toolset(&self) -> ToolSet { /* from capabilities */ }
pub fn resolve_capabilities(&self) -> Vec<CapabilityRef> {
    if !self.capabilities.is_empty() {
        return self.capabilities.clone();
    }
    CapabilityRef::from_legacy(&self.tools, &self.extensions)
}
```

Identity fallback (replace tool laundry list):

```rust
pub const DEFAULT_IDENTITY_PROMPT: &str =
    "You are a local desktop agent inside the KIM messenger. \
You run on the user's machine (not a cloud bot). Reply in the user's language. Be concise. \
Only use tools that appear in your tool list; never claim tools you were not given.";
```

### Prompt layering (every turn)

Order of `prompt_parts` contributions (labels for preview):

| Layer | Source | Op |
|---|---|---|
| L1 identity | `profile.system_prompt` or `DEFAULT_IDENTITY_PROMPT` | `PromptComposeOp` / `SystemPromptOp` rewritten |
| L2 capability digest | Each part’s `prompt_parts` + optional summary line “You have: …” | same / per-block fragments |
| L3 workspace | `AGENTS.md` ≤4KiB (existing) | compose |
| L4 skill catalog | `catalog_prompt_block` (existing; bodies never here) | compose |
| L5 steer | `profile.steer` | `SteerOp` or fold into compose |
| L6 dynamic (optional later) | date / memory recall via async `prompt_parts` | future block |

Do **not** `push_str` skill catalog into a single pre-baked string inside `assemble` before wrapping `SystemPromptOp` if those layers can be separate `prompt_parts` — prefer separate fragments so preview can show layers. Implementation may use one `PromptComposeOp` that holds `Vec<(label, text)>` and implements `prompt_parts`.

### Assemble algorithm (target)

```text
caps = profile.resolve_capabilities().filter(enabled)
ctx = AssembleCtx { … }
parts = []
warnings = []
for ref in caps:
  block = registry.get(ref.kind)?
  block.check(ref.params, ctx)? or push warning
  parts.push(block.build(ref.params, ctx)?)

// skills path unchanged: build_registry from profile.skills + portable scan rules
// if skill requires missing caps → warning / skip (B-KD 8)

deferred = flatten parts.deferred_tool_names (dedupe)
providers = flatten parts.in_process
rules = profile.permission_rules ++ flatten parts.permission_defaults
       ++ legacy permissions.tools as Tool{name} rules

steps = [
  PromptCompose(L1..L5),
  MaxTurns?, Compaction,
  PermissionOp(mode, rules),
  DeferredKim? if deferred non-empty,
  ToolOperation with providers,
  SkillOp? if registry non-empty,
  UnknownTool or ChatGuard,
  InferenceRunner,
]
```

Chat-only iff no deferred, no providers, empty skill registry, empty extensions-as-mcp parts.

### Initial block catalog (v1)

| kind | params (sketch) | executor | default risk / permission |
|---|---|---|---|
| `im.send_message` | `{}` | dart | Write / ask |
| `im.search_contacts` | `{}` | dart | Read / allow |
| `im.search_messages` | `{}` | dart | Read / allow |
| `im.get_conversation_context` | `{}` | dart | Read / allow |
| `im.read_clipboard` | `{}` | dart | External / ask |
| `im.list_profiles` | `{}` | dart | Read / allow |
| `fs` | `{ "writable": bool }` | rust | Read or Write |
| `bash` | `{ "allow_argv_prefixes": ["git ","cargo "] }` optional | rust | Destructive / ask (never always via UI) |
| `mcp` | `{ "name", "command": [], "transport": "stdio" }` | rust | External / ask per tool |
| `subagent` | `{ "max_turns": 8, "child_profile_id"?: string }` | rust | Write / ask on `delegate` |

IM kinds stay separate (not one `im` mega-block) so digest and permissions stay precise — matches today’s six tools.

### Legacy mapping

```text
tools.send_message          → { kind: "im.send_message" }
tools.search_* / …          → corresponding im.*
tools.fs && !fs_write       → { kind: "fs", params: { writable: false } }
tools.fs_write              → { kind: "fs", params: { writable: true } }  // implies read tools
tools.bash                  → { kind: "bash", params: {} }
tools.subagent              → { kind: "subagent", params: {} }
extensions[]                → { kind: "mcp", id: "mcp:{name}", params: {…} }
permissions.tools[name]     → PermissionRule::Tool { name } + effect
```

`kCreateDefaultTools` (Dart) becomes default `capabilities` list for new agents: `im.send_message` + `im.read_clipboard` (and any others product already enables).

### UX target (same release train as host truth)

```text
/agent/new          短表单：名 / Provider / 模型 / 身份提示词   (unchanged P-KD)
/agent/:id          总览：身份 + 模型 +「能力」入口 + 预览摘要 + 去聊天
/agent/:id/capabilities   积木卡：开关 + 风险徽章 + params 表单 + 依赖提示
                          工作区（sandbox/repo）嵌在 fs/bash 相关卡下方
                          技能：已分配列表 + 跳转广场(assignTo=id)
                          高级：MCP 实例卡、权限规则/矩阵
预览抽屉/底栏         AssembledPreview（工具 / 矩阵 / 分层 prompt）
```

Save: capability sheet autosaves on toggle (like today’s skills optimistic save). Overview save only identity/model/prompt.

Routes `/workspace` `/tools` `/skills` may redirect into sections of `/capabilities` for one release.

### Usage example (host)

```rust
let caps = profile.resolve_capabilities();
let preview = capability::preview::preview_assembled(&profile, project_root, mcp.clone())?;
assert!(preview.tools.iter().any(|t| t.name == "read_file") == profile_has_fs(&caps));

let steps = MachineFactory::assemble(&profile, provider, model, project_root, mcp);
```

---

## Phased Implementation

Phases are ordered by **goal completeness**. Each phase should leave `cargo test -p kim-agent-host` green. Desktop Flutter tests update in the phase that changes Dart.

### Phase 0: Contract freeze (doc + golden JSON only)

**File: `docs/impl/agent-capability-blocks.md`** (this file)

- Lock B-KD 1–11, block kind table, legacy map, preview DTO.
- Add appendix golden examples: legacy goose row → `capabilities[]`; coder-like row with fs writable + bash.

**File: `crates/kim-agent-host/tests/fixtures/capability_legacy_goose.json`** (new)

- Minimal legacy profile JSON used by Phase 1 unit tests.

No runtime change.

### Phase 1: Registry + resolve + prompt layers (host truth)

Goal: eliminate G1/G3 inside the host; `assemble` interprets capabilities (derived from legacy if needed).

**File: `crates/kim-agent-host/src/capability/mod.rs`** (new)

- `CapabilityRef`, `CapabilityPart`, `RiskTier`, `AssembleCtx`, `CapabilityBlock` trait.
- `registry()` returns `&'static […]` or `HashMap<&'static str, Arc<dyn CapabilityBlock>>`.

**File: `crates/kim-agent-host/src/capability/legacy.rs`** (new)

- `CapabilityRef::from_legacy(tools, extensions)`.
- `ToolSet::from_capabilities(&[CapabilityRef])` projection for any remaining callers.

**File: `crates/kim-agent-host/src/capability/blocks/*.rs`** (new)

- One module per kind group (`im.rs`, `fs.rs`, `bash.rs`, `mcp.rs`, `subagent.rs`).
- `build` wraps existing providers: e.g. fs block constructs `FsToolProvider { root: ctx.project_root, writable }` and prompt fragment describing exact tool names.
- IM blocks only push deferred names + prompt fragment; schemas stay in `deferred_kim.rs` (single schema source — call `kim_tool_schema`).

**File: `crates/kim-agent-host/src/ops/prompt_compose.rs`** (new)

- Holds `layers: Vec<(String /*label*/, String /*text*/)>`.
- `prompt_parts` returns all layers as system (or role policy as today).
- Unit test: layer order stable.

**File: `crates/kim-agent-host/src/ops/system_prompt.rs`**

- Either delete after migrate callers, or thin-wrap `PromptComposeOp` with one layer — prefer delete duplicate.

**File: `crates/kim-agent-host/src/lib.rs`**

- Add `DEFAULT_IDENTITY_PROMPT`; keep `DEFAULT_SYSTEM_PROMPT` as deprecated alias **equal to identity only** for one cycle, or remove tool list immediately and fix tests.
- Export capability preview types when ready (Phase 2 can add fn).

**File: `crates/kim-agent-host/src/profile.rs`**

- Add `capabilities`, `permission_rules` fields with `#[serde(default)]`.
- `resolve_capabilities()`, `project_toolset()`.
- `effective_system_prompt()` returns **identity only** (user text or `DEFAULT_IDENTITY_PROMPT`) — rename semantically to `effective_identity_prompt()`; update call sites.
- `normalize_mode()` uses `project_toolset().has_any()` || capabilities/mcp/skills.

**File: `crates/kim-agent-host/src/machine.rs`**

- Replace opening `push_str` prompt construction with: resolve parts → build `PromptComposeOp` layers (identity, digest from parts, AGENTS.md, catalog, steer).
- Replace tool if-else with fold over `CapabilityPart`.
- Keep Skill registry construction (S-KD 3 scan rules) before digest so catalog layer still correct.
- Tests: empty tools + empty capabilities → chat guard; legacy fs profile → `read_file` in preview/tools; identity without tool names when only translator-like empty caps.

**File: `crates/kim-agent-host/src/ops/deferred_kim.rs`**

- Unchanged schemas; names come from parts.

**File: `crates/kim-agent-host/src/ops/permission.rs`**

- Phase 1: still name map; merge `permission_defaults` from parts into a synthetic `PermissionConfig` for existing `resolve()`. Full rule engine in Phase 3.

Verification:

```bash
cargo test -p kim-agent-host
```

Assert: no assembled identity/digest claims `send_message` unless that capability is enabled.

### Phase 2: Preview API + FFI + Dart model write path

Goal: G8 + Dart persists `capabilities`.

**File: `crates/kim-agent-host/src/capability/preview.rs`** (new)

- `preview_assembled(profile, project_root, mcp) -> Result<AssembledPreview, HostError>`.
- Must call the same resolve/build as `MachineFactory` (extract `resolve_parts` shared fn).

**File: `sdk/mobile/rust_agent/src/api/session.rs`**

- Add top-level `pub fn preview_assembled(profile_json: String, project_root: String) -> Result<String, String>` returning JSON.
- Optional: `capability_catalog_json() -> String` for UI (kind, risk, param_schema, titles).
- FRB codegen; commit generated Dart.

**File: `sdk/mobile/lib/state/agent_profiles.dart`**

- Add `List<CapabilityRef> capabilities` (mirror serde).
- `fromJson`: if capabilities empty, derive from `AgentToolSet` / extensions (Dart-side same map as Rust, or trust host-only — **prefer Dart derive on load so UI works offline**).
- `toJson` / `toHostJson`: write `capabilities`; keep `tools` projection for one release (`projectToolSet()`).
- `kCreateDefaultTools` → `kCreateDefaultCapabilities`.

**File: `sdk/mobile/lib/state/chat_agent.dart`**

- `_ensureSession` continues `toHostJson`; no behavioral change if projection correct.
- When reconfigure detects capability/workspace/skills change (already compares tools/extensions/skills/workspace), include capabilities equality.

**File: `sdk/mobile/lib/agent_bridge.dart`**

- Expose `previewAssembled` / `capabilityCatalogJson`.

**Tests:** host preview fixture; Dart parse/serialize round-trip; `chat_agent_test` still one assistant bubble.

### Phase 3: Permission rules + Skill requires

**File: `crates/kim-agent-host/src/ops/permission.rs`**

- Introduce rule list evaluation; convert legacy map to rules at assemble.
- Bash argv / fs path matching: evaluate using tool arguments from `ToolRequest` (parse JSON args). If parse fails → AskBefore.
- Keep conversation ActionRequired / Response replay (no `set_message_meta`).

**File: `crates/kim-agent-host/src/skills.rs` + app-skill frontmatter**

- Extend parse for optional `metadata.kim.requires_tools: ["fs", …]` or map to capability kinds.
- `build_registry`: if requires unmet → omit from catalog + warning string for preview.

**File: `sdk/mobile/lib/screens/agent/agent_skills_page.dart`**

- On assign, if requires missing: dialog to enable capabilities (explicit confirm) then persist capabilities — not silent ToolSet bits only.

**File: Dart tools/permissions UI**

- Matrix driven by `AssembledPreview.permission_matrix` / catalog, not hard-coded five rows.

### Phase 4: Capability sheet UX consolidation

Goal: fix “too many / messy” without waiting for more blocks.

**File: `sdk/mobile/lib/router/app_router.dart`**

- Add `/agent/:id/capabilities`; redirect old workspace/tools/skills paths.

**File: `sdk/mobile/lib/screens/agent/agent_capabilities_page.dart`** (new)

- Sections: IM / Files & shell / MCP / Skills.
- Cards from `capability_catalog_json` + current refs.
- Workspace picker embedded under fs card (reuse `workspace_access`).
- Bottom persistent preview (fetch `preview_assembled` debounced).
- Autosave.

**File: `sdk/mobile/lib/screens/agent/agent_settings_page.dart`**

- Overview: remove dense duplication; one entry to capabilities; show preview subtitle (tool count + warnings).
- Create path unchanged.

**File: `agent_plaza_page.dart`**

- Require `assignTo` for assign actions; list icon entry secondary.

**File: l10n `app_zh.arb` / `app_en.arb`**

- Capability card copy; preview labels; redirect strings.

### Phase 5: Subagent via Factory + dry-run + bundles

**File: `crates/kim-agent-host/src/ops/subagent.rs`**

- Build child `AgentProfile` from `builtin_templates` **or** loaded profile id passed in host context (Phase 5 may require Dart to embed allowed child snapshots in params / host session registry).
- Child `MachineFactory::assemble` with stripped capabilities (no `im.send_message` unless params allow).
- `max_turns` from params.

**File: host dry-run**

- `dry_run_turn(profile_json, project_root, user_text) -> { advertised_tools, would_ask: [] }` using ScriptedProvider returning a fixed tool call plan **or** static analysis of parts only for v1 (prefer static from preview; full scripted loop optional).

**File: bundle import/export**

- Dart: export identity + capabilities + skills + permission_rules + workspace.kind (sandbox default on import).
- Validate kinds against catalog; show preview before apply.

### Phase 6: Verification gate

```bash
cargo test -p kim-agent-host
cd sdk/mobile && flutter test test/state/chat_agent_test.dart test/agent/ agent_profiles capability-related
```

Manual desktop checklist:

1. New agent: default caps → preview shows send_message + clipboard only; digest matches.
2. Enable fs read → preview gains read_file/list_dir; identity has no false shell claims.
3. Assign kim-im with requires → prompts to enable IM caps.
4. Legacy profile without capabilities still opens and chats.
5. Repo workspace + bash ask card still works.
6. Export bundle → import on second profile → same preview tool set.

---

## Architectural Notes

- **Semver:** `kim-agent-host` is an in-repo crate; treat profile JSON as the compatibility surface. Document dual-read in `docs/agent-goose.md` when landing Phase 2.
- **Thread safety:** Blocks are stateless registries; `McpHub` / providers remain as today behind `Arc`.
- **Effects:** Permission and tool execution semantics unchanged at the Goose yield layer; only how steps are chosen changes.
- **Prompt vs Skill body:** S-KD 13 preserved — catalog in L4, bodies only via `activate_skill`.
- **No `set_message_meta` for permission memory** — keep ActionRequired replay (personalized-agents decision).
- **Two FFIs:** Dart still executes IM tools; blocks marked deferred never gain Rust `ToolProvider`.
- **Performance:** `assemble` already runs per turn; registry build must stay cheap (no disk scan beyond existing skill/AGENTS paths).
- **Security:** bash allowlist is deny-by-default-ask; never expose AlwaysAllow as primary UI for bash/MCP (existing product rule).
- **Explicitly NOT changed:** Royal/chat bot identity, ProviderAccount catalog/`ReasoningSurface`, WGateway talk path, mobile IM-only clients.

### Mapping prior gaps → phases

| Gap | Phase |
|---|---|
| G1 prompt/tool lie | 1 |
| G2 closed ToolSet | 1–2 |
| G3 no layered/dynamic prompt | 1 (dynamic L6 later) |
| G4 permission dimensions | 3 |
| G5 subagent rigid | 5 |
| G6 skill requires | 3 |
| G7 capability bundle | 5 |
| G8 no preview | 2–4 |
| UX mess | 4 |

---

## File Change Summary

- `crates/kim-agent-host/src/capability/mod.rs` -- registry trait, refs, parts
- `crates/kim-agent-host/src/capability/legacy.rs` -- ToolSet ↔ capabilities
- `crates/kim-agent-host/src/capability/preview.rs` -- AssembledPreview
- `crates/kim-agent-host/src/capability/blocks/*.rs` -- im/fs/bash/mcp/subagent builders
- `crates/kim-agent-host/src/ops/prompt_compose.rs` -- layered prompt_parts
- `crates/kim-agent-host/src/ops/system_prompt.rs` -- remove or wrap
- `crates/kim-agent-host/src/ops/permission.rs` -- rule engine (Phase 3)
- `crates/kim-agent-host/src/ops/subagent.rs` -- Factory-based child (Phase 5)
- `crates/kim-agent-host/src/ops/deferred_kim.rs` -- schema source reused by im blocks
- `crates/kim-agent-host/src/machine.rs` -- interpreter assemble
- `crates/kim-agent-host/src/profile.rs` -- capabilities, permission_rules, resolve/project
- `crates/kim-agent-host/src/lib.rs` -- DEFAULT_IDENTITY_PROMPT, exports
- `crates/kim-agent-host/src/skills.rs` -- requires_tools honor
- `crates/kim-agent-host/app-skills/*/SKILL.md` -- optional requires metadata
- `crates/kim-agent-host/tests/fixtures/*.json` -- golden legacy/capability profiles
- `sdk/mobile/rust_agent/src/api/session.rs` -- preview_assembled (+ catalog JSON)
- `sdk/mobile/rust_agent` FRB outputs -- codegen
- `sdk/mobile/lib/agent_bridge.dart` -- preview/catalog wrappers
- `sdk/mobile/lib/state/agent_profiles.dart` -- CapabilityRef persist + projection
- `sdk/mobile/lib/state/chat_agent.dart` -- dirty compare capabilities
- `sdk/mobile/lib/screens/agent/agent_capabilities_page.dart` -- unified sheet
- `sdk/mobile/lib/screens/agent/agent_settings_page.dart` -- light overview + preview entry
- `sdk/mobile/lib/screens/agent/agent_workspace_page.dart` -- redirect or embed
- `sdk/mobile/lib/screens/agent/agent_tools_page.dart` -- redirect or embed
- `sdk/mobile/lib/screens/agent/agent_skills_page.dart` -- section / requires dialog
- `sdk/mobile/lib/screens/agent/agent_plaza_page.dart` -- assignTo required
- `sdk/mobile/lib/router/app_router.dart` -- capabilities route + redirects
- `sdk/mobile/lib/l10n/app_*.arb` -- copy
- `docs/impl/agent-capability-blocks.md` -- this plan
- `docs/agent-goose.md` -- short pointer when Phase 2 lands

Unchanged on purpose: `crates/kim-client/**`, `services/**`, `sdk/mobile/rust/**` (IM FFI), WGateway proto.

---

## Appendix A — Example legacy → capabilities

Legacy tools:

```json
{
  "tools": {
    "send_message": true,
    "read_clipboard": true,
    "fs": true,
    "fs_write": false,
    "bash": false
  },
  "extensions": []
}
```

Resolved:

```json
{
  "capabilities": [
    { "kind": "im.send_message", "params": {}, "enabled": true },
    { "kind": "im.read_clipboard", "params": {}, "enabled": true },
    { "kind": "fs", "params": { "writable": false }, "enabled": true }
  ]
}
```

Digest (illustrative):

```text
You have: send_message (requires user confirmation); read_clipboard (requires user confirmation);
read_file, list_dir (workspace files, read-only). You do not have write_file or bash.
```

---

## Appendix B — Engineer checklist

1. Never put tool laundry lists in user-editable identity prompt defaults.
2. Never advertise a tool in digest that is absent from `inference_tools` aggregation.
3. Never merge agent FFI with kim-client.
4. Never persist API keys in capability bundles.
5. `preview_assembled` and `MachineFactory::assemble` must share `resolve_parts`.
6. bash/MCP: no AlwaysAllow primary control.
7. Portable skills still never copied into app support; app skills never written to `~/.agents`.
8. After Dart cutover, new saves write `capabilities`; `tools` projection optional then removed in a follow-up PR.
9. rust-strict: no `unwrap`/`expect` on production paths; no logging of repo home details at info+.
10. Goal-first: do not land “prompt concat cleanup” as the final API without `capabilities[]` registry.
