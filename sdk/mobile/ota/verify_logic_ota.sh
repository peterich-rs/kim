#!/usr/bin/env bash
# Verify logic OTA zip against manifest + Ed25519 signature.
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage:
  verify_logic_ota.sh --manifest PATH --zip PATH --public-key PATH [--signature PATH]

Defaults:
  --signature  <manifest>.sig
USAGE
}

MANIFEST=""
ZIP=""
PUBKEY=""
SIG=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --manifest) MANIFEST="$2"; shift 2 ;;
    --zip) ZIP="$2"; shift 2 ;;
    --public-key) PUBKEY="$2"; shift 2 ;;
    --signature) SIG="$2"; shift 2 ;;
    -h|--help) usage; exit 0 ;;
    *) echo "unknown arg: $1" >&2; usage; exit 2 ;;
  esac
done

if [[ -z "$MANIFEST" || -z "$ZIP" || -z "$PUBKEY" ]]; then
  usage
  exit 2
fi
SIG="${SIG:-${MANIFEST}.sig}"

for f in "$MANIFEST" "$ZIP" "$PUBKEY" "$SIG"; do
  if [[ ! -f "$f" ]]; then
    echo "missing file: $f" >&2
    exit 2
  fi
done

sha256_file() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  else
    shasum -a 256 "$1" | awk '{print $1}'
  fi
}

# Signature over exact manifest bytes.
if ! openssl pkeyutl -verify -pubin -inkey "$PUBKEY" -rawin -in "$MANIFEST" -sigfile "$SIG" >/dev/null; then
  echo "Ed25519 signature verification FAILED" >&2
  exit 1
fi
echo "signature: OK"

ZIP_SHA_EXPECT=$(python3 - <<PY
import json
print(json.load(open("$MANIFEST"))["zip_sha256"])
PY
)
ZIP_SHA_ACTUAL=$(sha256_file "$ZIP")
if [[ "$ZIP_SHA_EXPECT" != "$ZIP_SHA_ACTUAL" ]]; then
  echo "zip sha256 mismatch: expect $ZIP_SHA_EXPECT got $ZIP_SHA_ACTUAL" >&2
  exit 1
fi
echo "zip sha256: OK"

STAGE=$(mktemp -d)
trap 'rm -rf "$STAGE"' EXIT
unzip -q -o "$ZIP" -d "$STAGE"

python3 - <<PY
import hashlib, json, os, sys
manifest = json.load(open("$MANIFEST"))
stage = "$STAGE"
allowed = {"libapp.so", "libkim_client_ffi.so"}
names = set(os.listdir(stage))
if names != allowed:
    print(f"zip members {names} != {allowed}", file=sys.stderr)
    sys.exit(1)
by_name = {a["name"]: a for a in manifest["artifacts"]}
for name in sorted(allowed):
    path = os.path.join(stage, name)
    data = open(path, "rb").read()
    digest = hashlib.sha256(data).hexdigest()
    meta = by_name[name]
    if digest != meta["sha256"]:
        print(f"{name} sha256 mismatch", file=sys.stderr)
        sys.exit(1)
    if len(data) != meta["size"]:
        print(f"{name} size mismatch", file=sys.stderr)
        sys.exit(1)
    print(f"{name}: OK")
if manifest.get("abi") != "arm64-v8a":
    print("abi must be arm64-v8a", file=sys.stderr)
    sys.exit(1)
print("verify: OK")
PY
