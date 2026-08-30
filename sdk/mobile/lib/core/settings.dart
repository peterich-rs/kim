/// Last WGateway URL + dest in SharedPreferences.
/// JWT only in flutter_secure_storage (Keychain / Android Keystore).
/// Never mint a token in the app. Never commit one.
library;

import 'package:flutter_secure_storage/flutter_secure_storage.dart';
import 'package:shared_preferences/shared_preferences.dart';

class SettingsStore {
  SettingsStore({required this._prefs, this._secure});

  static const defaultUrl = 'wss://kim.ainexc.com/';
  static const localUrl = 'ws://127.0.0.1:8001/';
  static const defaultHttp = 'https://kim.ainexc.com';
  static const localHttp = 'http://127.0.0.1:8080';
  static const defaultDest = 'bob';

  static const _kUrl = 'kim.wgateway_url';
  static const _kDest = 'kim.dest_account';
  static const _kHttp = 'kim.http_origin';
  static const _kAccount = 'kim.account';
  static const _kAvatar = 'kim.avatar';
  static const _kNotifAsked = 'kim.notifications_asked';
  static const _kToken = 'kim.jwt';

  final SharedPreferences _prefs;
  final FlutterSecureStorage? _secure;
  final Map<String, String> _memorySecure = {};

  String url = defaultUrl;
  String dest = defaultDest;
  String httpOrigin = defaultHttp;
  String account = '';
  String token = '';
  String avatar = '';
  bool notificationsAsked = false;

  static FlutterSecureStorage productionSecureStorage() {
    // iOS Keychain. Android: RSA-OAEP + AES-GCM (EncryptedSharedPreferences
    // was removed in flutter_secure_storage 11; this is the replacement).
    return const FlutterSecureStorage(
      aOptions: AndroidOptions(),
      iOptions: IOSOptions(
        accessibility: KeychainAccessibility.unlocked_this_device,
      ),
    );
  }

  static Future<SettingsStore> load({
    SharedPreferences? prefs,
    FlutterSecureStorage? secure,
    bool useSecureStorage = true,
  }) async {
    final store = SettingsStore(
      prefs: prefs ?? await SharedPreferences.getInstance(),
      secure: useSecureStorage ? (secure ?? productionSecureStorage()) : null,
    );
    await store.reload();
    return store;
  }

  Future<void> reload() async {
    url = _prefs.getString(_kUrl)?.trim().isNotEmpty == true
        ? _prefs.getString(_kUrl)!.trim()
        : defaultUrl;
    dest = _prefs.getString(_kDest)?.trim().isNotEmpty == true
        ? _prefs.getString(_kDest)!.trim()
        : defaultDest;
    httpOrigin = _prefs.getString(_kHttp)?.trim().isNotEmpty == true
        ? _prefs.getString(_kHttp)!.trim()
        : defaultHttp;
    account = _prefs.getString(_kAccount)?.trim() ?? '';
    avatar = avatarOf(account);
    notificationsAsked = _prefs.getBool(_kNotifAsked) ?? false;
    token = await _readToken();
  }

  String avatarOf(String account) {
    if (account.isEmpty) {
      return '';
    }
    return _prefs.getString('$_kAvatar.$account')?.trim() ?? '';
  }

  Future<void> saveUrl(String value) async {
    url = value.trim().isEmpty ? defaultUrl : value.trim();
    await _prefs.setString(_kUrl, url);
  }

  Future<void> saveDest(String value) async {
    dest = value.trim().isEmpty ? defaultDest : value.trim();
    await _prefs.setString(_kDest, dest);
  }

  Future<void> saveHttpOrigin(String value) async {
    httpOrigin = value.trim().isEmpty
        ? defaultHttp
        : value.trim().replaceAll(RegExp(r'/$'), '');
    await _prefs.setString(_kHttp, httpOrigin);
  }

  Future<void> saveAccount(String value) async {
    account = value.trim();
    if (account.isEmpty) {
      await _prefs.remove(_kAccount);
    } else {
      await _prefs.setString(_kAccount, account);
    }
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

  Future<void> saveSession({
    required String token,
    required String account,
  }) async {
    await saveToken(token);
    await saveAccount(account);
    avatar = avatarOf(account);
  }

  Future<void> clearSession() async {
    await saveToken('');
    await saveAccount('');
    avatar = '';
  }

  Future<void> useLocal() async {
    await saveUrl(localUrl);
    await saveHttpOrigin(localHttp);
  }

  Future<void> useProd() async {
    await saveUrl(defaultUrl);
    await saveHttpOrigin(defaultHttp);
  }

  Future<void> saveToken(String value) async {
    token = value.trim();
    if (_secure != null) {
      if (token.isEmpty) {
        await _secure.delete(key: _kToken);
      } else {
        await _secure.write(key: _kToken, value: token);
      }
    } else {
      if (token.isEmpty) {
        _memorySecure.remove(_kToken);
      } else {
        _memorySecure[_kToken] = token;
      }
    }
  }

  Future<void> markNotificationsAsked() async {
    notificationsAsked = true;
    await _prefs.setBool(_kNotifAsked, true);
  }

  Future<String> _readToken() async {
    try {
      if (_secure != null) {
        return (await _secure.read(key: _kToken))?.trim() ?? '';
      }
      return _memorySecure[_kToken] ?? '';
    } catch (_) {
      // Missing plugin / Keystore errors: treat as empty, never mint.
      return '';
    }
  }
}
