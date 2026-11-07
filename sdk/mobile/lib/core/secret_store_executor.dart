/// Keychain/Keystore executor: Rust owns the keys and the lifecycle; this
/// file only executes reads/writes/deletes over the `kim.keystore`
/// MethodChannel and answers the request channel. Platform storage options
/// live here and nowhere else.
library;

import 'dart:async';

import 'package:flutter/services.dart';

import 'package:kim_mobile/core/logger.dart';
import 'package:kim_mobile/src/rust/api/bootstrap.dart' as rust;

/// The one platform-keystore entry point. iOS/macOS: Keychain generic
/// passwords (`kSecAttrAccessibleAfterFirstUnlockThisDevice`; macOS uses
/// the Data-Protection keychain with no auth UI). Android: Keystore AES
/// key wrapping a SharedPreferences blob (AES/GCM/NoPadding). Implemented
/// in `ios/Runner/AppDelegate.swift`, `macos/Runner/KimKeystoreCore.swift`,
/// and `KimKeystore.kt`.
const kimKeystoreChannel = 'kim.keystore';

abstract class KeystoreBackend {
  Future<String?> read(String key);
  Future<void> write(String key, String value);
  Future<void> delete(String key);
}

class KimKeystore implements KeystoreBackend {
  KimKeystore({MethodChannel? channel})
    : _channel = channel ?? const MethodChannel(kimKeystoreChannel);

  final MethodChannel _channel;

  @override
  Future<String?> read(String key) async {
    return _channel.invokeMethod<String>('read', {'key': key});
  }

  @override
  Future<void> write(String key, String value) async {
    await _channel.invokeMethod<void>('write', {'key': key, 'value': value});
  }

  @override
  Future<void> delete(String key) async {
    await _channel.invokeMethod<void>('delete', {'key': key});
  }
}

/// Storage the executor answers with when no channel is attached (tests,
/// web build). Everything reports empty without touching a platform store.
class InMemoryKeystore implements KeystoreBackend {
  final Map<String, String> _values = {};

  @override
  Future<String?> read(String key) async => _values[key];

  @override
  Future<void> write(String key, String value) async {
    _values[key] = value;
  }

  @override
  Future<void> delete(String key) async {
    _values.remove(key);
  }
}

class SecretStoreExecutor {
  SecretStoreExecutor({KeystoreBackend? keystore})
    : _keystoreOverride = keystore;

  final KeystoreBackend? _keystoreOverride;

  StreamSubscription<rust.SecretRequest>? _sub;
  bool _attached = false;

  Future<void> attach() async {
    if (_attached) {
      return;
    }
    _attached = true;
    final keystore = _keystoreOverride ?? KimKeystore();
    _sub = rust.watchSecretRequests().listen(
      (req) => unawaited(_execute(keystore, req)),
      onError: (Object e, StackTrace st) {
        KimLogger.warn('secret executor stream', e, st);
      },
    );
  }

  Future<void> _execute(
    KeystoreBackend keystore,
    rust.SecretRequest req,
  ) async {
    try {
      switch (req.op) {
        case 'read':
          final value = await keystore.read(req.key);
          rust.secretStoreRespond(id: req.id, ok: true, value: value);
        case 'write':
          await keystore.write(req.key, req.value);
          rust.secretStoreRespond(id: req.id, ok: true, value: req.value);
        case 'delete':
          await keystore.delete(req.key);
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
