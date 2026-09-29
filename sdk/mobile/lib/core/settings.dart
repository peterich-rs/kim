/// UI-only preferences (theme / dest / avatar / notificationsAsked) live in
/// SharedPreferences. Token + account are Rust-owned (Keychain via the
/// executor channel); URL/origin/env are Rust settings-table rows. Dart
/// keeps no mirrors of either.
library;

import 'package:shared_preferences/shared_preferences.dart';

class SettingsStore {
  SettingsStore({required this._prefs});

  static const _kDest = 'kim.dest_account';
  static const _kAvatar = 'kim.avatar';
  static const _kNotifAsked = 'kim.notifications_asked';
  static const _kTheme = 'kim.theme';

  final SharedPreferences _prefs;

  SharedPreferences get prefs => _prefs;

  String dest = '';
  String avatar = '';
  bool notificationsAsked = false;
  String theme = 'system';

  /// Rust-owned projection cache: signed-in account for avatar keying.
  String account = '';

  static Future<SettingsStore> load({SharedPreferences? prefs}) async {
    final store = SettingsStore(
      prefs: prefs ?? await SharedPreferences.getInstance(),
    );
    await store.reload();
    return store;
  }

  Future<void> reload() async {
    dest = _prefs.getString(_kDest)?.trim() ?? '';
    notificationsAsked = _prefs.getBool(_kNotifAsked) ?? false;
    theme = _prefs.getString(_kTheme)?.trim() ?? 'system';
    avatar = avatarOf(account);
  }

  /// One-shot legacy handoff helper: after Rust confirms the import the
  /// device settings keys are dead and get dropped.
  Future<void> dropImportedPrefs() async {
    await _prefs.remove('kim.wgateway_url');
    await _prefs.remove('kim.http_origin');
    await _prefs.remove('kim.account');
    await _prefs.remove('kim.jwt.fallback');
    await _prefs.remove('kim.account.fallback');
  }

  String avatarOf(String account) {
    if (account.isEmpty) {
      return '';
    }
    return _prefs.getString('$_kAvatar.$account')?.trim() ?? '';
  }

  Future<void> saveDest(String value) async {
    dest = value.trim();
    await _prefs.setString(_kDest, dest);
  }

  Future<void> saveAvatar(String value) async {
    avatar = value.trim();
    if (account.isEmpty) {
      return;
    }
    final key = '$_kAvatar.$account';
    if (avatar.isEmpty) {
      await _prefs.remove(key);
    } else {
      await _prefs.setString(key, avatar);
    }
  }

  Future<void> saveTheme(String value) async {
    theme = switch (value.trim()) {
      'light' || 'dark' => value.trim(),
      _ => 'system',
    };
    await _prefs.setString(_kTheme, theme);
  }

  Future<void> markNotificationsAsked() async {
    notificationsAsked = true;
    await _prefs.setBool(_kNotifAsked, true);
  }
}
