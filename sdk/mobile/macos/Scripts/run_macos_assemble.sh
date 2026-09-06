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

# Xcode 15+ fails CreateBuildDescription if script-phase xcfilelists are
# missing. `flutter build macos` creates empty placeholders first; a raw
# Xcode GUI / xcodebuild does not. Touch them during the scheme pre-action
# (`prepare`) so the first Xcode build can proceed.
EPHEMERAL="${SRCROOT}/Flutter/ephemeral"
mkdir -p "$EPHEMERAL"
[ -f "$EPHEMERAL/FlutterInputs.xcfilelist" ] || : > "$EPHEMERAL/FlutterInputs.xcfilelist"
[ -f "$EPHEMERAL/FlutterOutputs.xcfilelist" ] || : > "$EPHEMERAL/FlutterOutputs.xcfilelist"
[ -f "$EPHEMERAL/tripwire" ] || : > "$EPHEMERAL/tripwire"

MODE="${1:-build}"
if [ "$MODE" = "embed" ]; then
  echo "$PRODUCT_NAME.app" > "$PROJECT_DIR"/Flutter/ephemeral/.app_filename
  exec "$FLUTTER_ROOT"/packages/flutter_tools/bin/macos_assemble.sh embed
fi
if [ "$MODE" = "prepare" ]; then
  exec "$FLUTTER_ROOT"/packages/flutter_tools/bin/macos_assemble.sh prepare
fi
"$FLUTTER_ROOT"/packages/flutter_tools/bin/macos_assemble.sh
touch Flutter/ephemeral/tripwire
