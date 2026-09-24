library;

import 'package:kim_mobile/copy.dart';
import 'package:kim_mobile/core/failures.dart';
import 'package:kim_mobile/src/rust/api/failure.dart';

bool _keychainEntitlement(Object err) {
  final msg = err.toString();
  return msg.contains('-34018') ||
      msg.contains('entitlement isn\'t present') ||
      msg.contains('Unexpected security result code');
}

/// Errors that must not trip Riverpod 3 automatic retry (auth, validation).
bool isPermanentClientError(Object err) {
  final failure = apiFailureOf(err);
  if (failure != null) {
    return !failure.retryable;
  }
  return _keychainEntitlement(err);
}

String mapUserError(Object err) {
  if (_keychainEntitlement(err)) {
    return Copy.sessionPersistFailed;
  }
  return switch (apiFailureOf(err)) {
    ApiFailure_Unauthorized() ||
    ApiFailure_AuthExpired() => Copy.badCredentials,
    ApiFailure_InvalidAccount() => Copy.invalidAccount,
    ApiFailure_InvalidPassword() => Copy.invalidPassword,
    ApiFailure_AccountExists() => Copy.accountExists,
    ApiFailure_NotConnected() => Copy.network,
    ApiFailure_Disk() || ApiFailure_PasswordSeal() => Copy.sessionPersistFailed,
    ApiFailure_Http(:final status) when status >= 500 && status < 600 =>
      Copy.unavailable,
    _ => Copy.unavailable,
  };
}

String mapTalkError(Object err) {
  return switch (apiFailureOf(err)) {
    ApiFailure_NotFriends() => Copy.notFriends,
    ApiFailure_Blocked() => Copy.blocked,
    ApiFailure_UserNotFound() => Copy.userNotFound,
    ApiFailure_CannotChatSelf() => Copy.cannotAddSelf,
    ApiFailure_NotConnected() => Copy.notConnected,
    _ => Copy.sendFailed,
  };
}

String socialFailureCopy(Object err) {
  return switch (apiFailureOf(err)) {
    ApiFailure_UserNotFound() => Copy.userNotFound,
    ApiFailure_Blocked() => Copy.blocked,
    ApiFailure_CannotChatSelf() => Copy.cannotAddSelf,
    ApiFailure_NotFriends() => Copy.notFriends,
    ApiFailure_Protocol(:final status) when status == 113 =>
      Copy.botSocialDenied,
    _ => Copy.sendFailed,
  };
}

String agentRegisterFailureCopy(Object err) {
  if (err is StateError && err.message.isNotEmpty) {
    return err.message;
  }
  return Copy.agentRegisterFailed;
}

bool botAlreadyGone(Object err) {
  return switch (apiFailureOf(err)) {
    ApiFailure_UserNotFound() => true,
    _ => false,
  };
}

String avatarFailureCopy(Object err) {
  if (err.toString().contains(Copy.avatarExportFailed)) {
    return Copy.avatarExportFailed;
  }
  return switch (apiFailureOf(err)) {
    ApiFailure_UnsupportedMedia() => Copy.avatarUnsupportedType,
    ApiFailure_Unauthorized() || ApiFailure_AuthExpired() => Copy.avatarRelogin,
    ApiFailure_NotConnected() => Copy.notConnected,
    _ => Copy.avatarFailed,
  };
}
