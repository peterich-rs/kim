/// Boot calls that must not leak the generated FFI into `main`.
library;

import 'package:kim_mobile/src/rust/api/bootstrap.dart' as rust;
import 'package:kim_mobile/src/rust/api/catalog.dart' as catalog;

Future<void> platformBootstrap({
  required String documents,
  required String support,
  required String cache,
  required String temp,
  required String appVersion,
  required String buildNumber,
  required bool simulator,
}) {
  return rust.platformBootstrap(
    documents: documents,
    support: support,
    cache: cache,
    temp: temp,
    appVersion: appVersion,
    buildNumber: buildNumber,
    simulator: simulator,
  );
}

Future<void> setPlatformSupportRoot(String support) {
  return catalog.setPlatformSupportRoot(support: support);
}
