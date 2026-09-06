/// Last WGateway URL + dest in SharedPreferences.
/// JWT only in flutter_secure_storage (Keychain / Android Keystore).
/// Never mint a token in the app. Never commit one.
library;

import 'package:flutter_secure_storage/flutter_secure_storage.dart';
import 'package:shared_preferences/shared_preferences.dart';

import 'jwt.dart';

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

  /// Used only when Keychain throws (macOS ad-hoc -34018). Not the happy path.
  static const _kTokenFallback = 'kim.jwt.fallback';

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
  bool discardedExpiredToken = false;

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
    if (account.isEmpty && token.isNotEmpty) {
      final acc = JwtPeek.account(token);
      if (acc != null && acc.isNotEmpty) {
        await saveAccount(acc);
      }
    }
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
    discardedExpiredToken = false;
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
    final secure = _secure;
    if (secure != null) {
      try {
        if (token.isEmpty) {
          await secure.delete(key: _kToken);
        } else {
          await secure.write(key: _kToken, value: token);
        }
        await _prefs.remove(_kTokenFallback);
        return;
      } catch (_) {
        // Missing Keychain entitlement (-34018) must not fail login.
      }
    }
    if (token.isEmpty) {
      _memorySecure.remove(_kToken);
      await _prefs.remove(_kTokenFallback);
    } else {
      _memorySecure[_kToken] = token;
      if (secure != null) {
        await _prefs.setString(_kTokenFallback, token);
      }
    }
  }

  Future<void> markNotificationsAsked() async {
    notificationsAsked = true;
    await _prefs.setBool(_kNotifAsked, true);
  }

  Future<String> _readToken() async {
    try {
      final raw = (await _readTokenRaw()).trim();
      if (raw.isEmpty) {
        return '';
      }
      if (JwtPeek.isExpired(raw)) {
        discardedExpiredToken = true;
        await saveToken('');
        return '';
      }
      return raw;
    } catch (_) {
      // Missing plugin / Keystore errors: treat as empty, never mint.
      return '';
    }
  }

  Future<String> _readTokenRaw() async {
    final secure = _secure;
    if (secure == null) {
      return _memorySecure[_kToken] ?? '';
    }
    try {
      final fromKeychain = (await secure.read(key: _kToken))?.trim() ?? '';
      if (fromKeychain.isNotEmpty) {
        return fromKeychain;
      }
    } catch (_) {}
    return _prefs.getString(_kTokenFallback)?.trim() ??
        _memorySecure[_kToken] ??
        '';
  }
}
