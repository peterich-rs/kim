library;

import '../copy.dart';

String mapUserError(Object err) {
  final msg = err.toString();
  if (msg.contains('401') || msg.contains('账号或密码错误')) {
    return Copy.badCredentials;
  }
  if (msg.contains('409') || msg.contains('账号已存在')) {
    return Copy.accountExists;
  }
  if (msg.contains('invalid account')) {
    return Copy.invalidAccount;
  }
  if (msg.contains('invalid password')) {
    return Copy.invalidPassword;
  }
  if (msg.contains('timeout') || msg.contains('timed out')) {
    return Copy.timeout;
  }
  if (msg.contains('Failed to fetch') ||
      msg.contains('NetworkError') ||
      msg.contains('Connection refused') ||
      msg.contains('network') ||
      msg.contains('offline')) {
    return Copy.network;
  }
  if (msg.contains('http 5') || msg.contains('status: 5')) {
    return Copy.unavailable;
  }
  return Copy.unavailable;
}
