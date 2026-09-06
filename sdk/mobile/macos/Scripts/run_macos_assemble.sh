#!/bin/sh
# Native Assets shell out to rustup/cargo; Xcode GUI PATH is minimal.
set -e
SRCROOT="${SRCROOT:-$(cd "$(dirname "$0")/.." && pwd)}"
if [ -f "${SRCROOT}/.xcode.env" ]; then
  # shellcheck disable=SC1091
  . "${SRCROOT}/.xcode.env"
fi
if [ -f "${SRCROOT}/.xcode.env.local" ]; then
  # shellcheck disable=SC1091
  . "${SRCROOT}/.xcode.env.local"
fi
case ":${PATH}:" in
  *:"${HOME}/.cargo/bin":*) ;;
  *) export PATH="${HOME}/.cargo/bin:${PATH}" ;;
esac
MODE="${1:-build}"
if [ "$MODE" = "embed" ]; then
  echo "$PRODUCT_NAME.app" > "$PROJECT_DIR"/Flutter/ephemeral/.app_filename
  exec "$FLUTTER_ROOT"/packages/flutter_tools/bin/macos_assemble.sh embed
fi
"$FLUTTER_ROOT"/packages/flutter_tools/bin/macos_assemble.sh
touch Flutter/ephemeral/tripwire
