#!/usr/bin/env bash
# Run SQL against the Compose postgres service. Does not put DATABASE_URL
# (or its password) on the process command line.
set -euo pipefail

deploy_dir=$(cd "$(dirname "$0")" && pwd)
if [[ $# -lt 1 ]]; then
  echo "usage: $0 file.sql" >&2
  exit 2
fi
sql=$1
if [[ "$sql" != /* ]]; then
  sql="$deploy_dir/$sql"
fi
if [[ ! -f "$sql" ]]; then
  echo "error: missing $sql" >&2
  exit 1
fi

compose=(docker compose -f "$deploy_dir/compose.yml")
if [[ -f "$deploy_dir/kim.env" ]]; then
  compose+=(--env-file "$deploy_dir/kim.env")
fi

exec "${compose[@]}" exec -T postgres sh -c \
  'exec psql -U "$POSTGRES_USER" -d "$POSTGRES_DB" -v ON_ERROR_STOP=1 -X' \
  <"$sql"
