#!/usr/bin/env bash
# First-time VPS layout, and preflight for an existing one.
# Generates kim.env (once), Consul TLS, gossip encrypt key, and ACL secrets.
# Does not print secrets. Existing kim.env secret values are never rewritten;
# missing required keys are appended (names only in logs).
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

# docker-entrypoint.sh su-execs to USER consul (uid 100) even when docker
# --user is root, so --user cannot write $name-<uuid>.tmp into a root:root
# 755 bind mount. Skip the entrypoint; open the dir for the generate window.
consul_tls() {
  docker run --rm --user consul --entrypoint /bin/consul \
    -v "$TLS_DIR:/out" -w /out "$CONSUL_IMAGE" tls "$@"
}

tls_complete() {
  [[ -f "$TLS_DIR/consul-agent-ca.pem" \
    && -f "$TLS_DIR/consul-server.pem" \
    && -f "$TLS_DIR/consul-server-key.pem" \
    && -f "$TLS_DIR/consul-client.pem" \
    && -f "$TLS_DIR/consul-client-key.pem" ]]
}

# Official consul image USER is uid 100. CI `chown -R root:root` plus chmod 640
# would make secrets.hcl / TLS keys unreadable and the agent never healthy.
ensure_consul_readable() {
  local uid=100
  chmod 755 "$TLS_DIR"
  chown -R "$uid:$uid" "$TLS_DIR" || true
  chmod 644 "$TLS_DIR"/*.pem 2>/dev/null || true
  chmod 640 "$TLS_DIR"/*-key.pem 2>/dev/null || true
  if [[ -f "$SECRETS" ]]; then
    chown "$uid:$uid" "$SECRETS" || true
    chmod 640 "$SECRETS"
  fi
}

ensure_tls() {
  if ! tls_complete; then
    require_docker
    chmod 0777 "$TLS_DIR"
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
    echo "wrote Consul TLS under $TLS_DIR (values not printed)"
  fi
  if ! tls_complete; then
    echo "error: Consul TLS material incomplete under $TLS_DIR" >&2
    exit 1
  fi
  ensure_consul_readable
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
    ensure_consul_readable
    return 0
  fi
  local gossip
  gossip="$(gossip_keygen)"
  if [[ -z "$gossip" ]]; then
    echo "error: consul keygen produced an empty gossip key" >&2
    exit 1
  fi
  write_secrets_hcl "$gossip" "$mgmt"
  ensure_consul_readable
  echo "wrote $SECRETS (gossip encrypt + ACL; values not printed)"
}

require_openssl() {
  if ! command -v openssl >/dev/null 2>&1; then
    echo "error: openssl is required to generate secrets" >&2
    exit 1
  fi
}

append_env() {
  local k=$1
  local v=$2
  umask 077
  printf '%s=%s\n' "$k" "$v" >>"$ENV_FILE"
  chmod 640 "$ENV_FILE"
  printf -v "$k" '%s' "$v"
}

# Replace or append KEY=value. Used when REDIS_URL exists but has no password.
set_env_key() {
  local k=$1
  local v=$2
  local tmp
  umask 077
  tmp="$(mktemp)"
  grep -vE "^${k}=" "$ENV_FILE" >"$tmp" || true
  printf '%s=%s\n' "$k" "$v" >>"$tmp"
  mv "$tmp" "$ENV_FILE"
  chmod 640 "$ENV_FILE"
  printf -v "$k" '%s' "$v"
}

redis_password_from_url() {
  local url=${1:-}
  if [[ "$url" =~ ^rediss?://:([^@/]+)@ ]]; then
    printf '%s' "${BASH_REMATCH[1]}"
    return 0
  fi
  return 1
}

postgres_password_from_url() {
  local url=${1:-}
  if [[ "$url" =~ ^postgres(ql)?://[^:/@]+:([^@/]+)@ ]]; then
    printf '%s' "${BASH_REMATCH[2]}"
    return 0
  fi
  return 1
}

# redis://host:6379/0 → redis://:PASS@host:6379/0
redis_url_with_password() {
  local url=${1:-}
  local pass=$2
  if [[ "$url" =~ ^(rediss?://)([^@]+)$ ]]; then
    printf '%s:%s@%s' "${BASH_REMATCH[1]}" "$pass" "${BASH_REMATCH[2]}"
    return 0
  fi
  return 1
}

mgmt_from_secrets_hcl() {
  if [[ ! -f "$SECRETS" ]]; then
    return 1
  fi
  local val
  val="$(sed -n 's/^[[:space:]]*initial_management[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' "$SECRETS" | tail -n1)"
  if [[ -z "$val" ]]; then
    return 1
  fi
  printf '%s' "$val"
}

# Append only empty required keys. Never overwrite a non-empty value.
fill_missing_kim_env() {
  local added=()
  local k v

  if [[ -z "${KIM_ENV:-}" ]]; then
    append_env KIM_ENV production
    added+=(KIM_ENV)
  fi
  if [[ -z "${CONSUL_HTTP_ADDR:-}" ]]; then
    append_env CONSUL_HTTP_ADDR "https://consul:8501"
    added+=(CONSUL_HTTP_ADDR)
  fi
  if [[ -z "${REDIS_PASSWORD:-}" ]]; then
    if v="$(redis_password_from_url "${REDIS_URL:-}")"; then
      append_env REDIS_PASSWORD "$v"
      added+=(REDIS_PASSWORD)
    else
      require_openssl
      v="$(openssl rand -hex 24)"
      append_env REDIS_PASSWORD "$v"
      added+=(REDIS_PASSWORD)
      if [[ -n "${REDIS_URL:-}" ]]; then
        local new_url
        if ! new_url="$(redis_url_with_password "$REDIS_URL" "$v")"; then
          echo "error: REDIS_URL has no password and cannot be rewritten" >&2
          exit 1
        fi
        set_env_key REDIS_URL "$new_url"
        added+=(REDIS_URL)
      fi
    fi
  fi
  if [[ -z "${REDIS_URL:-}" ]]; then
    append_env REDIS_URL "redis://:${REDIS_PASSWORD}@redis:6379/0"
    added+=(REDIS_URL)
  fi
  if [[ -z "${POSTGRES_PASSWORD:-}" ]]; then
    if v="$(postgres_password_from_url "${DATABASE_URL:-}")"; then
      append_env POSTGRES_PASSWORD "$v"
      added+=(POSTGRES_PASSWORD)
    else
      echo "error: POSTGRES_PASSWORD is empty and DATABASE_URL has no password to copy" >&2
      echo "error: set POSTGRES_PASSWORD to the running Postgres password; will not mint a new one" >&2
      exit 1
    fi
  fi
  if [[ -z "${DATABASE_URL:-}" ]]; then
    local user=${POSTGRES_USER:-kim}
    local db=${POSTGRES_DB:-kim}
    append_env DATABASE_URL "postgres://${user}:${POSTGRES_PASSWORD}@postgres:5432/${db}"
    added+=(DATABASE_URL)
  fi
  if [[ -z "${KIM_IMAGE:-}" ]]; then
    append_env KIM_IMAGE "$IMAGE_DEFAULT"
    added+=(KIM_IMAGE)
  fi
  if [[ -z "${KIM_JWT_SECRET:-}" ]]; then
    require_openssl
    append_env KIM_JWT_SECRET "$(openssl rand -hex 32)"
    added+=(KIM_JWT_SECRET)
  fi
  if [[ -z "${KIM_INTERNAL_HMAC_SECRET:-}" ]]; then
    require_openssl
    append_env KIM_INTERNAL_HMAC_SECRET "$(openssl rand -hex 32)"
    added+=(KIM_INTERNAL_HMAC_SECRET)
  fi
  if [[ -z "${CONSUL_MANAGEMENT_TOKEN:-}" ]]; then
    if v="$(mgmt_from_secrets_hcl)"; then
      append_env CONSUL_MANAGEMENT_TOKEN "$v"
    else
      append_env CONSUL_MANAGEMENT_TOKEN "$(uuid)"
    fi
    added+=(CONSUL_MANAGEMENT_TOKEN)
  fi
  for k in CONSUL_TOKEN_CHAT CONSUL_TOKEN_ROYAL CONSUL_TOKEN_GATEWAY CONSUL_TOKEN_ROUTER; do
    if [[ -z "${!k:-}" ]]; then
      append_env "$k" "$(uuid)"
      added+=("$k")
    fi
  done

  if (( ${#added[@]} > 0 )); then
    echo "appended missing kim.env keys (values not printed): ${added[*]}"
  fi
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
    POSTGRES_PASSWORD \
    DATABASE_URL \
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
  fill_missing_kim_env
  preflight_kim_env
  ensure_secrets_hcl
  if ! tls_complete || ! secrets_have_encrypt; then
    echo "error: Consul TLS or secrets.hcl still incomplete after preflight" >&2
    exit 1
  fi
  echo "preflight ok (TLS, gossip encrypt, kim.env keys)"
  exit 0
fi

require_openssl

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
