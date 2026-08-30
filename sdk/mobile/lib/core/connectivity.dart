/// Network presence for an offline banner. Not a Dart socket.
library;

import 'dart:async';

import 'package:connectivity_plus/connectivity_plus.dart';
import 'package:flutter/widgets.dart';

bool networkLinkUp(List<ConnectivityResult> results) {
  if (results.isEmpty) {
    return false;
  }
  if (results.length == 1 && results.first == ConnectivityResult.none) {
    return false;
  }
  return results.any(
    (r) =>
        r == ConnectivityResult.wifi ||
        r == ConnectivityResult.mobile ||
        r == ConnectivityResult.ethernet ||
        r == ConnectivityResult.vpn,
  );
}

class KimConnectivity with WidgetsBindingObserver {
  KimConnectivity._({
    required this.online,
    this._plugin,
    this._sub,
    this._observeLifecycle = false,
  }) {
    if (_observeLifecycle) {
      WidgetsBinding.instance.addObserver(this);
    }
  }

  final ValueNotifier<bool> online;
  final Connectivity? _plugin;
  final StreamSubscription<List<ConnectivityResult>>? _sub;
  final bool _observeLifecycle;

  factory KimConnectivity() {
    final plugin = Connectivity();
    final online = ValueNotifier<bool>(true);
    void apply(List<ConnectivityResult> results) {
      online.value = networkLinkUp(results);
    }

    final sub = plugin.onConnectivityChanged.listen(
      apply,
      onError: (_) {
        online.value = false;
      },
    );
    plugin.checkConnectivity().then(apply).catchError((_) {
      online.value = false;
    });
    return KimConnectivity._(
      online: online,
      plugin: plugin,
      sub: sub,
      observeLifecycle: true,
    );
  }

  /// Tests / hosts without the plugin.
  factory KimConnectivity.fake({bool isOnline = true}) {
    return KimConnectivity._(online: ValueNotifier<bool>(isOnline));
  }

  bool get isOnline => online.value;

  Future<void> recheck() async {
    final plugin = _plugin;
    if (plugin == null) {
      return;
    }
    try {
      online.value = networkLinkUp(await plugin.checkConnectivity());
    } catch (_) {
      online.value = false;
    }
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    if (state == AppLifecycleState.resumed) {
      unawaited(recheck());
    }
  }

  void dispose() {
    if (_observeLifecycle) {
      WidgetsBinding.instance.removeObserver(this);
    }
    _sub?.cancel();
    online.dispose();
  }
}
