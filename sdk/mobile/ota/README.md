# Android Logic SO OTA tooling

Pack and verify **only** `libapp.so` + `libkim_client_ffi.so` (arm64-v8a). Design: [docs/mobile-android-so-ota.md](../../../docs/mobile-android-so-ota.md).

## Policy (short)

- Store APK is primary. OTA is a small hotfix channel for non-platform logic.
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
  --logic-version 20260905.1 \
  --channel dev

# Signed (CI):
OTA_SIGNING_KEY=/run/secrets/ota_ed25519.pem ./pack_logic_ota.sh ...
```

Without `OTA_SIGNING_KEY`, the script writes `UNSIGNED` for local dry-run.

## Verify

```bash
./verify_logic_ota.sh \
  --manifest ./dist/manifest.json \
  --zip ./dist/logic-ota-20260905.1-arm64-v8a.zip \
  --public-key ./dev_placeholder_ed25519_public.pem
```

## Placeholder public key

`dev_placeholder_ed25519_public.pem` (and `android/.../assets/ota/ed25519_public.pem`) is for **dev/test only**. Replace both with the production public key before a store release that enables the stable channel. Generate:

```bash
openssl genpkey -algorithm Ed25519 -out ota_ed25519_private.pem   # CI secret — do not commit
openssl pkey -in ota_ed25519_private.pem -pubout -out ed25519_public.pem
```

## Publish

1. Upload `logic-ota-*.zip`, `manifest.json`, `manifest.json.sig` to CDN.
2. Point Royal check API (see design doc) at the new logic_version for the matching `host_line` / channel.
3. Clients download on the next check; apply on the **following cold start**.

## CI workflow reference

GitHub App / OAuth tokens without the `workflow` scope cannot create
`.github/workflows/*.yml` via git push. The full workflow YAML lives at
[`logic-ota.workflow.yml`](logic-ota.workflow.yml). To enable it on the repo:

```bash
cp sdk/mobile/ota/logic-ota.workflow.yml .github/workflows/logic-ota.yml
git add .github/workflows/logic-ota.yml
git commit -m "ci: enable logic SO OTA pack workflow"
git push   # requires a credential with the workflow scope
```

Required secret (optional for UNSIGNED dry-run): `OTA_ED25519_PRIVATE_PEM`.
