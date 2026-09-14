library;

import '../copy.dart';
import 'failures.dart';

/// Errors that must not trip Riverpod 3 automatic retry (auth, validation).
bool isPermanentClientError(Object err) {
  final kim = KimException.tryFrom(err);
  if (kim != null) {
    return !kim.retryable;
  }
  final msg = err.toString();
  return msg.contains('401') ||
      msg.contains('409') ||
      msg.contains('-34018') ||
      msg.contains('entitlement isn\'t present') ||
      msg.contains('账号或密码错误') ||
      msg.contains('账号已存在') ||
      msg.contains('invalid account') ||
      msg.contains('invalid password') ||
      msg.contains('unauthorized') ||
      msg.contains('invalid token');
}

String mapUserError(Object err) {
  final kim = KimException.tryFrom(err);
  if (kim != null) {
    return switch (kim.kind) {
      KimErrorKind.unauthorized ||
      KimErrorKind.authExpired => Copy.badCredentials,
      KimErrorKind.invalidArgument => Copy.invalidAccount,
      KimErrorKind.notConnected => Copy.network,
      KimErrorKind.disk => Copy.sessionPersistFailed,
      _ => Copy.unavailable,
    };
  }
  final msg = err.toString();
  if (msg.contains('-34018') ||
      msg.contains('entitlement isn\'t present') ||
      msg.contains('Unexpected security result code')) {
    return Copy.sessionPersistFailed;
  }
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
  final kim = KimException.tryFrom(err);
  if (kim == null) {
    return Copy.sendFailed;
  }
  return switch (kim.kind) {
    KimErrorKind.notFriends => Copy.notFriends,
    KimErrorKind.blocked => Copy.blocked,
    KimErrorKind.userNotFound => Copy.userNotFound,
    KimErrorKind.cannotChatSelf => Copy.cannotAddSelf,
    KimErrorKind.notConnected => Copy.notConnected,
    _ => Copy.sendFailed,
  };
}
