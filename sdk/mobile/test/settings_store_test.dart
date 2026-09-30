import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/core/secret_store_executor.dart';
import 'package:kim_mobile/core/settings.dart';
import 'package:shared_preferences/shared_preferences.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  setUp(() {
    SharedPreferences.setMockInitialValues({});
  });

  test('keystore channel name is stable', () {
    // The platform side (AppDelegate.swift / KimKeystore.kt) registers this
    // exact channel name; Rust key names ride as arguments.
    expect(kimKeystoreChannel, 'kim.keystore');
  });

  test('load keeps UI prefs only', () async {
    final store = await SettingsStore.load();
    expect(store.dest, isEmpty);
    expect(store.theme, 'system');
    expect(store.notificationsAsked, isFalse);
    expect(store.account, isEmpty);
  });

  test('dest persists; legacy url and token keys are not rehydrated', () async {
    SharedPreferences.setMockInitialValues({
      'kim.wgateway_url': 'ws://127.0.0.1:8001/ws',
      'kim.jwt.fallback': 'header.payload.sig',
    });
    final first = await SettingsStore.load();
    await first.saveDest('carol');
    expect(first.dest, 'carol');

    final second = await SettingsStore.load();
    expect(second.dest, 'carol');
    expect(
      second.prefs.getString('kim.wgateway_url'),
      'ws://127.0.0.1:8001/ws',
    );

    await second.dropImportedPrefs();
    expect(second.prefs.getString('kim.wgateway_url'), isNull);
    expect(second.prefs.getString('kim.jwt.fallback'), isNull);
    expect(second.dest, 'carol');
  });

  test('avatar is stored per account', () async {
    final store = await SettingsStore.load();
    store.account = 'alice';
    await store.saveAvatar('https://media.kim.ainexc.com/alice/a.jpg');
    expect(store.avatar, 'https://media.kim.ainexc.com/alice/a.jpg');
    expect(store.avatarOf('alice'), 'https://media.kim.ainexc.com/alice/a.jpg');
  });

  test('theme pref is dart-only and defaults to system', () async {
    final first = await SettingsStore.load();
    expect(first.theme, 'system');
    await first.saveTheme('dark');
    final second = await SettingsStore.load();
    expect(second.theme, 'dark');
  });

  test('notifications-asked flag is sticky and not spammed', () async {
    final first = await SettingsStore.load();
    expect(first.notificationsAsked, isFalse);
    await first.markNotificationsAsked();
    final second = await SettingsStore.load();
    expect(second.notificationsAsked, isTrue);
  });
}
