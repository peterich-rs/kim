#!/bin/sh
# Idempotent: create per-service ACL policies and tokens. Management token
# stays on this job / the Consul agent — never in Chat/Gateway/Royal/Router env.
# Token create failures must exit non-zero so compose depends_on does not
# start app containers with missing ACL bindings.
set -eu

i=0
while [ "$i" -lt 30 ]; do
  if consul members >/dev/null 2>&1; then
    break
  fi
  i=$((i + 1))
  sleep 1
done
consul members >/dev/null

create_policy() {
  name=$1
  rules=$2
  if consul acl policy read -name "$name" >/dev/null 2>&1; then
    consul acl policy update -name "$name" -rules "$rules" >/dev/null
  else
    consul acl policy create -name "$name" -rules "$rules" >/dev/null
  fi
}

# Idempotent only when the secret already authenticates and lists `policy`.
# Any other failure (Consul down, mTLS, empty secret, create error) exits.
create_token() {
  secret=$1
  policy=$2
  if [ -z "$secret" ]; then
    echo "error: empty CONSUL token for policy ${policy}" >&2
    exit 1
  fi
  if CONSUL_HTTP_TOKEN="$secret" consul acl token read -self >/dev/null 2>&1; then
    desc=$(CONSUL_HTTP_TOKEN="$secret" consul acl token read -self -format=json)
    echo "$desc" | grep -q "\"Name\": *\"${policy}\"" || {
      echo "error: token exists but is not bound to policy ${policy}" >&2
      exit 1
    }
    return 0
  fi
  consul acl token create -secret "$secret" -policy-name "$policy" >/dev/null
}

create_policy chat '
service "chat" {
  policy = "write"
}
service_prefix "" {
  policy = "read"
}
node_prefix "" {
  policy = "read"
}
agent_prefix "" {
  policy = "read"
}
'

create_policy royal '
service "royal" {
  policy = "write"
}
service_prefix "" {
  policy = "read"
}
node_prefix "" {
  policy = "read"
}
agent_prefix "" {
  policy = "read"
}
'

create_policy gateway '
service "wgateway" {
  policy = "write"
}
service "chat" {
  policy = "read"
}
service_prefix "" {
  policy = "read"
}
node_prefix "" {
  policy = "read"
}
agent_prefix "" {
  policy = "read"
}
'

create_policy router '
service "router" {
  policy = "write"
}
service "wgateway" {
  policy = "read"
}
service "tgateway" {
  policy = "read"
}
node_prefix "" {
  policy = "read"
}
agent_prefix "" {
  policy = "read"
}
'

create_token "$CONSUL_TOKEN_CHAT" chat
create_token "$CONSUL_TOKEN_ROYAL" royal
create_token "$CONSUL_TOKEN_GATEWAY" gateway
create_token "$CONSUL_TOKEN_ROUTER" router

echo "consul acl ready"
