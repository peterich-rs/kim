# Android Logic SO OTA tooling

Pack and verify **only** `libapp.so` + `libkim_client_ffi.so` (arm64-v8a). Design: [docs/mobile-android-so-ota.md](../../../docs/mobile-android-so-ota.md).

**Catalog = GitHub Releases** on `peterich-rs/kim` (tags `logic-ota-v*`). No Royal check API.

## Policy (short)

- Store APK is primary. OTA is a small hotfix channel for non-platform logic.
- Always ship **both** SOs together (atomic pair).
- Never pack `libflutter.so`, plugins, Java/Kotlin, resources, sqlite3 (phase 1).
- SHA-256 + Ed25519-signed `manifest.json`. Public key in the app; private key CI-only.

## Pack

```bash
# Build release SOs on a logic-only change, then:
./pack_logic_ota.sh \
  --libapp /path/to/libapp.so \
  --libffi /path/to/libkim_client_ffi.so \
  --out-dir ./dist \
  --host-line kim-android-1 \
  --min-host 1 \
  --max-host 99 \
  --engine-build-id 3.47.2 \
  --logic-version 42 \
  --channel dev

# Signed (CI):
OTA_SIGNING_KEY=/run/secrets/ota_ed25519.pem ./pack_logic_ota.sh ...
```

Without `OTA_SIGNING_KEY`, the script writes `UNSIGNED` for local dry-run.

## Verify

```bash
./verify_logic_ota.sh \
  --manifest ./dist/manifest.json \
  --zip ./dist/logic-ota-42-arm64-v8a.zip \
  --public-key ./dev_placeholder_ed25519_public.pem
```

## Placeholder public key

`dev_placeholder_ed25519_public.pem` (and `android/.../assets/ota/ed25519_public.pem`) is for **dev/test only**. Replace both with the production public key before a store release that enables the stable channel. Generate:

```bash
openssl genpkey -algorithm Ed25519 -out ota_ed25519_private.pem   # CI secret — do not commit
openssl pkey -in ota_ed25519_private.pem -pubout -out ed25519_public.pem
```

## Publish (GitHub Release)

CI workflow builds the arm64 release APK, extracts both SOs, packs/signs, and creates a GitHub Release:

- Tag: `logic-ota-v{logic_version}`
- Assets: `logic-ota-*-arm64-v8a.zip`, `manifest.json`, `manifest.json.sig`

Clients discover updates with:

```http
GET https://api.github.com/repos/peterich-rs/kim/releases?per_page=30
User-Agent: KimMobileOTA/1.0
Accept: application/vnd.github+json
```

### Cut a release

```bash
gh workflow run logic-ota.yml \
  -f logic_version=42 \
  -f host_line=kim-android-1 \
  -f min_host=1 \
  -f max_host=9999 \
  -f channel=dev \
  -f engine_build_id=3.47.2

# or
git tag logic-ota-v42 && git push origin logic-ota-v42
```

Required secret: `OTA_ED25519_PRIVATE_PEM`.

## CI workflow

Production YAML:

- [`.github/workflows/logic-ota.yml`](../../../.github/workflows/logic-ota.yml) (preferred install path)
- Mirror: [`logic-ota.workflow.yml`](logic-ota.workflow.yml)

If `git push` rejects workflow files (OAuth missing `workflow` scope):

```bash
cp sdk/mobile/ota/logic-ota.workflow.yml .github/workflows/logic-ota.yml
git add .github/workflows/logic-ota.yml
git commit -m "ci: enable logic SO OTA build+publish workflow"
git push   # requires a credential with the workflow scope
```

Or create via API (same scope requirement):

```bash
# after committing the mirror; see docs for gh api contents PUT
```
