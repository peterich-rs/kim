/// Token + account live in Keychain. Device URL/origin/env live in Rust
/// settings (account=''). Prefs keep dest, avatar, notification flag only.
/// Never mint a token in the app. Never commit one.
library;

import 'package:flutter_secure_storage/flutter_secure_storage.dart';
import 'package:shared_preferences/shared_preferences.dart';

import 'package:kim_mobile/core/logger.dart';

class SettingsStore {
  SettingsStore({required this._prefs, this._secure});

  static const defaultUrl = 'wss://kim.ainexc.com/';
  static const localUrl = 'ws://127.0.0.1:8001/';
  static const defaultHttp = 'https://kim.ainexc.com';
  static const localHttp = 'http://127.0.0.1:8080';

  static const _kUrl = 'kim.wgateway_url';
  static const _kDest = 'kim.dest_account';
  static const _kHttp = 'kim.http_origin';
  static const _kAccount = 'kim.account';
  static const _kAvatar = 'kim.avatar';
  static const _kNotifAsked = 'kim.notifications_asked';
  static const _kTheme = 'kim.theme';
  static const _kToken = 'kim.jwt';
  static const _kWrap = 'kim.agent_wrap';

  /// Used only when Keychain throws (macOS ad-hoc -34018). Not the happy path.
  static const _kTokenFallback = 'kim.jwt.fallback';
  static const _kAccountFallback = 'kim.account.fallback';

  final SharedPreferences _prefs;
  final FlutterSecureStorage? _secure;

  SharedPreferences get prefs => _prefs;
  final Map<String, String> _memorySecure = {};

  String url = defaultUrl;
  String dest = '';
  String httpOrigin = defaultHttp;
  String env = 'prod';
  String account = '';
  String token = '';
  String avatar = '';
  bool notificationsAsked = false;
  bool discardedExpiredToken = false;
  String theme = 'system';

  /// macOS Data Protection keychain (iOS-style). No login-keychain password
  /// dialog, no biometry, no passcode. Available after first unlock.
  ///
  /// Do not set [MacOsOptions.usesDataProtectionKeychain] to false: that
  /// stores into the file-based login keychain, which can prompt for the
  /// user's login password. Ad-hoc "Sign to Run Locally" still lacks
  /// `application-identifier` and may throw -34018; [saveToken] then falls
  /// back without prompting.
  static const macOsKeychain = MacOsOptions(
    usesDataProtectionKeychain: true,
    accessibility: KeychainAccessibility.first_unlock_this_device,
    synchronizable: false,
    useSecureEnclave: false,
    accessControlFlags: [],
    // kSecUseAuthenticationUIFail — never present an auth UI.
    authenticationUIBehavior: 'u_AuthUIF',
  );

