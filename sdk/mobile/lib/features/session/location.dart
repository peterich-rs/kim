library;

import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:kim_mobile/router/app_routes.dart';

/// Current GoRouter path. Updated from [redirect], not from widget dispose.
class LocationNotifier extends Notifier<String> {
  @override
  String build() => AppRoutes.home;

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

String? chatIdFromPath(String path) => AppRoutes.chatIdFromPath(path);
