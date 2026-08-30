library;

import 'package:flutter_riverpod/flutter_riverpod.dart';

/// Current GoRouter path. Updated from [redirect], not from widget dispose.
class LocationNotifier extends Notifier<String> {
  @override
  String build() => '/';

  void setPath(String path) {
    if (state == path) {
      return;
    }
    state = path;
  }
}

final locationProvider = NotifierProvider<LocationNotifier, String>(
  LocationNotifier.new,
);

String? chatIdFromPath(String path) {
  const prefix = '/chat/';
  if (!path.startsWith(prefix)) {
    return null;
  }
  final rest = path.substring(prefix.length);
  final slash = rest.indexOf('/');
  final raw = slash == -1 ? rest : rest.substring(0, slash);
  if (raw.isEmpty) {
    return null;
  }
  return Uri.decodeComponent(raw);
}
