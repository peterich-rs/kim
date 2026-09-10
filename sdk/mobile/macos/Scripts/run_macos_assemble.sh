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

# Project FVM pin first, then cargo / fvm default / Homebrew from .xcode.env.
fvm_bin="$(cd "${SRCROOT}/.." && pwd)/.fvm/flutter_sdk/bin"
case ":${PATH}:" in
  *:"$fvm_bin":*) ;;
  *) [ -d "$fvm_bin" ] && PATH="$fvm_bin:$PATH" ;;
esac
case ":${PATH}:" in
  *:"${HOME}/.cargo/bin":*) ;;
  *) PATH="${HOME}/.cargo/bin:$PATH" ;;
esac
export PATH

# Scheme pre-actions often omit xcconfig, so FLUTTER_ROOT is empty.
# Same idea as `which flutter`: bin/flutter → SDK root.
if [ -z "${FLUTTER_ROOT:-}" ] || [ ! -f "${FLUTTER_ROOT}/packages/flutter_tools/bin/macos_assemble.sh" ]; then
  flutter_bin=$(command -v flutter || true)
  if [ -z "$flutter_bin" ]; then
    echo "error: flutter not on PATH. Install FVM/Flutter or set FLUTTER_ROOT in macos/.xcode.env.local." >&2
    exit 1
  fi
  FLUTTER_ROOT="$(cd "$(dirname "$flutter_bin")/.." && pwd -P)"
  export FLUTTER_ROOT
fi
assemble="${FLUTTER_ROOT}/packages/flutter_tools/bin/macos_assemble.sh"
if [ ! -f "$assemble" ]; then
  echo "error: macos_assemble.sh missing under FLUTTER_ROOT=$FLUTTER_ROOT" >&2
  exit 1
fi

# Xcode 15+ fails CreateBuildDescription if script-phase xcfilelists are
# missing. `flutter build macos` creates empty placeholders first; a raw
# Xcode GUI / xcodebuild does not. Touch them during the scheme pre-action
# (`prepare`) so the first Xcode build can proceed.
EPHEMERAL="${SRCROOT}/Flutter/ephemeral"
mkdir -p "$EPHEMERAL"
[ -f "$EPHEMERAL/FlutterInputs.xcfilelist" ] || : > "$EPHEMERAL/FlutterInputs.xcfilelist"
[ -f "$EPHEMERAL/FlutterOutputs.xcfilelist" ] || : > "$EPHEMERAL/FlutterOutputs.xcfilelist"
[ -f "$EPHEMERAL/tripwire" ] || : > "$EPHEMERAL/tripwire"

# Pre-actions often omit Xcode's CONFIGURATION / SOURCE_ROOT.
export SOURCE_ROOT="${SOURCE_ROOT:-$SRCROOT}"
export CONFIGURATION="${CONFIGURATION:-Debug}"

MODE="${1:-build}"
if [ "$MODE" = "embed" ]; then
  echo "$PRODUCT_NAME.app" > "${PROJECT_DIR:-$SRCROOT}"/Flutter/ephemeral/.app_filename
  exec "$assemble" embed
fi
if [ "$MODE" = "prepare" ]; then
  # Unpack copies the Flutter.framework into BUILT_PRODUCTS_DIR. Without it,
  # Flutter's prepare would pass --output=/ — skip and keep placeholders so
  # CreateBuildDescription can proceed; the build phase has full env.
  if [ -z "${BUILT_PRODUCTS_DIR:-}" ]; then
    echo "note: BUILT_PRODUCTS_DIR unset; placeholders ready for Xcode."
    exit 0
  fi
  exec "$assemble" prepare
fi
if ! command -v cargo >/dev/null 2>&1; then
  echo "error: cargo not on PATH (Xcode does not load ~/.zshrc). Install rustup or add ~/.cargo/bin." >&2
  exit 1
fi
"$assemble"
touch "${EPHEMERAL}/tripwire"
