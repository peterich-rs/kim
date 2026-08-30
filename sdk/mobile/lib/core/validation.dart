library;

import '../copy.dart';

final _account = RegExp(r'^[A-Za-z0-9_]{3,32}$');

String? validateAccount(String raw) {
  if (!_account.hasMatch(raw.trim())) {
    return Copy.invalidAccount;
  }
  return null;
}

String? validatePassword(String raw) {
  if (raw.length < 8 || raw.length > 128) {
    return Copy.invalidPassword;
  }
  return null;
}

String? validateConfirm(String password, String confirm) {
  if (password != confirm) {
    return Copy.mismatch;
  }
  return null;
}
