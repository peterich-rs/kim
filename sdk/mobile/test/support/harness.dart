import 'dart:io';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/core/connectivity.dart';
import 'package:kim_mobile/core/paths.dart';
import 'package:kim_mobile/core/runtime.dart';
import 'package:kim_mobile/core/settings.dart';
import 'package:kim_mobile/data/conversation_store.dart';
import 'package:kim_mobile/state/providers.dart';
import 'package:kim_mobile/state/retry.dart';
import 'package:shared_preferences/shared_preferences.dart';

import 'fake_kim.dart';

class KimHarness {
  KimHarness({
    required this.container,
    required this.fake,
    required this.runtime,
    required this.store,
  });

  final ProviderContainer container;
  final FakeKim fake;
  final KimRuntime runtime;
  final ConversationStore store;
}

Future<KimHarness> kimHarness({
  String token = '',
  String account = '',
  bool online = true,
}) async {
  SharedPreferences.setMockInitialValues({});
  final tmp = Directory.systemTemp.createTempSync('kim-shell-');
  addTearDown(() {
    if (tmp.existsSync()) {
      tmp.deleteSync(recursive: true);
    }
  });
  final settings = await SettingsStore.load(useSecureStorage: false);
  if (token.isNotEmpty) {
    await settings.saveSession(token: token, account: account);
  }
  final runtime = await KimRuntime.bootstrap(
    requestNotifications: false,
    paths: KimPaths.forTest(tmp),
    settings: settings,
    connectivity: KimConnectivity.fake(isOnline: online),
    appName: 'KIM',
    version: '1.0.0',
    buildNumber: '1',
  );
  final fake = FakeKim();
  final store = ConversationStore.memory();
  addTearDown(store.close);
  final container = ProviderContainer.test(
    retry: kimRetry,
    overrides: kimProviderOverrides(
      runtime: runtime,
      auth: fake,
      client: fake,
      store: store,
      media: FakeKimMedia(),
    ),
  );
  return KimHarness(
    container: container,
    fake: fake,
    runtime: runtime,
    store: store,
  );
}
