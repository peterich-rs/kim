library;

import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:kim_mobile/copy.dart';
import 'package:kim_mobile/core/haptics.dart';
import 'package:kim_mobile/core/logger.dart';
import 'package:kim_mobile/core/secure_origin.dart';
import 'package:kim_mobile/core/user_agent.dart';
import 'package:kim_mobile/features/session/providers.dart';

class AuthState {
  const AuthState({required this.signedIn, required this.account, this.notice});

  factory AuthState.signedOut({String? notice}) =>
      AuthState(signedIn: false, account: '', notice: notice);

  final bool signedIn;
  final String account;
  final String? notice;
}

class AuthNotifier extends Notifier<AuthState> {
  @override
  AuthState build() {
    final settings = ref.watch(runtimeProvider).settings;
    if (settings.token.isEmpty) {
      return AuthState.signedOut(
        notice: settings.discardedExpiredToken ? Copy.sessionExpired : null,
      );
    }
    return AuthState(signedIn: true, account: settings.account);
  }

  Future<void> signIn({
    required bool register,
    required String account,
    required String password,
  }) async {
    final runtime = ref.read(runtimeProvider);
    final auth = ref.read(authPortProvider);
    final ua = kimUserAgent(runtime);
    final origin = runtime.settings.httpOrigin;
    final insecure = insecureAuthOriginReason(origin);
    if (insecure != null) {
      throw StateError(Copy.insecureAuthOrigin);
    }
    final session = register
        ? await auth.register(
            origin: origin,
            userAgent: ua,
            account: account,
            password: password,
          )
        : await auth.login(
            origin: origin,
            userAgent: ua,
            account: account,
            password: password,
          );
    // Persist before any mounted check. saveToken writes memory first, so a
    // rebuild of [build] during I/O can already see a live session. Bailing
    // out before persist is the "server ok, UI stuck on /login" dead zone.
    await runtime.settings.saveSession(
      token: session.token,
      account: session.account,
    );
    if (!ref.mounted) {
      return;
    }
    await KimHaptics.success();
    state = AuthState(signedIn: true, account: session.account);
  }

  Future<void> signOut({bool expired = false, String? notice}) async {
    final runtime = ref.read(runtimeProvider);
    final auth = ref.read(authPortProvider);
    final client = ref.read(clientPortProvider);
    try {
      await client.stopSession();
    } catch (e, st) {
      KimLogger.warn('signOut stopSession', e, st);
    }
    if (!ref.mounted) {
      return;
    }
    try {
      await auth.logout(
        origin: runtime.settings.httpOrigin,
        userAgent: kimUserAgent(runtime),
        token: runtime.settings.token,
      );
    } catch (e, st) {
      KimLogger.warn('signOut logout', e, st);
    }
    if (!ref.mounted) {
      return;
    }
    await runtime.settings.clearSession();
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
    final runtime = ref.read(runtimeProvider);
    final insecure = insecureAuthOriginReason(runtime.settings.httpOrigin);
    if (insecure != null) {
      throw StateError(Copy.insecureAuthOrigin);
    }
    await ref
        .read(authPortProvider)
        .changePassword(
          origin: runtime.settings.httpOrigin,
          userAgent: kimUserAgent(runtime),
          token: runtime.settings.token,
          oldPassword: oldPassword,
          newPassword: newPassword,
        );
    if (!ref.mounted) {
      return;
    }
    await KimHaptics.success();
  }

  Future<void> savePushedToken(String token) async {
    if (token.isEmpty) {
      return;
    }
    await ref.read(runtimeProvider).settings.saveToken(token);
  }
}

final authProvider = NotifierProvider<AuthNotifier, AuthState>(
  AuthNotifier.new,
);
