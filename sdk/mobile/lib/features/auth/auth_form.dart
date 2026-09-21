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

  AuthDraft copyWith({
    bool? register,
    String? accountErr,
    String? passwordErr,
    String? confirmErr,
    bool clearErrors = false,
  }) {
    return AuthDraft(
      register: register ?? this.register,
      accountErr: clearErrors ? accountErr : (accountErr ?? this.accountErr),
      passwordErr: clearErrors
          ? passwordErr
          : (passwordErr ?? this.passwordErr),
      confirmErr: clearErrors ? confirmErr : (confirmErr ?? this.confirmErr),
    );
  }
}

class AuthDraftNotifier extends Notifier<AuthDraft> {
  @override
  AuthDraft build() => const AuthDraft();

  void setRegister(bool register) {
    state = state.copyWith(register: register, clearErrors: true);
  }

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

final authDraftProvider =
    NotifierProvider.autoDispose<AuthDraftNotifier, AuthDraft>(
      AuthDraftNotifier.new,
    );

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
