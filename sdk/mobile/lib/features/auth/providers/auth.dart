library;

import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:kim_mobile/copy.dart';
import 'package:kim_mobile/core/haptics.dart';
import 'package:kim_mobile/core/logger.dart';
import 'package:kim_mobile/features/session/providers.dart';

class AuthState {
  const AuthState({required this.signedIn, required this.account, this.notice});

  factory AuthState.signedOut({String? notice}) =>
      AuthState(signedIn: false, account: '', notice: notice);

  final bool signedIn;
  final String account;
  final String? notice;
}

/// Rust-owned credential presence: usable JWT in the secure store.
final storedAuthProvider = FutureProvider<AuthState>((ref) async {
  final client = ref.watch(clientPortProvider);
  final runtime = ref.watch(runtimeProvider);
  try {
    final hasToken = await client.hasStoredToken();
    if (!hasToken) {
      return AuthState.signedOut();
    }
    final account = await client.storedAccount();
    runtime.settings.account = account;
    return AuthState(signedIn: true, account: account);
  } catch (e, st) {
    KimLogger.warn('storedAuth', e, st);
    return AuthState.signedOut();
  }
});

class AuthNotifier extends Notifier<AuthState> {
  String? _notice;

  @override
  AuthState build() {
    // Token presence is Rust-owned; this is its projection.
    final gated = ref.watch(storedAuthProvider);
    final projected = gated.maybeWhen(
      data: (v) => v,
      orElse: () => AuthState.signedOut(),
    );
    if (!projected.signedIn && _notice != null) {
      return AuthState.signedOut(notice: _notice);
    }
    return projected;
  }

  Future<void> signIn({
    required bool register,
    required String account,
    required String password,
  }) async {
    final runtime = ref.read(runtimeProvider);
    final auth = ref.read(authPortProvider);
    final client = ref.read(clientPortProvider);
    _notice = null;
    KimLogger.info(register ? 'register' : 'login');
    final session = register
        ? await auth.register(account: account, password: password)
        : await auth.login(account: account, password: password);
    // Rust owns persistence from here: Keychain write via the executor.
    await client.storeAuth(token: session.token, account: session.account);
    runtime.settings.account = session.account;
    if (!ref.mounted) {
      return;
    }
    await KimHaptics.success();
    KimLogger.info('signed in account=${session.account}');
    state = AuthState(signedIn: true, account: session.account);
  }

  Future<void> signOut({bool expired = false, String? notice}) async {
    KimLogger.info('signOut expired=$expired');
    _notice = notice ?? (expired ? Copy.sessionExpired : null);
    final runtime = ref.read(runtimeProvider);
    final client = ref.read(clientPortProvider);
    try {
      await client.stopSession();
    } catch (e, st) {
      KimLogger.warn('signOut stopSession', e, st);
    }
    // Server logout with the stored credential; Rust reads its own token.
    try {
      await client.authLogout();
    } catch (e, st) {
      KimLogger.warn('signOut logout', e, st);
    }
    await client.clearAuth();
    runtime.settings.account = '';
    ref.invalidate(storedAuthProvider);
    // Settle the projection before publishing signed-out, so a stale
    // in-flight read cannot paint the user back in.
    try {
      await ref.read(storedAuthProvider.future);
    } catch (e, st) {
      KimLogger.warn('signOut storedAuth', e, st);
    }
    if (!ref.mounted) {
      return;
    }
    final message = notice ?? (expired ? Copy.sessionExpired : null);
    if (message != null) {
      await KimHaptics.error();
      state = AuthState.signedOut(notice: message);
      return;
    }
    await KimHaptics.success();
    state = AuthState.signedOut();
  }

  Future<void> changePassword({
    required String oldPassword,
    required String newPassword,
  }) async {
    await ref
        .read(clientPortProvider)
        .authChangePassword(oldPassword: oldPassword, newPassword: newPassword);
    if (!ref.mounted) {
      return;
    }
    await KimHaptics.success();
  }
}

final authProvider = NotifierProvider<AuthNotifier, AuthState>(
  AuthNotifier.new,
);
