#!/usr/bin/env bash
# Runs on the VPS. Env from the deploy job: IMAGE, GHCR_USER, GHCR_TOKEN.
# Does not print secrets. Does not write GHCR_TOKEN to disk.
set -euo pipefail

DEPLOY_DIR="${KIM_DEPLOY_DIR:-/opt/kim/deploy}"
cd "$DEPLOY_DIR"

if [[ ! -f compose.yml ]]; then
  echo "error: missing $DEPLOY_DIR/compose.yml" >&2
  exit 1
fi

if [[ ! -f kim.env ]]; then
  echo "error: missing $DEPLOY_DIR/kim.env (bootstrap first)" >&2
  exit 1
fi

if [[ -z "${IMAGE:-}" ]]; then
  IMAGE="$(sed -n 's/^KIM_IMAGE=//p' kim.env | tail -n1)"
fi
if [[ -z "${IMAGE:-}" ]]; then
  echo "error: IMAGE is empty (set IMAGE or KIM_IMAGE in kim.env)" >&2
  exit 1
fi

umask 077
tmp="$(mktemp)"
grep -v '^KIM_IMAGE=' kim.env >"$tmp" || true
printf 'KIM_IMAGE=%s\n' "$IMAGE" >>"$tmp"
mv "$tmp" kim.env
chmod 640 kim.env

if [[ -n "${GHCR_TOKEN:-}" ]]; then
  printf '%s\n' "$GHCR_TOKEN" | docker login ghcr.io -u "$GHCR_USER" --password-stdin
  logged_in=1
fi

compose=(docker compose --env-file kim.env --profile metrics)
logged_in=${logged_in:-0}

cleanup() {
  local exit_status="$?"
  trap - EXIT
  if (( logged_in == 1 )); then
    docker logout ghcr.io >/dev/null 2>&1 || true
  fi
  exit "$exit_status"
}
trap cleanup EXIT

dump_stack() {
  echo "error: stack not healthy; diagnostics follow" >&2
  "${compose[@]}" ps -a || true
  "${compose[@]}" logs --no-color --tail=200 \
    consul consul-acl royal royal-2 chat chat-gray gateway router || true
}

# Bind-mount edits to acl-init.sh do not recreate a completed oneshot.
# Policy updates are live for existing tokens; Router needs the new write.
if ! "${compose[@]}" up --force-recreate --no-deps consul-acl; then
  dump_stack
  exit 1
fi

if ! "${compose[@]}" up -d --remove-orphans; then
  dump_stack
  exit 1
fi

healthy_services=(consul postgres redis royal royal-2 chat chat-gray gateway router)
deadline=$(( $(date +%s) + 180 ))
while true; do
  all_ok=1
  for svc in "${healthy_services[@]}"; do
    id="$("${compose[@]}" ps -q "$svc" || true)"
    if [[ -z "$id" ]]; then
      all_ok=0
      break
    fi
    running="$(docker inspect -f '{{.State.Running}}' "$id" 2>/dev/null || echo false)"
    health="$(docker inspect -f '{{if .State.Health}}{{.State.Health.Status}}{{else}}none{{end}}' "$id" 2>/dev/null || echo none)"
    if [[ "$running" != true || "$health" != healthy ]]; then
      all_ok=0
      break
    fi
  done
  if (( all_ok == 1 )); then
    break
  fi
  if (( $(date +%s) >= deadline )); then
    echo "error: timed out waiting for healthy services" >&2
    dump_stack
    exit 1
  fi
  sleep 2
done

if ! curl -fsS --max-time 5 http://127.0.0.1:8080/health >/dev/null; then
  echo "error: royal /health failed" >&2
  dump_stack
  exit 1
fi
if ! curl -fsS --max-time 5 http://127.0.0.1:8088/health >/dev/null; then
  echo "error: router /health failed" >&2
  dump_stack
  exit 1
fi

"${compose[@]}" ps
echo "deploy ok (env values not printed)"
