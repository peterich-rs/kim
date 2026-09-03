#!/usr/bin/env bash
# Re-runnable inbox materialization backfill via the Compose postgres container.
set -euo pipefail
dir=$(cd "$(dirname "$0")" && pwd)
exec "$dir/psql-compose.sh" "$dir/backfill-inbox.sql"
