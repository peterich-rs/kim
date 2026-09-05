# Android Logic SO OTA（hotfix channel）

App-store APK is **primary**. This channel ships **small Android-only hotfixes of non-platform logic** by swapping two arm64-v8a shared objects at cold start. It is not a general in-app updater, not an iOS feature, and not a substitute for Play / store releases when platform or resources change.

Related: [mobile-client.md](mobile-client.md), tooling under [`sdk/mobile/ota/`](../sdk/mobile/ota/).

## Policy

| Rule | Detail |
|---|---|
| Primary distribution | Store APK / AAB. Users always have a working builtin engine + Dart AOT + FFI. |
| OTA allowed | Only `libapp.so` (Dart AOT) + `libkim_client_ffi.so` (Rust FRB FFI), `abi=arm64-v8a`. |
| OTA forbidden | `libflutter.so`, Java/Kotlin, resources, Manifest, Gradle, plugins, permissions, `libsqlite3.so` (phase 1), assets, JNI bridges other than the two SOs above. |
| Platform / resource change | Cut a **full app release** and bump `host_line`. Do not attempt SO OTA. |
| Integrity | SHA-256 of every artifact + **Ed25519** signature over canonical `manifest.json` bytes. Not MD5. |
| Keys | Public key embedded in the APK (`assets/ota/ed25519_public.pem`). Private key is **CI-only** (GitHub Actions secret). Never commit private keys. |
| Scope | Android only. No iOS OTA. |

## Boundaries (what “logic” means)

**Logic (OTA-eligible):** Dart UI/business compiled into `libapp.so`, and Rust `kim-client` FFI in `libkim_client_ffi.so`, when both stay ABI-compatible with the installed host APK.

**Platform (full release + new `host_line`):** Flutter engine (`libflutter.so`), Android embedding / Activity / Application Kotlin, Gradle, Manifest permissions, plugins (camera, secure storage, …), resources/themes, sqlite3 native, anything that changes the JNI / plugin registrant surface.

If a PR touches both logic and platform, ship a store build. CI path filters for the logic-ota workflow must fail closed on platform paths.

## Version mapping

| Field | Meaning |
|---|---|
| `host_line` | Compatibility line of the store APK (e.g. `kim-android-1`). Bump when platform/engine/plugins change. |
| `min_host_version_code` / `max_host_version_code` | Inclusive `versionCode` window that may apply this logic package. |
| `engine_build_id` | Flutter / engine identity the AOT was built against (e.g. `3.47.2`). Must match host. |
| `logic_version` | Monotonic string/int for the logic package (e.g. `20260905.1`). |
| `abi` | Always `arm64-v8a` in phase 1. |
| `channel` | `stable` / `beta` / `dev`. Client sends its channel; server filters. |

### Mapping examples

| Host APK | `host_line` | `versionCode` | `engine_build_id` | Eligible logic |
|---|---|---|---|---|
| Play 1.0.0+12 | `kim-android-1` | 12 | `3.47.2` | manifests with `host_line=kim-android-1`, `min≤12≤max`, same engine |
| Play 1.1.0+20 after plugin bump | `kim-android-2` | 20 | `3.47.2` | only `kim-android-2` packages; old line ignored |
| Engine bump to 3.48 | `kim-android-3` | 30 | `3.48.0` | new line; never apply 3.47 AOT onto 3.48 host |

Client rejects a package when any of: wrong `host_line`, `versionCode` outside window, `engine_build_id` mismatch, wrong `abi` / `channel`, failed SHA-256, failed Ed25519, or crash-loop marker from a prior OTA boot.

## Artifact zip layout

Published objects (CDN or Royal static):

```text
logic-ota-<logic_version>-arm64-v8a.zip
  ├── libapp.so
  └── libkim_client_ffi.so
manifest.json                 # sibling, not inside the zip
manifest.json.sig            # Ed25519 over exact manifest.json bytes
```

`manifest.json` lists SHA-256 for each SO **and** for the zip. Client downloads zip + manifest + sig, verifies sig → parses manifest → checks host mapping → verifies hashes → stages files.

## Manifest schema

See [`sdk/mobile/ota/manifest.schema.json`](../sdk/mobile/ota/manifest.schema.json) and [`manifest.example.json`](../sdk/mobile/ota/manifest.example.json).

