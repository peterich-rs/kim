# Android Logic SO OTA（hotfix channel）

App-store APK is **primary**. This channel ships **small Android-only hotfixes of non-platform logic** by swapping two arm64-v8a shared objects at cold start. It is not a general in-app updater, not an iOS feature, and not a substitute for Play / store releases when platform or resources change.

**Catalog = GitHub Releases** (no Royal check API). Clients list public releases on `peterich-rs/kim`, verify Ed25519-signed manifests, and download release assets.

Related: [mobile-client.md](mobile-client.md), tooling under [`sdk/mobile/ota/`](../sdk/mobile/ota/).

## Policy

| Rule | Detail |
|---|---|
| Primary distribution | Store APK / AAB. Users always have a working builtin engine + Dart AOT + FFI. |
| OTA allowed | Only `libapp.so` (Dart AOT) + `libkim_client_ffi.so` (Rust FRB FFI), `abi=arm64-v8a`. **Both SOs always ship together** (atomic pair). |
| OTA forbidden | `libflutter.so`, Java/Kotlin, resources, Manifest, Gradle, plugins, permissions, `libsqlite3.so` (phase 1), assets, JNI bridges other than the two SOs above. |
| Platform / resource change | Cut a **full app release** and bump `host_line`. Do not attempt SO OTA. |
| Integrity | SHA-256 of every artifact + **Ed25519** signature over canonical `manifest.json` bytes. Not MD5. |
| Keys | Public key embedded in the APK (`assets/ota/ed25519_public.pem`). Private key is **CI-only** (`OTA_ED25519_PRIVATE_PEM` GitHub Actions secret). Never commit private keys. |
| Scope | Android only. No iOS OTA. |
| Catalog | GitHub Releases API + assets. No Royal / CDN check server. |

## Boundaries (what “logic” means)

**Logic (OTA-eligible):** Dart UI/business compiled into `libapp.so`, and Rust `kim-client` FFI in `libkim_client_ffi.so`, when both stay ABI-compatible with the installed host APK.

**Platform (full release + new `host_line`):** Flutter engine (`libflutter.so`), Android embedding / Activity / Application Kotlin, Gradle, Manifest permissions, plugins (camera, secure storage, …), resources/themes, sqlite3 native, anything that changes the JNI / plugin registrant surface.

If a PR touches both logic and platform, ship a store build. CI path filters for the logic-ota workflow must fail closed on platform paths.

## Version mapping

| Field | Meaning |
|---|---|
| Release tag | `logic-ota-v{logic_version}` (e.g. `logic-ota-v42`). Client filters tags by `OTA_TAG_PREFIX` (default `logic-ota-v`). |
| `host_line` | Compatibility line of the store APK (e.g. `kim-android-1`). Bump when platform/engine/plugins change. |
| `min_host_version_code` / `max_host_version_code` | Inclusive `versionCode` window that may apply this logic package. |
| `engine_build_id` | Flutter / engine identity the AOT was built against (e.g. `3.47.2`). Must match host. |
| `logic_version` | Monotonic string for the logic package (e.g. `42` or `20260905.1`). Compared dotted-numeric when possible. |
| `abi` | Always `arm64-v8a` in phase 1. |
| `channel` | `stable` / `beta` / `dev`. Must match client BuildConfig channel. |

### Mapping examples

| Host APK | `host_line` | `versionCode` | `engine_build_id` | Eligible logic |
|---|---|---|---|---|
| Play 1.0.0+12 | `kim-android-1` | 12 | `3.47.2` | manifests with `host_line=kim-android-1`, `min≤12≤max`, same engine |
| Play 1.1.0+20 after plugin bump | `kim-android-2` | 20 | `3.47.2` | only `kim-android-2` packages; old line ignored |
| Engine bump to 3.48 | `kim-android-3` | 30 | `3.48.0` | new line; never apply 3.47 AOT onto 3.48 host |

