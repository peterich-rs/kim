import 'package:flutter_secure_storage/flutter_secure_storage.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/core/settings.dart';
import 'package:shared_preferences/shared_preferences.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  setUp(() {
    SharedPreferences.setMockInitialValues({});
  });

  test('macOS keychain is data-protection and never asks for a password', () {
    const opts = SettingsStore.macOsKeychain;
    expect(opts.usesDataProtectionKeychain, isTrue);
    expect(opts.useSecureEnclave, isFalse);
    expect(opts.synchronizable, isFalse);
    expect(opts.accessControlFlags, isEmpty);
    expect(opts.accessibility, KeychainAccessibility.first_unlock_this_device);
    expect(opts.authenticationUIBehavior, 'u_AuthUIF');
  });

  test('load uses production URL; dest and token are empty', () async {
    final store = await SettingsStore.load(useSecureStorage: false);
    expect(store.url, SettingsStore.defaultUrl);
    expect(store.dest, isEmpty);
    expect(store.token, isEmpty);
  });

  test('token stays in memory when Keychain is disabled', () async {
    final store = await SettingsStore.load(useSecureStorage: false);
    await store.saveSession(token: 'tok.jwt', account: 'peterich');
    expect(store.token, 'tok.jwt');
    expect(store.account, 'peterich');
  });

  test('dest persists via SharedPreferences; url and token do not', () async {
    final first = await SettingsStore.load(useSecureStorage: false);
    await first.saveUrl(SettingsStore.localUrl);
    await first.saveDest('carol');
    await first.saveToken('header.payload.sig');
    expect(first.token, 'header.payload.sig');
    expect(first.url, SettingsStore.localUrl);

    final second = await SettingsStore.load(useSecureStorage: false);
    expect(second.dest, 'carol');
    expect(second.url, SettingsStore.defaultUrl);
    expect(second.token, isEmpty);
  });

  test('empty url falls back to default; dest stays empty', () async {
    final store = await SettingsStore.load(useSecureStorage: false);
    await store.saveUrl('   ');
    await store.saveDest('');
    expect(store.url, SettingsStore.defaultUrl);
    expect(store.dest, isEmpty);
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

  test('avatar is stored per account', () async {
    final store = await SettingsStore.load(useSecureStorage: false);
    await store.saveSession(token: 'tok', account: 'alice');
    await store.saveAvatar('https://media.kim.ainexc.com/alice/a.jpg');
    expect(store.avatar, 'https://media.kim.ainexc.com/alice/a.jpg');
    expect(store.avatarOf('alice'), 'https://media.kim.ainexc.com/alice/a.jpg');
  });

  test('reload keeps a token without peeking JWT expiry', () async {
    final store = await SettingsStore.load(useSecureStorage: false);
    await store.saveSession(token: 'tok.jwt', account: 'alice');
    expect(store.token, isNotEmpty);
    await store.reload();
    expect(store.token, 'tok.jwt');
    expect(store.discardedExpiredToken, isFalse);
  });

  test('clearSession drops token and account', () async {
    final store = await SettingsStore.load(useSecureStorage: false);
    await store.saveSession(token: 'tok', account: 'alice');
    expect(store.account, 'alice');
    await store.clearSession();
    expect(store.token, isEmpty);
    expect(store.account, isEmpty);
  });

  test('theme pref is dart-only and defaults to system', () async {
    final first = await SettingsStore.load(useSecureStorage: false);
    expect(first.theme, 'system');
    await first.saveTheme('dark');
    final second = await SettingsStore.load(useSecureStorage: false);
    expect(second.theme, 'dark');
  });

  test('notifications-asked flag is sticky and not spammed', () async {
    final first = await SettingsStore.load(useSecureStorage: false);
    expect(first.notificationsAsked, isFalse);
    await first.markNotificationsAsked();
    final second = await SettingsStore.load(useSecureStorage: false);
    expect(second.notificationsAsked, isTrue);
  });
}
