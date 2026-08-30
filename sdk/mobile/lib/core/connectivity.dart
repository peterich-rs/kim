/// Network presence for an offline banner. Not a Dart socket.
library;

import 'dart:async';

import 'package:connectivity_plus/connectivity_plus.dart';
import 'package:flutter/foundation.dart';

class KimConnectivity {
  KimConnectivity._({required this.online, this._sub});

  final ValueNotifier<bool> online;
  final StreamSubscription<List<ConnectivityResult>>? _sub;

  factory KimConnectivity() {
    final plugin = Connectivity();
    final online = ValueNotifier<bool>(true);
    void apply(List<ConnectivityResult> results) {
      online.value = results.hasConnectivity;
    }

    final sub = plugin.onConnectivityChanged.listen(apply);
    plugin.checkConnectivity().then(apply);
    return KimConnectivity._(online: online, sub: sub);
  }

  /// Tests / hosts without the plugin.
  factory KimConnectivity.fake({bool isOnline = true}) {
    return KimConnectivity._(online: ValueNotifier<bool>(isOnline));
  }

  bool get isOnline => online.value;

  void dispose() {
    _sub?.cancel();
    online.dispose();
  }
}
