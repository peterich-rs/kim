library;

import 'package:flutter_riverpod/flutter_riverpod.dart';

class AuthDraft {
  const AuthDraft({
    this.register = false,
    this.accountErr,
    this.passwordErr,
    this.confirmErr,
  });

  final bool register;
  final String? accountErr;
  final String? passwordErr;
  final String? confirmErr;
}

class AuthDraftNotifier extends Notifier<AuthDraft> {
  AuthDraftNotifier(this._register);

  final bool _register;

  @override
  AuthDraft build() => AuthDraft(register: _register);

  void showErrors({String? account, String? password, String? confirm}) {
    state = AuthDraft(
      register: state.register,
      accountErr: account,
      passwordErr: password,
      confirmErr: confirm,
    );
  }

  void toggleRegister() {
    state = AuthDraft(register: !state.register);
  }
}

final authDraftProvider = NotifierProvider.autoDispose
    .family<AuthDraftNotifier, AuthDraft, bool>(AuthDraftNotifier.new);

class PasswordDraft {
  const PasswordDraft({this.oldErr, this.nextErr, this.confirmErr});

  final String? oldErr;
  final String? nextErr;
  final String? confirmErr;
}

class PasswordDraftNotifier extends Notifier<PasswordDraft> {
  @override
  PasswordDraft build() => const PasswordDraft();

  void showErrors({String? old, String? next, String? confirm}) {
    state = PasswordDraft(oldErr: old, nextErr: next, confirmErr: confirm);
  }
}

final passwordDraftProvider =
    NotifierProvider.autoDispose<PasswordDraftNotifier, PasswordDraft>(
      PasswordDraftNotifier.new,
    );
