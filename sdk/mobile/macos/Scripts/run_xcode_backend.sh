#!/bin/sh
# Flutter native-assets (hook/build.dart) shells out to rustup. Xcode's PATH
# is /usr/bin:/bin:/usr/sbin:/sbin, so rustup is missing unless we add it.
set -e
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
exec /bin/sh "$FLUTTER_ROOT/packages/flutter_tools/bin/xcode_backend.sh" "$@"
