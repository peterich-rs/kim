import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/core/settings.dart';
import 'package:shared_preferences/shared_preferences.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  setUp(() {
    SharedPreferences.setMockInitialValues({});
  });

  test('load uses production URL and dest defaults; token is empty', () async {
    final store = await SettingsStore.load(useSecureStorage: false);
    expect(store.url, SettingsStore.defaultUrl);
    expect(store.dest, SettingsStore.defaultDest);
    expect(store.token, isEmpty);
  });

  test('url and dest persist via SharedPreferences, not the token', () async {
    final first = await SettingsStore.load(useSecureStorage: false);
    await first.saveUrl(SettingsStore.localUrl);
    await first.saveDest('carol');
    await first.saveToken('header.payload.sig');
    expect(first.token, 'header.payload.sig');

    final second = await SettingsStore.load(useSecureStorage: false);
    expect(second.url, SettingsStore.localUrl);
    expect(second.dest, 'carol');
    // New store: in-memory vault is empty. JWT is not in SharedPreferences.
    expect(second.token, isEmpty);
  });

  test('empty url/dest fall back to defaults', () async {
    final store = await SettingsStore.load(useSecureStorage: false);
    await store.saveUrl('   ');
    await store.saveDest('');
    expect(store.url, SettingsStore.defaultUrl);
    expect(store.dest, SettingsStore.defaultDest);
  });

  test('notifications-asked flag is sticky and not spammed', () async {
    final first = await SettingsStore.load(useSecureStorage: false);
    expect(first.notificationsAsked, isFalse);
    await first.markNotificationsAsked();
    final second = await SettingsStore.load(useSecureStorage: false);
    expect(second.notificationsAsked, isTrue);
  });
}
