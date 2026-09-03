#!/usr/bin/env bash
# First-time VPS layout, and preflight for an existing one.
# Generates kim.env (once), Consul TLS, gossip encrypt key, and ACL secrets.
# Does not print secrets. Existing kim.env secret values are never rewritten.
# Usage (on the VPS as root):
#   bash bootstrap.sh
set -euo pipefail

DEPLOY_DIR="${KIM_DEPLOY_DIR:-/opt/kim/deploy}"
IMAGE_DEFAULT="${KIM_IMAGE:-ghcr.io/peterich-rs/kim:latest}"
CONSUL_IMAGE="${KIM_CONSUL_IMAGE:-hashicorp/consul:1.20}"
TLS_DIR="$DEPLOY_DIR/consul/tls"
SECRETS="$DEPLOY_DIR/consul/secrets.hcl"

mkdir -p "$TLS_DIR"
chmod 755 /opt/kim "$DEPLOY_DIR" "$DEPLOY_DIR/consul" "$TLS_DIR"

uuid() {
  if command -v uuidgen >/dev/null 2>&1; then
    uuidgen | tr '[:upper:]' '[:lower:]'
  else
    python3 -c 'import uuid; print(uuid.uuid4())'
  fi
}

require_docker() {
  if ! command -v docker >/dev/null 2>&1; then
    echo "error: docker is required to generate Consul TLS / gossip material" >&2
    exit 1
  fi
}

# Official hashicorp/consul image USER is consul (uid 100). tls ca/cert create
# writes $name-<uuid>.tmp then renames; that fails on a root:root 755 bind
# mount (CI chown -R root:root before this script). Run as the host user.
consul_tls() {
  docker run --rm --user "$(id -u):$(id -g)" \
    -v "$TLS_DIR:/out" -w /out "$CONSUL_IMAGE" consul tls "$@"
}

tls_complete() {
  [[ -f "$TLS_DIR/consul-agent-ca.pem" \
    && -f "$TLS_DIR/consul-server.pem" \
    && -f "$TLS_DIR/consul-server-key.pem" \
    && -f "$TLS_DIR/consul-client.pem" \
    && -f "$TLS_DIR/consul-client-key.pem" ]]
}

ensure_tls() {
  if tls_complete; then
    return 0
  fi
  require_docker
  if [[ ! -f "$TLS_DIR/consul-agent-ca.pem" ]]; then
    consul_tls ca create
  fi
  if [[ ! -f "$TLS_DIR/consul-server.pem" || ! -f "$TLS_DIR/consul-server-key.pem" ]]; then
    consul_tls cert create -server -dc dc1 \
      -additional-dnsname=consul \
      -additional-dnsname=localhost \
      -additional-ipaddress=127.0.0.1
    mv "$TLS_DIR/dc1-server-consul-0.pem" "$TLS_DIR/consul-server.pem"
    mv "$TLS_DIR/dc1-server-consul-0-key.pem" "$TLS_DIR/consul-server-key.pem"
  fi
  if [[ ! -f "$TLS_DIR/consul-client.pem" || ! -f "$TLS_DIR/consul-client-key.pem" ]]; then
    consul_tls cert create -client
    mv "$TLS_DIR/dc1-client-consul-0.pem" "$TLS_DIR/consul-client.pem"
    mv "$TLS_DIR/dc1-client-consul-0-key.pem" "$TLS_DIR/consul-client-key.pem"
  fi
  chmod 640 "$TLS_DIR/"*.pem
  if ! tls_complete; then
    echo "error: Consul TLS material incomplete under $TLS_DIR" >&2
    exit 1
  fi
  echo "wrote Consul TLS under $TLS_DIR (values not printed)"
}

gossip_keygen() {
  require_docker
  docker run --rm "$CONSUL_IMAGE" consul keygen | tr -d '\r\n'
}

write_secrets_hcl() {
  local gossip=$1
  local mgmt=$2
  umask 077
  cat >"$SECRETS" <<EOF
encrypt = "${gossip}"

acl {
  tokens {
    initial_management = "${mgmt}"
    agent              = "${mgmt}"
  }
}
EOF
  chmod 640 "$SECRETS"
}

