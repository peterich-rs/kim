#!/usr/bin/env bash
# First-time VPS layout. Generates kim.env on the server and does not print it.
# Usage (on the VPS as root):
#   bash bootstrap.sh
set -euo pipefail

DEPLOY_DIR="${KIM_DEPLOY_DIR:-/opt/kim/deploy}"
IMAGE_DEFAULT="${KIM_IMAGE:-ghcr.io/peterich-rs/kim:latest}"

mkdir -p "$DEPLOY_DIR"
chmod 755 /opt/kim "$DEPLOY_DIR"

if [[ -f "$DEPLOY_DIR/kim.env" ]]; then
  echo "kim.env already exists; not rotating secrets"
  exit 0
fi

if ! command -v openssl >/dev/null 2>&1; then
  echo "error: openssl is required to generate secrets" >&2
  exit 1
fi

jwt="$(openssl rand -hex 32)"
pg="$(openssl rand -hex 24)"

umask 077
cat >"$DEPLOY_DIR/kim.env" <<EOF
KIM_IMAGE=${IMAGE_DEFAULT}
KIM_JWT_SECRET=${jwt}
POSTGRES_USER=kim
POSTGRES_PASSWORD=${pg}
POSTGRES_DB=kim
DATABASE_URL=postgres://kim:${pg}@postgres:5432/kim
REDIS_URL=redis://redis:6379/0
RUST_LOG=info
KIM_DOMAIN=kim.ainexc.com
CADDY_EMAIL=ops@example.com
ROYAL_URL=http://royal:8080
CONSUL_HTTP_ADDR=http://consul:8500
EOF
chmod 640 "$DEPLOY_DIR/kim.env"

echo "wrote $DEPLOY_DIR/kim.env (values not printed)"
