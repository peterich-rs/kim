/// Dart view of kim-sdk `SdkError` (19 kinds). No string matching on messages.
library;

import 'package:kim_mobile/src/rust/api/types.dart' show SdkErrorDto;

enum KimErrorKind {
  notFriends,
  blocked,
  userNotFound,
  cannotChatSelf,
  authExpired,
  unauthorized,
  notConnected,
  storageFull,
  sqliteBusy,
  disk,
  rateLimited,
  payloadTooLarge,
  unsupportedMedia,
  busy,
  staleEpoch,
  notFound,
  invalidArgument,
  protocol,
  internal;

  static KimErrorKind fromWire(String kind) {
    return switch (kind) {
      'not_friends' => KimErrorKind.notFriends,
      'blocked' => KimErrorKind.blocked,
      'user_not_found' => KimErrorKind.userNotFound,
      'cannot_chat_self' => KimErrorKind.cannotChatSelf,
      'auth_expired' => KimErrorKind.authExpired,
      'unauthorized' => KimErrorKind.unauthorized,
      'not_connected' => KimErrorKind.notConnected,
      'storage_full' => KimErrorKind.storageFull,
      'sqlite_busy' => KimErrorKind.sqliteBusy,
      'disk' => KimErrorKind.disk,
      'rate_limited' => KimErrorKind.rateLimited,
      'payload_too_large' => KimErrorKind.payloadTooLarge,
      'unsupported_media' => KimErrorKind.unsupportedMedia,
      'busy' => KimErrorKind.busy,
      'stale_epoch' => KimErrorKind.staleEpoch,
      'not_found' => KimErrorKind.notFound,
      'invalid_argument' => KimErrorKind.invalidArgument,
      'protocol' => KimErrorKind.protocol,
      'internal' => KimErrorKind.internal,
      _ => KimErrorKind.internal,
    };
  }

  String get wire => switch (this) {
    KimErrorKind.notFriends => 'not_friends',
    KimErrorKind.blocked => 'blocked',
    KimErrorKind.userNotFound => 'user_not_found',
    KimErrorKind.cannotChatSelf => 'cannot_chat_self',
    KimErrorKind.authExpired => 'auth_expired',
    KimErrorKind.unauthorized => 'unauthorized',
    KimErrorKind.notConnected => 'not_connected',
    KimErrorKind.storageFull => 'storage_full',
    KimErrorKind.sqliteBusy => 'sqlite_busy',
    KimErrorKind.disk => 'disk',
    KimErrorKind.rateLimited => 'rate_limited',
    KimErrorKind.payloadTooLarge => 'payload_too_large',
    KimErrorKind.unsupportedMedia => 'unsupported_media',
    KimErrorKind.busy => 'busy',
    KimErrorKind.staleEpoch => 'stale_epoch',
    KimErrorKind.notFound => 'not_found',
    KimErrorKind.invalidArgument => 'invalid_argument',
    KimErrorKind.protocol => 'protocol',
    KimErrorKind.internal => 'internal',
  };
}

sealed class KimException implements Exception {
  const KimException({required this.kind, required this.message, this.cause});

  factory KimException.fromDto(SdkErrorDto dto) {
    return KimSdkException(
      kind: KimErrorKind.fromWire(dto.kind),
      message: dto.message,
      cause: dto,
    );
  }

  final KimErrorKind kind;
  final String message;
  final Object? cause;

  /// Transport / congestion. Aligns with `SdkError::retryable`.
  bool get retryable => switch (kind) {
    KimErrorKind.notConnected ||
    KimErrorKind.busy ||
    KimErrorKind.sqliteBusy ||
    KimErrorKind.rateLimited => true,
    _ => false,
  };

  static KimException? tryFrom(Object err) {
    return switch (err) {
      KimException e => e,
      SdkErrorDto dto => KimException.fromDto(dto),
      _ => null,
    };
  }

  @override
  String toString() => 'KimException(${kind.wire}): $message';
}

final class KimSdkException extends KimException {
  const KimSdkException({
    required super.kind,
    required super.message,
    super.cause,
  });
}