secrets_have_encrypt() {
  [[ -f "$SECRETS" ]] && grep -qE '^[[:space:]]*encrypt[[:space:]]*=' "$SECRETS"
}

ensure_secrets_hcl() {
  local mgmt=${CONSUL_MANAGEMENT_TOKEN:-}
  if [[ -z "$mgmt" ]]; then
    echo "error: CONSUL_MANAGEMENT_TOKEN is required to write $SECRETS" >&2
    exit 1
  fi
  if secrets_have_encrypt; then
    chmod 640 "$SECRETS"
    return 0
  fi
  local gossip
  gossip="$(gossip_keygen)"
  if [[ -z "$gossip" ]]; then
    echo "error: consul keygen produced an empty gossip key" >&2
    exit 1
  fi
  write_secrets_hcl "$gossip" "$mgmt"
  echo "wrote $SECRETS (gossip encrypt + ACL; values not printed)"
}

preflight_kim_env() {
  local missing=()
  local k
  for k in \
    KIM_ENV \
    KIM_JWT_SECRET \
    KIM_INTERNAL_HMAC_SECRET \
    REDIS_PASSWORD \
    REDIS_URL \
    CONSUL_HTTP_ADDR \
    CONSUL_MANAGEMENT_TOKEN \
    CONSUL_TOKEN_CHAT \
    CONSUL_TOKEN_ROYAL \
    CONSUL_TOKEN_GATEWAY \
    CONSUL_TOKEN_ROUTER
  do
    if [[ -z "${!k:-}" ]]; then
      missing+=("$k")
    fi
  done
  if (( ${#missing[@]} > 0 )); then
    echo "error: $DEPLOY_DIR/kim.env is missing: ${missing[*]}" >&2
    echo "error: append the keys by hand; this script does not rewrite existing kim.env secrets" >&2
    exit 1
  fi
}

ensure_tls

ENV_FILE="$DEPLOY_DIR/kim.env"
if [[ -f "$ENV_FILE" ]]; then
  echo "kim.env already exists; not rotating secrets"
  set -a
  # shellcheck disable=SC1090
  source "$ENV_FILE"
  set +a
  preflight_kim_env
  ensure_secrets_hcl
  if ! tls_complete || ! secrets_have_encrypt; then
    echo "error: Consul TLS or secrets.hcl still incomplete after preflight" >&2
    exit 1
  fi
  echo "preflight ok (TLS, gossip encrypt, kim.env keys)"
  exit 0
fi

if ! command -v openssl >/dev/null 2>&1; then
  echo "error: openssl is required to generate secrets" >&2
  exit 1
fi

jwt="$(openssl rand -hex 32)"
hmac="$(openssl rand -hex 32)"
pg="$(openssl rand -hex 24)"
redis_pass="$(openssl rand -hex 24)"
mgmt="$(uuid)"
token_chat="$(uuid)"
token_royal="$(uuid)"
token_gateway="$(uuid)"
token_router="$(uuid)"
CONSUL_MANAGEMENT_TOKEN="$mgmt"
ensure_secrets_hcl

umask 077
cat >"$ENV_FILE" <<EOF
KIM_IMAGE=${IMAGE_DEFAULT}
KIM_ENV=production
KIM_JWT_SECRET=${jwt}
KIM_INTERNAL_HMAC_SECRET=${hmac}
POSTGRES_USER=kim
POSTGRES_PASSWORD=${pg}
POSTGRES_DB=kim
DATABASE_URL=postgres://kim:${pg}@postgres:5432/kim
REDIS_PASSWORD=${redis_pass}
REDIS_URL=redis://:${redis_pass}@redis:6379/0
RUST_LOG=info
KIM_DOMAIN=kim.ainexc.com
CADDY_EMAIL=ops@example.com
ROYAL_URL=http://royal:8080
CONSUL_HTTP_ADDR=https://consul:8501
CONSUL_MANAGEMENT_TOKEN=${mgmt}
CONSUL_TOKEN_CHAT=${token_chat}
CONSUL_TOKEN_ROYAL=${token_royal}
CONSUL_TOKEN_GATEWAY=${token_gateway}
CONSUL_TOKEN_ROUTER=${token_router}
EOF
chmod 640 "$ENV_FILE"

echo "wrote $ENV_FILE (values not printed)"
