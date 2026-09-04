import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/core/settings.dart';
import 'package:shared_preferences/shared_preferences.dart';

import 'support/jwt.dart';

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

  test('local/prod presets keep http origin next to wgateway', () async {
    final store = await SettingsStore.load(useSecureStorage: false);
    expect(store.httpOrigin, SettingsStore.defaultHttp);
    await store.useLocal();
    expect(store.url, SettingsStore.localUrl);
    expect(store.httpOrigin, SettingsStore.localHttp);
    await store.useProd();
    expect(store.url, SettingsStore.defaultUrl);
    expect(store.httpOrigin, SettingsStore.defaultHttp);
  });

  test('avatar is stored per account and reloaded', () async {
    final first = await SettingsStore.load(useSecureStorage: false);
    await first.saveSession(token: 'tok', account: 'alice');
    await first.saveAvatar('https://media.kim.ainexc.com/alice/a.jpg');
    expect(first.avatar, 'https://media.kim.ainexc.com/alice/a.jpg');

    final second = await SettingsStore.load(useSecureStorage: false);
    expect(second.account, 'alice');
    expect(second.avatar, 'https://media.kim.ainexc.com/alice/a.jpg');
  });

  test('reload discards an expired JWT', () async {
    final store = await SettingsStore.load(useSecureStorage: false);
    await store.saveSession(
      token: testJwt(acc: 'alice', exp: 1),
      account: 'alice',
    );
    expect(store.token, isNotEmpty);
    await store.reload();
    expect(store.token, isEmpty);
    expect(store.discardedExpiredToken, isTrue);
  });

  test('reload recovers account from JWT when prefs are empty', () async {
    final store = await SettingsStore.load(useSecureStorage: false);
    await store.saveToken(testJwt(acc: 'alice', exp: 4_000_000_000));
    await store.saveAccount('');
    await store.reload();
    expect(store.token, isNotEmpty);
    expect(store.account, 'alice');
  });

  test('clearSession drops token and account', () async {
    final store = await SettingsStore.load(useSecureStorage: false);
    await store.saveSession(token: 'tok', account: 'alice');
    expect(store.account, 'alice');
    await store.clearSession();
    expect(store.token, isEmpty);
    expect(store.account, isEmpty);
  });

  test('notifications-asked flag is sticky and not spammed', () async {
    final first = await SettingsStore.load(useSecureStorage: false);
    expect(first.notificationsAsked, isFalse);
    await first.markNotificationsAsked();
    final second = await SettingsStore.load(useSecureStorage: false);
    expect(second.notificationsAsked, isTrue);
  });
}
