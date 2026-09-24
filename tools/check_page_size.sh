#!/bin/sh
# Fail when a feature page exceeds the soft cap from docs/impl/flutter-layering.md.
set -eu
root="$(cd "$(dirname "$0")/.." && pwd)"
cap=400
fail=0
for f in "$root"/sdk/mobile/lib/features/**/*_page.dart; do
  [ -f "$f" ] || continue
  n=$(wc -l < "$f" | tr -d ' ')
  if [ "$n" -gt "$cap" ]; then
    echo "page over ${cap}: ${n} ${f#"$root"/}"
    fail=1
  fi
done
exit "$fail"
