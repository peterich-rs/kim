#!/usr/bin/env bash
# Gate: SCAN login:loc:v2:* fail-closed.
# Exit 0 only when empty_jti=0 invalid=0 wrong_type=0.
# Prefers the running royal container (same decode as count_empty_jti_locations).
# Fallback: cargo example when REDIS_URL is set and rustc is available.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
compose=(docker compose -f "$root/deploy/compose.yml")
if [[ -f "$root/deploy/kim.env" ]]; then
  compose+=(--env-file "$root/deploy/kim.env")
fi

if "${compose[@]}" ps --status running --services 2>/dev/null | grep -qx royal; then
  exec "${compose[@]}" exec -T royal royal --scan-empty-jti
fi

if [[ -n "${REDIS_URL:-}" ]] && command -v cargo >/dev/null; then
  exec cargo run -q -p kim-session --features redis --example scan_empty_jti -- "$REDIS_URL"
fi

echo "royal container not running and REDIS_URL/cargo unavailable" >&2
exit 2
