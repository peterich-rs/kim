library;

import '../copy.dart';

/// Errors that must not trip Riverpod 3 automatic retry (auth, validation).
bool isPermanentClientError(Object err) {
  final msg = err.toString();
  return msg.contains('401') ||
      msg.contains('409') ||
      msg.contains('账号或密码错误') ||
      msg.contains('账号已存在') ||
      msg.contains('invalid account') ||
      msg.contains('invalid password') ||
      msg.contains('unauthorized') ||
      msg.contains('invalid token') ||
      msg.contains('status 101') ||
      msg.contains('status 105') ||
      msg.contains('status 108') ||
      msg.contains('status 109') ||
      msg.contains('status 110');
}

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

String mapTalkError(Object err) {
  final msg = err.toString();
  if (msg.contains(Copy.notConnected) || msg.contains('not connected')) {
    return Copy.notConnected;
  }
  if (msg.contains('status 109') || msg.contains(Copy.notFriends)) {
    return Copy.notFriends;
  }
  if (msg.contains('status 110') || msg.contains(Copy.blocked)) {
    return Copy.blocked;
  }
  if (msg.contains('status 108') || msg.contains(Copy.userNotFound)) {
    return Copy.userNotFound;
  }
  if (msg.contains('status 101') || msg.contains(Copy.cannotAddSelf)) {
    return Copy.cannotAddSelf;
  }
  return Copy.sendFailed;
}
