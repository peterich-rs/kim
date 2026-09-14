import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/copy.dart';
import 'package:kim_mobile/core/errors.dart';
import 'package:kim_mobile/core/failures.dart';
import 'package:kim_mobile/src/rust/api/types.dart';
import 'package:kim_mobile/features/session/retry.dart';

const _allKinds = <String>[
  'not_friends',
  'blocked',
  'user_not_found',
  'cannot_chat_self',
  'auth_expired',
  'unauthorized',
  'not_connected',
  'storage_full',
  'sqlite_busy',
  'disk',
  'rate_limited',
  'payload_too_large',
  'unsupported_media',
  'busy',
  'stale_epoch',
  'not_found',
  'invalid_argument',
  'protocol',
  'internal',
];

void main() {
  test('fromDto maps all 19 SdkError kinds', () {
    expect(_allKinds, hasLength(19));
    expect({for (final k in _allKinds) k}, hasLength(19));
    for (final k in _allKinds) {
      final ex = KimException.fromDto(SdkErrorDto(kind: k, message: k));
      expect(ex.kind.wire, k);
      expect(KimErrorKind.values, contains(ex.kind));
    }
  });

  test('unknown kind maps to internal', () {
    final ex = KimException.fromDto(
      const SdkErrorDto(kind: 'other', message: 'x'),
    );
    expect(ex.kind, KimErrorKind.internal);
    expect(ex.retryable, isFalse);
  });

  test('retryable matches SdkError::retryable', () {
    bool retryable(String kind) =>
        KimException.fromDto(SdkErrorDto(kind: kind, message: 'x')).retryable;
    expect(retryable('not_connected'), isTrue);
    expect(retryable('busy'), isTrue);
    expect(retryable('sqlite_busy'), isTrue);
    expect(retryable('rate_limited'), isTrue);
    expect(retryable('unauthorized'), isFalse);
    expect(retryable('auth_expired'), isFalse);
    expect(retryable('storage_full'), isFalse);
    expect(retryable('not_friends'), isFalse);
    expect(retryable('internal'), isFalse);
  });

  test('mapTalkError switches on kind not message substring', () {
    expect(
      mapTalkError(
        const SdkErrorDto(kind: 'not_friends', message: 'status 110'),
      ),
      Copy.notFriends,
    );
    expect(
      mapTalkError(const SdkErrorDto(kind: 'blocked', message: 'status 109')),
      Copy.blocked,
    );
    expect(
      mapTalkError(
        const SdkErrorDto(kind: 'user_not_found', message: 'status 101'),
      ),
      Copy.userNotFound,
    );
    expect(
      mapTalkError(
        KimException.fromDto(
          const SdkErrorDto(kind: 'cannot_chat_self', message: 'x'),
        ),
      ),
      Copy.cannotAddSelf,
    );
    expect(
      mapTalkError(const SdkErrorDto(kind: 'protocol', message: 'status 109')),
      Copy.sendFailed,
    );
  });

  test('kimRetry uses KimException.retryable', () {
    expect(
      kimRetry(
        0,
        KimException.fromDto(
          const SdkErrorDto(kind: 'unauthorized', message: 'x'),
        ),
      ),
      isNull,
    );
    expect(
      kimRetry(
        0,
        KimException.fromDto(
          const SdkErrorDto(kind: 'not_connected', message: 'x'),
        ),
      ),
      isNotNull,
    );
    expect(
      kimRetry(0, const SdkErrorDto(kind: 'not_friends', message: 'x')),
      isNull,
    );
  });
}