Client rejects a package when any of: wrong `host_line`, `versionCode` outside window, `engine_build_id` mismatch, wrong `abi` / `channel`, failed SHA-256, failed Ed25519, or crash-loop marker from a prior OTA boot.

## Artifact zip layout

Published as **GitHub Release assets** (not CDN / Royal):

```text
Release tag: logic-ota-v<logic_version>
  logic-ota-<logic_version>-arm64-v8a.zip
    ├── libapp.so
    └── libkim_client_ffi.so
  manifest.json                 # sibling asset, not inside the zip
  manifest.json.sig            # Ed25519 over exact manifest.json bytes
```

`manifest.json` lists SHA-256 for each SO **and** for the zip. Client downloads zip + manifest + sig from `browser_download_url`, verifies sig → parses manifest → checks host mapping → verifies hashes → stages files.

## Manifest schema

See [`sdk/mobile/ota/manifest.schema.json`](../sdk/mobile/ota/manifest.schema.json) and [`manifest.example.json`](../sdk/mobile/ota/manifest.example.json).

Canonical signing input: UTF-8 bytes of `manifest.json` as published (no re-serialization). Signature is raw 64-byte Ed25519 (OpenSSL `pkeyutl -sign -rawin`), distributed as binary `manifest.json.sig`.

## Catalog: GitHub Releases (no Royal check API)

Client (background, soft-fail):

```http
GET https://api.github.com/repos/{OTA_GITHUB_OWNER}/{OTA_GITHUB_REPO}/releases?per_page=30
Accept: application/vnd.github+json
User-Agent: KimMobileOTA/1.0
```

Defaults: owner `peterich-rs`, repo `kim`, tag prefix `logic-ota-v`.

Walk releases **newest-first**:

1. Skip drafts.
2. Require `tag_name` starts with `OTA_TAG_PREFIX`.
3. Map assets by name; require `manifest.json`, `manifest.json.sig`, and a zip named `logic-ota-*-arm64-v8a.zip` (or exactly one `.zip` asset).
4. Download manifest + sig; verify Ed25519 with embedded public key.
5. Parse `OtaManifest`; require channel / host_line / engine / abi / host version window match.
6. Skip if `logic_version` is not newer than installed (equal → skip; installed null → accept; prefer dotted-numeric compare, else lexicographic).
7. On first compatible newer offer: download zip, verify sha256, unzip allowlist, promote. Apply on **next cold start**.

Errors: network / HTTP 403 / 429 / non-2xx → treat as no update (fail soft). Client never installs without verifying the signed manifest bytes.

BuildConfig / `OtaConfig`: `OTA_GITHUB_OWNER`, `OTA_GITHUB_REPO`, `OTA_TAG_PREFIX` (no `OTA_CHECK_BASE_URL`).

## Client cold-start + rollback

Storage under `context.filesDir/ota/`:

```text
ota/
  staging/     # download + unpack + verify
  current/     # libapp.so + libkim_client_ffi.so (active)
  previous/    # last good (optional one-shot rollback)
```

Prefs: `logic_version`, `ota_active`, crash marker `ota_crash_pending`.

### Boot sequence (early)

1. `KimApplication.onCreate` → `OtaGate.bootstrap()` **before** Flutter engine init.
2. If `ota_crash_pending`: clear `current/`, clear marker, use APK builtins (crash-loop protection).
3. Else if `current/libkim_client_ffi.so` exists and prefs say active: `System.load(absPath)`; set in-process flag so Dart FRB uses that library (no double-load of APK copy).
4. If `current/libapp.so` exists: `MainActivity.getFlutterShellArgs()` adds `--aot-shared-library-name=<filesDir>/ota/current/libapp.so`. Flutter 3.47 `FlutterLoader` accepts paths under app internal storage (see `getSafeAotSharedLibraryName`). Fallback: omit flag → APK builtin `libapp.so`.
5. Set `ota_crash_pending=true` when booting with OTA libs; clear it after Dart reports first successful frame / MethodChannel `ota.markHealthy`.
6. Background: GitHub Releases catalog → download → verify → atomic promote `staging → current` (and shift old current → previous). **Apply on next cold start** (no mid-session SO swap).

