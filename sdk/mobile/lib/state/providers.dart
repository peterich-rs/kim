library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_riverpod/misc.dart';

import '../core/media.dart';
import '../core/runtime.dart';
import '../kim_bridge.dart';

final runtimeProvider = Provider<KimRuntime>((ref) {
  throw StateError('runtimeProvider must be overridden in main / tests');
});

final authPortProvider = Provider<KimAuthPort>((ref) {
  throw StateError('authPortProvider must be overridden in main / tests');
});

final clientPortProvider = Provider<KimClientPort>((ref) {
  throw StateError('clientPortProvider must be overridden in main / tests');
});

final mediaPortProvider = Provider<KimMediaPort>((ref) {
  throw StateError('mediaPortProvider must be overridden in main / tests');
});

ThemeMode kimThemeModeFromPrefs(String raw) {
  return switch (raw) {
    'light' => ThemeMode.light,
    'dark' => ThemeMode.dark,
    _ => ThemeMode.system,
  };
}

final themeModeProvider = NotifierProvider<ThemeModeNotifier, ThemeMode>(
  ThemeModeNotifier.new,
);

class ThemeModeNotifier extends Notifier<ThemeMode> {
  @override
  ThemeMode build() {
    return kimThemeModeFromPrefs(ref.watch(runtimeProvider).settings.theme);
  }

  Future<void> setMode(ThemeMode mode) async {
    final value = switch (mode) {
      ThemeMode.light => 'light',
      ThemeMode.dark => 'dark',
      ThemeMode.system => 'system',
    };
    await ref.read(runtimeProvider).settings.saveTheme(value);
    state = mode;
  }
}

List<Override> kimProviderOverrides({
  required KimRuntime runtime,
  required KimAuthPort auth,
  required KimClientPort client,
  KimMediaPort? media,
}) {
  return [
    runtimeProvider.overrideWithValue(runtime),
    authPortProvider.overrideWithValue(auth),
    clientPortProvider.overrideWithValue(client),
    mediaPortProvider.overrideWithValue(
      media ??
          (client is KimMediaPort
              ? client as KimMediaPort
              : (throw StateError('mediaPort required'))),
    ),
  ];
}

/// Radio from [KimConnectivity]. Independent of the WGateway socket.
final radioOnlineProvider = NotifierProvider<RadioOnlineNotifier, bool>(
  RadioOnlineNotifier.new,
);

class RadioOnlineNotifier extends Notifier<bool> {
  @override
  bool build() {
    final listenable = ref.watch(runtimeProvider).connectivity.online;
    void tick() {
      state = listenable.value;
    }

    listenable.addListener(tick);
    ref.onDispose(() => listenable.removeListener(tick));
    return listenable.value;
  }
}
