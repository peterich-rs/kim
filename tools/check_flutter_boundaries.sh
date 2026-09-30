#!/bin/sh
# Architecture boundary gate for the Flutter shell. Mirrors
# docs/flutter-layering.md: FFI capability surfaces (client / auth / handles
# / bootstrap / generated loaders) are only reachable from lib/bridge/** and
# the approved core boot files. Projection types (src/rust/api/types.dart,
# src/rust_agent/api/catalog.dart) are pure data and may be imported anywhere,
# like protobuf generated code.
set -eu
root="$(cd "$(dirname "$0")/.." && pwd)"
mobile="$root/sdk/mobile"
cd "$mobile"

fail=0

# 1. Client FFI capability surface: bridge layer + boot files only.
violators="$(rg -l "package:kim_mobile/src/rust/(api/(client|auth|handles|bootstrap|simple)\.dart|frb_generated\.dart)" lib --glob '!lib/src/**' 2>/dev/null \
  | grep -v '^lib/bridge/' \
  | grep -v '^lib/core/secret_store_executor.dart$' \
  | grep -v '^lib/main.dart$' || true)"
if [ -n "$violators" ]; then
  echo "client FFI capability surface imported outside the bridge layer:"
  echo "$violators"
  fail=1
fi

# 2. Agent catalog loader: goose bridge + main only (types.dart of rust_agent
#    is projection data and stays allowed).
violators="$(rg -l "package:kim_mobile/src/rust_agent/frb_generated\.dart" lib --glob '!lib/src/**' 2>/dev/null \
  | grep -v '^lib/bridge/goose_bridge.dart$' \
  | grep -v '^lib/main.dart$' || true)"
if [ -n "$violators" ]; then
  echo "agent FFI loader imported outside the approved files:"
  echo "$violators"
  fail=1
fi

# 3. SharedPreferences: pure UI state only (theme/dest/avatar flags, agent
#    form prefs, active-profile pointer).
violators="$(rg -l "package:shared_preferences/shared_preferences.dart" lib 2>/dev/null \
  | grep -v '^lib/core/settings.dart$' \
  | grep -v '^lib/features/agent/providers/agent_settings.dart$' \
  | grep -v '^lib/features/agent/providers/agent_profiles.dart$' \
  | grep -v '^lib/features/agent/providers/provider_accounts.dart$' || true)"
if [ -n "$violators" ]; then
  echo "SharedPreferences used outside approved UI-state files:"
  echo "$violators"
  fail=1
fi

if [ "$fail" -ne 0 ]; then
  echo "flutter boundary check FAILED"
  exit 1
fi
echo "flutter boundary check OK"
