/// Keychain/Keystore executor: Rust owns the keys and the lifecycle; this
/// file only executes reads/writes/deletes against `flutter_secure_storage`
/// and answers the request channel. Platform storage options live here and
/// nowhere else.
library;

import 'dart:async';

import 'package:flutter_secure_storage/flutter_secure_storage.dart';

import 'package:kim_mobile/core/logger.dart';
import 'package:kim_mobile/src/rust/api/bootstrap.dart' as rust;

/// macOS Data Protection keychain (iOS-style). No login-keychain password
/// dialog, no biometry, no passcode. Available after first unlock.
///
/// Do not set [MacOsOptions.usesDataProtectionKeychain] to false: that
/// stores into the file-based login keychain, which can prompt for the
/// user's login password. Ad-hoc "Sign to Run Locally" still lacks
/// `application-identifier` and may throw -34018; the executor then
/// reports failure without prompting.
const macOsKeychain = MacOsOptions(
  usesDataProtectionKeychain: true,
  accessibility: KeychainAccessibility.first_unlock_this_device,
  synchronizable: false,
  useSecureEnclave: false,
  accessControlFlags: [],
  // kSecUseAuthenticationUIFail — never present an auth UI.
  authenticationUIBehavior: 'u_AuthUIF',
);

/// The one place platform storage options live. iOS Keychain. Android:
/// RSA-OAEP + AES-GCM (EncryptedSharedPreferences was removed in
/// flutter_secure_storage 11; this is the replacement).
FlutterSecureStorage productionSecureStorage() {
  return const FlutterSecureStorage(
    aOptions: AndroidOptions(),
    iOptions: IOSOptions(
      accessibility: KeychainAccessibility.first_unlock_this_device,
    ),
    mOptions: macOsKeychain,
  );
}

class SecretStoreExecutor {
  SecretStoreExecutor({FlutterSecureStorage? secure})
    : _secureOverride = secure;

  final FlutterSecureStorage? _secureOverride;

  StreamSubscription<rust.SecretRequest>? _sub;
  bool _attached = false;

  Future<void> attach() async {
    if (_attached) {
      return;
    }
    _attached = true;
    final secure = _secureOverride ?? productionSecureStorage();
    _sub = rust.watchSecretRequests().listen(
      (req) => unawaited(_execute(secure, req)),
      onError: (Object e, StackTrace st) {
        KimLogger.warn('secret executor stream', e, st);
      },
    );
  }

  Future<void> _execute(
    FlutterSecureStorage secure,
    rust.SecretRequest req,
  ) async {
    try {
      switch (req.op) {
        case 'read':
          final value = await secure.read(key: req.key);
          rust.secretStoreRespond(id: req.id, ok: true, value: value);
        case 'write':
          await secure.write(key: req.key, value: req.value);
          rust.secretStoreRespond(id: req.id, ok: true, value: req.value);
        case 'delete':
          await secure.delete(key: req.key);
          rust.secretStoreRespond(id: req.id, ok: true);
        default:
          rust.secretStoreRespond(id: req.id, ok: false);
      }
    } catch (e, st) {
      KimLogger.warn('secret executor ${req.op} ${req.key}', e, st);
      try {
        rust.secretStoreRespond(id: req.id, ok: false);
      } catch (_) {
        // Channel already closed.
      }
    }
  }

  Future<void> detach() async {
    await _sub?.cancel();
    _sub = null;
    _attached = false;
  }
}