### Rollback

- Automatic: crash marker → next launch drops `current`.
- Manual / future: restore `previous/` → `current/` if hashes still match embedded policy.

## Security

- HTTPS only (GitHub API + asset CDN).
- Ed25519 verify with embedded public key; reject if sig missing or wrong.
- SHA-256 over file bytes before promote.
- Refuse install if zip contains unexpected members (path traversal, extra `.so`, non-allowlisted names).
- Private key never in APK or git; rotate by shipping a new host APK with a new public key (`host_line` bump).

## CI: build + publish Release

Workflow: [`.github/workflows/logic-ota.yml`](../.github/workflows/logic-ota.yml) (mirror: [`sdk/mobile/ota/logic-ota.workflow.yml`](../sdk/mobile/ota/logic-ota.workflow.yml)).

Triggers:

- `workflow_dispatch` with inputs: `logic_version`, `host_line`, `min_host`, `max_host`, `channel`, `engine_build_id`
- `push` tags `logic-ota-v*`

Jobs:

1. **guard** — on `workflow_dispatch`, refuse if `origin/main...HEAD` touches `sdk/mobile/android/**`, `ios/**`, or `plugins/**`. Tag builds skip the path guard.
2. **build-pack-release** — Flutter release APK (arm64) → extract both SOs → `pack_logic_ota.sh` signed with `secrets.OTA_ED25519_PRIVATE_PEM` (required) → `verify_logic_ota.sh` → `gh release create` with zip + manifest + sig.

Allowed for packing inputs: logic-only Dart/Rust changes. Changing Kotlin under `…/ota/` or wiring in `MainActivity` is a **host** change — ship via normal app CI / store release.

### Cut a release

```bash
# Option A: dispatch (builds SOs from the selected ref)
gh workflow run logic-ota.yml \
  -f logic_version=42 \
  -f host_line=kim-android-1 \
  -f min_host=1 \
  -f max_host=9999 \
  -f channel=dev \
  -f engine_build_id=3.47.2

# Option B: push a tag (uses defaults for host/channel/engine metadata)
git tag logic-ota-v42
git push origin logic-ota-v42
```

Required secret: `OTA_ED25519_PRIVATE_PEM` (full Ed25519 private PEM). Public key must match `sdk/mobile/android/app/src/main/assets/ota/ed25519_public.pem`.

## Rollout phases

| Phase | Scope |
|---|---|
| 0 (this PR) | Design, pack/verify scripts, schema, Android `OtaGate` (GitHub Releases catalog), cold-start hooks, CI build+publish Release. |
| 1 | Internal / `dev` channel + signed releases; crash metrics; arm64 only; no sqlite3 OTA. |
| 2 | Stable channel percentage rollout; stricter host_line discipline; optional `previous/` auto-rollback UX. |
| 3 (optional) | Consider additional allowlisted SOs only with new design review — not sqlite3 by default. |

## Known limitations

- **libapp.so**: Depends on Flutter 3.47 Android embedding accepting `--aot-shared-library-name=` under `filesDir`. Debug/JIT builds do not use AOT `libapp.so`; OTA AOT applies to **release/profile** AOT hosts. If validation rejects the path, client falls back to APK builtin.
- **FRB Native Assets**: OTA must win over the APK/native-assets copy. Bootstrap `System.load` + Dart `RustLib.init(externalLibrary: ExternalLibrary.open(otaPath))` (or `process()` after load).
- **No mid-session swap**: Install is atomic on disk; process restart required.
- **GitHub API rate limits**: Unauthenticated public API; 403/429 → soft fail (no install).
- **Workflow file**: If git push lacks `workflow` scope, install via credential with scope or `cp sdk/mobile/ota/logic-ota.workflow.yml .github/workflows/logic-ota.yml`.