Canonical signing input: UTF-8 bytes of `manifest.json` as published (no re-serialization). Signature is raw 64-byte Ed25519 (OpenSSL `pkeyutl -sign -rawin`), distributed as binary `manifest.json.sig` or base64 in CDN metadata — client tooling accepts the binary file next to the manifest.

## Check API contract (HTTP GET stub)

Royal / CDN may implement this later. Client only needs a stable GET.

```http
GET {base}/v1/logic-ota/check
  ?host_line=kim-android-1
  &host_version_code=12
  &engine_build_id=3.47.2
  &logic_version=20260901.0
  &abi=arm64-v8a
  &channel=stable
```

**200 — no update**

```json
{ "update": null }
```

**200 — update available**

```json
{
  "update": {
    "host_line": "kim-android-1",
    "min_host_version_code": 10,
    "max_host_version_code": 19,
    "engine_build_id": "3.47.2",
    "logic_version": "20260905.1",
    "abi": "arm64-v8a",
    "channel": "stable",
    "zip_url": "https://cdn.example/ota/logic-ota-20260905.1-arm64-v8a.zip",
    "zip_sha256": "…",
    "manifest_url": "https://cdn.example/ota/manifest-20260905.1.json",
    "signature_url": "https://cdn.example/ota/manifest-20260905.1.json.sig"
  }
}
```

Errors: non-2xx → treat as no update (fail soft). Client never installs without verifying the signed manifest body from `manifest_url` (do not trust check JSON hashes alone for install).

Default check base URL is configurable via `OtaConfig` / BuildConfig (`https://kim.ainexc.com` stub path `/api/v1/logic-ota/check`).

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
6. Background: `check(url)` → download → verify → atomic promote `staging → current` (and shift old current → previous). **Apply on next cold start** (no mid-session SO swap).

### Rollback

- Automatic: crash marker → next launch drops `current`.
- Manual / future: restore `previous/` → `current/` if hashes still match embedded policy.

## Security

- HTTPS only for check + CDN.
- Ed25519 verify with embedded public key; reject if sig missing or wrong.
- SHA-256 over file bytes before promote.
- Refuse install if zip contains unexpected members (path traversal, extra `.so`, non-allowlisted names).
- Private key never in APK or git; rotate by shipping a new host APK with a new public key (`host_line` bump).

## CI path whitelist

Logic-ota pack workflow (`.github/workflows/logic-ota.yml`) may run only when the diff is logic-safe. **Fail** if the PR/push touches:

- `sdk/mobile/android/**` (except documented ota tooling if ever colocated — app Kotlin gate lives in android but pack workflow is for SO artifacts, not for changing the gate)
- `sdk/mobile/ios/**`
- `sdk/mobile/plugins/**`
- Gradle / Manifest / engine pins that imply platform change

Allowed inputs for packing: prebuilt `libapp.so` + `libkim_client_ffi.so` from a logic-only Flutter/Rust build job, plus version env vars. See [`sdk/mobile/ota/README.md`](../sdk/mobile/ota/README.md).

> Note: Changing Kotlin under `…/ota/` or wiring in `MainActivity` is a **host** change — ship via normal app CI / store release, not via the logic-ota artifact workflow.

## Rollout phases

| Phase | Scope |
|---|---|
| 0 (this PR) | Design, pack/verify scripts, schema, Android `OtaGate`, cold-start hooks, debug MethodChannel, no Royal server required. |
| 1 | Internal channel + signed CDN objects; crash metrics; arm64 only; no sqlite3 OTA. |
| 2 | Stable channel percentage rollout; stricter host_line discipline; optional `previous/` auto-rollback UX. |
| 3 (optional) | Consider additional allowlisted SOs only with new design review — not sqlite3 by default. |

## Known limitations

- **libapp.so**: Depends on Flutter 3.47 Android embedding accepting `--aot-shared-library-name=` under `filesDir`. Debug/JIT builds do not use AOT `libapp.so`; OTA AOT applies to **release/profile** AOT hosts. If validation rejects the path, client falls back to APK builtin.
- **FRB Native Assets**: OTA must win over the APK/native-assets copy. Bootstrap `System.load` + Dart `RustLib.init(externalLibrary: ExternalLibrary.open(otaPath))` (or `process()` after load).
- **No mid-session swap**: Install is atomic on disk; process restart required.
- **Server**: Check API is documented only; stub URL may 404 → no update.