  static FlutterSecureStorage productionSecureStorage() {
    // iOS Keychain. Android: RSA-OAEP + AES-GCM (EncryptedSharedPreferences
    // was removed in flutter_secure_storage 11; this is the replacement).
    return const FlutterSecureStorage(
      aOptions: AndroidOptions(),
      iOptions: IOSOptions(
        accessibility: KeychainAccessibility.first_unlock_this_device,
      ),
      mOptions: macOsKeychain,
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
    dest = _prefs.getString(_kDest)?.trim() ?? '';
    httpOrigin = _prefs.getString(_kHttp)?.trim().isNotEmpty == true
        ? _prefs.getString(_kHttp)!.trim()
        : defaultHttp;
    notificationsAsked = _prefs.getBool(_kNotifAsked) ?? false;
    theme = _prefs.getString(_kTheme)?.trim() ?? 'system';
    token = await _readToken();
    account = await _readAccount();
    avatar = avatarOf(account);
  }

  void applyRemote({
    required String wsUrl,
    required String httpOrigin,
    required String account,
    String env = '',
  }) {
    if (wsUrl.isNotEmpty) {
      url = wsUrl;
    }
    if (httpOrigin.isNotEmpty) {
      this.httpOrigin = httpOrigin.replaceAll(RegExp(r'/$'), '');
    }
    if (env.isNotEmpty) {
      this.env = env;
    }
    if (account.isNotEmpty) {
      this.account = account;
      avatar = avatarOf(account);
    }
  }

  Future<void> dropImportedPrefs() async {
    if (account.isNotEmpty) {
      await saveAccount(account);
    }
    await _prefs.remove(_kUrl);
    await _prefs.remove(_kHttp);
    await _prefs.remove(_kAccount);
    await _prefs.remove(_kTokenFallback);
    await _prefs.remove(_kAccountFallback);
  }

  String avatarOf(String account) {
    if (account.isEmpty) {
      return '';
    }
    return _prefs.getString('$_kAvatar.$account')?.trim() ?? '';
  }

  Future<void> saveUrl(String value) async {
    url = value.trim().isEmpty ? defaultUrl : value.trim();
  }

  Future<void> saveDest(String value) async {
    dest = value.trim();
    await _prefs.setString(_kDest, dest);
  }

  Future<void> saveHttpOrigin(String value) async {
    httpOrigin = value.trim().isEmpty
        ? defaultHttp
        : value.trim().replaceAll(RegExp(r'/$'), '');
  }

  Future<void> saveAccount(String value) async {
    account = value.trim();
    await _writeSecret(_kAccount, account, fallbackKey: _kAccountFallback);
    await _prefs.remove(_kAccount);
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
    discardedExpiredToken = false;
    await saveToken('');
    await saveAccount('');
    avatar = '';
  }

  Future<void> useLocal() async {
    await saveUrl(localUrl);
    await saveHttpOrigin(localHttp);
    env = 'dev';
  }

  Future<void> useProd() async {
    await saveUrl(defaultUrl);
    await saveHttpOrigin(defaultHttp);
    env = 'prod';
  }

  Future<void> saveToken(String value) async {
    token = value.trim();
    await _writeSecret(_kToken, token, fallbackKey: _kTokenFallback);
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

  Future<String> _readToken() async {
    try {
      return (await _readSecret(_kToken, fallbackKey: _kTokenFallback)).trim();
    } catch (e, st) {
      KimLogger.warn('readToken', e, st);
      return '';
    }
  }

  Future<String> _readAccount() async {
    try {
      final fromSecret = (await _readSecret(
        _kAccount,
        fallbackKey: _kAccountFallback,
      )).trim();
      if (fromSecret.isNotEmpty) {
        return fromSecret;
      }
    } catch (e, st) {
      KimLogger.warn('readAccount', e, st);
    }
    return _prefs.getString(_kAccount)?.trim() ?? '';
  }

  Future<void> _writeSecret(
    String key,
    String value, {
    required String fallbackKey,
  }) async {
    final secure = _secure;
    if (secure != null) {
      try {
        if (value.isEmpty) {
          await secure.delete(key: key);
        } else {
          await secure.write(key: key, value: value);
        }
        await _prefs.remove(fallbackKey);
        return;
      } catch (e, st) {
        // Missing Keychain entitlement (-34018) must not fail login.
        KimLogger.warn('writeSecret keychain', e, st);
      }
    }
    if (value.isEmpty) {
      _memorySecure.remove(key);
      await _prefs.remove(fallbackKey);
    } else {
      _memorySecure[key] = value;
      if (secure != null) {
        await _prefs.setString(fallbackKey, value);
      }
    }
  }

  Future<String> _readSecret(String key, {required String fallbackKey}) async {
    final secure = _secure;
    if (secure == null) {
      return _memorySecure[key] ?? '';
    }
    try {
      final fromKeychain = (await secure.read(key: key))?.trim() ?? '';
      if (fromKeychain.isNotEmpty) {
        return fromKeychain;
      }
    } catch (e, st) {
      KimLogger.warn('readSecret keychain', e, st);
    }
    return _prefs.getString(fallbackKey)?.trim() ?? _memorySecure[key] ?? '';
  }

  /// Independent of JWT. Created empty until P4 fills the wrapping key.
  Future<String> readAgentWrapKey() async {
    try {
      return (await _readSecret(
        _kWrap,
        fallbackKey: '$_kWrap.fallback',
      )).trim();
    } catch (e, st) {
      KimLogger.warn('readAgentWrapKey', e, st);
      return '';
    }
  }
}
