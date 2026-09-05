#!/usr/bin/env bash
# Pack Android logic SO OTA: libapp.so + libkim_client_ffi.so → zip + manifest (+ optional Ed25519 sig).
# Patch-only dynamic semver: logic_version=x.y.z → host_line=x.y (auto).
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage:
  pack_logic_ota.sh --libapp PATH --libffi PATH --out-dir DIR \
    --logic-version x.y.z --engine-build-id STR \
    [--channel stable|beta|dev] [--host-line x.y]

  --logic-version   Required. Full semver x.y.z (release tag suffix).
  --host-line       Optional. Defaults to MAJOR.MINOR of --logic-version.
                    Override only for tests; production always derives.
  --engine-build-id Required. Flutter version used to build the SOs (.fvmrc).

Env:
  OTA_SIGNING_KEY   Path to Ed25519 private key PEM (OpenSSL). If set, writes manifest.json.sig.
                    If unset, writes UNSIGNED note for local dry-run.

Outputs in --out-dir:
  logic-ota-<logic_version>-arm64-v8a.zip
  manifest.json
  manifest.json.sig          (only when OTA_SIGNING_KEY is set)
  UNSIGNED                   (only when signing key is absent)
USAGE
}

LIBAPP=""
LIBFFI=""
OUT_DIR=""
HOST_LINE=""
ENGINE_BUILD_ID=""
LOGIC_VERSION=""
CHANNEL="dev"
ABI="arm64-v8a"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --libapp) LIBAPP="$2"; shift 2 ;;
    --libffi) LIBFFI="$2"; shift 2 ;;
    --out-dir) OUT_DIR="$2"; shift 2 ;;
    --host-line) HOST_LINE="$2"; shift 2 ;;
    --engine-build-id) ENGINE_BUILD_ID="$2"; shift 2 ;;
    --logic-version) LOGIC_VERSION="$2"; shift 2 ;;
    --channel) CHANNEL="$2"; shift 2 ;;
    --min-host|--max-host)
      echo "removed: $1 (patch-only semver uses host_line=x.y + logic_version)" >&2
      exit 2
      ;;
    -h|--help) usage; exit 0 ;;
    *) echo "unknown arg: $1" >&2; usage; exit 2 ;;
  esac
done

for req in LIBAPP LIBFFI OUT_DIR ENGINE_BUILD_ID LOGIC_VERSION; do
  if [[ -z "${!req}" ]]; then
    echo "missing required: $req" >&2
    usage
    exit 2
  fi
done

if [[ ! "$LOGIC_VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "logic_version must be x.y.z, got: $LOGIC_VERSION" >&2
  exit 2
fi

DERIVED_HOST_LINE="${LOGIC_VERSION%.*}"
if [[ -z "$HOST_LINE" ]]; then
  HOST_LINE="$DERIVED_HOST_LINE"
elif [[ "$HOST_LINE" != "$DERIVED_HOST_LINE" ]]; then
  echo "warning: --host-line $HOST_LINE != derived $DERIVED_HOST_LINE (test override)" >&2
fi

if [[ ! "$HOST_LINE" =~ ^[0-9]+\.[0-9]+$ ]]; then
  echo "host_line must be x.y, got: $HOST_LINE" >&2
  exit 2
fi

if [[ ! -f "$LIBAPP" || ! -f "$LIBFFI" ]]; then
  echo "libapp / libffi paths must exist" >&2
  exit 2
fi

mkdir -p "$OUT_DIR"
STAGE=$(mktemp -d)
trap 'rm -rf "$STAGE"' EXIT

cp "$LIBAPP" "$STAGE/libapp.so"
cp "$LIBFFI" "$STAGE/libkim_client_ffi.so"

ZIP_NAME="logic-ota-${LOGIC_VERSION}-${ABI}.zip"
ZIP_PATH="$OUT_DIR/$ZIP_NAME"
rm -f "$ZIP_PATH"
if command -v zip >/dev/null 2>&1; then
  ( cd "$STAGE" && zip -9 -X "$ZIP_PATH" libapp.so libkim_client_ffi.so )
else
  python3 -c "import zipfile; from pathlib import Path; s=Path(r\"$STAGE\"); z=Path(r\"$ZIP_PATH\");
zf=zipfile.ZipFile(z,\"w\",compression=zipfile.ZIP_DEFLATED);
[zf.write(s/n, arcname=n) for n in (\"libapp.so\",\"libkim_client_ffi.so\")]; zf.close()"
fi

sha256_file() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  else
    shasum -a 256 "$1" | awk '{print $1}'
  fi
}

APP_SHA=$(sha256_file "$STAGE/libapp.so")
FFI_SHA=$(sha256_file "$STAGE/libkim_client_ffi.so")
ZIP_SHA=$(sha256_file "$ZIP_PATH")
APP_SIZE=$(wc -c <"$STAGE/libapp.so" | tr -d ' ')
FFI_SIZE=$(wc -c <"$STAGE/libkim_client_ffi.so" | tr -d ' ')
CREATED=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

MANIFEST="$OUT_DIR/manifest.json"
# Compact-ish but stable field order for humans; signature covers these exact bytes.
cat >"$MANIFEST" <<JSON
{
  "schema_version": 1,
  "host_line": "${HOST_LINE}",
  "engine_build_id": "${ENGINE_BUILD_ID}",
  "logic_version": "${LOGIC_VERSION}",
  "abi": "${ABI}",
  "channel": "${CHANNEL}",
  "created_at": "${CREATED}",
  "zip_sha256": "${ZIP_SHA}",
  "artifacts": [
    {
      "name": "libapp.so",
      "sha256": "${APP_SHA}",
      "size": ${APP_SIZE}
    },
    {
      "name": "libkim_client_ffi.so",
      "sha256": "${FFI_SHA}",
      "size": ${FFI_SIZE}
    }
  ]
}
JSON

rm -f "$OUT_DIR/manifest.json.sig" "$OUT_DIR/UNSIGNED"
if [[ -n "${OTA_SIGNING_KEY:-}" ]]; then
  if [[ ! -f "$OTA_SIGNING_KEY" ]]; then
    echo "OTA_SIGNING_KEY not a file: $OTA_SIGNING_KEY" >&2
    exit 2
  fi
  openssl pkeyutl -sign -inkey "$OTA_SIGNING_KEY" -rawin -in "$MANIFEST" -out "$OUT_DIR/manifest.json.sig"
  echo "signed: $OUT_DIR/manifest.json.sig"
else
  cat >"$OUT_DIR/UNSIGNED" <<'NOTE'
UNSIGNED local dry-run.

Set OTA_SIGNING_KEY to an Ed25519 private key PEM to produce manifest.json.sig.
Release signing keys are CI secrets only; never commit private keys.
NOTE
  echo "warning: no OTA_SIGNING_KEY — wrote UNSIGNED" >&2
fi

echo "wrote $ZIP_PATH"
echo "wrote $MANIFEST"
echo "host_line=$HOST_LINE"
echo "zip_sha256=$ZIP_SHA"
