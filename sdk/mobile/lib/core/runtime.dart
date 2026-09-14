/// One-shot bootstrap: paths, prefs, connectivity, package version.
library;

import 'package:package_info_plus/package_info_plus.dart';

import 'connectivity.dart';
import 'paths.dart';
import 'permissions.dart';
import 'settings.dart';

class KimRuntime {
  KimRuntime({
    required this.paths,
    required this.settings,
    required this.connectivity,
    required this.appName,
    required this.version,
    required this.buildNumber,
  });

  final KimPaths paths;
  final SettingsStore settings;
  final KimConnectivity connectivity;
  final String appName;
  final String version;
  final String buildNumber;

  String get versionLabel => '$version+$buildNumber';

  static Future<KimRuntime> bootstrap({
    bool requestNotifications = true,
    KimPaths? paths,
    SettingsStore? settings,
    KimConnectivity? connectivity,
    String? version,
    String? buildNumber,
    String? appName,
  }) async {
    final resolvedPaths = paths ?? await KimPaths.ensure();
    final resolvedSettings = settings ?? await SettingsStore.load();
    if (requestNotifications) {
      await KimPermissions.requestNotificationsOnce(resolvedSettings);
    }
    final resolvedConnectivity = connectivity ?? KimConnectivity();
    var name = appName ?? 'KIM';
    var ver = version ?? '1.0.0';
    var build = buildNumber ?? '1';
    if (appName == null || version == null || buildNumber == null) {
      try {
        final info = await PackageInfo.fromPlatform();
        name = appName ?? (info.appName.isEmpty ? 'KIM' : info.appName);
        ver = version ?? info.version;
        build = buildNumber ?? info.buildNumber;
      } catch (_) {
        // Widget tests / missing plugin: keep defaults.
      }
    }
    return KimRuntime(
      paths: resolvedPaths,
      settings: resolvedSettings,
      connectivity: resolvedConnectivity,
      appName: name,
      version: ver,
      buildNumber: build,
    );
  }
}
