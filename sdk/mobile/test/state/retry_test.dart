import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/copy.dart';
import 'package:kim_mobile/core/errors.dart';
import 'package:kim_mobile/features/session/retry.dart';
import 'package:kim_mobile/src/rust/api/failure.dart';

void main() {
  test('permanent auth errors are not retried', () {
    expect(isPermanentClientError(const ApiFailure.unauthorized()), isTrue);
    expect(kimRetry(0, const ApiFailure.unauthorized()), isNull);
    expect(kimRetry(0, const ApiFailure.accountExists()), isNull);
  });

  test('macOS Keychain -34018 is not bad credentials', () {
    const err =
        'PlatformException(Unexpected security result code, Code: -34018, Message: A required entitlement isn\'t present., -34018, null)';
    expect(isPermanentClientError(Exception(err)), isTrue);
    expect(mapUserError(Exception(err)), Copy.sessionPersistFailed);
    expect(mapUserError(Exception(err)), isNot(Copy.badCredentials));
  });

  test('transient network errors use default backoff', () {
    final delay = kimRetry(0, Exception('Connection refused'));
    expect(delay, const Duration(milliseconds: 200));
    expect(
      kimRetry(1, Exception('Connection refused')),
      const Duration(milliseconds: 400),
    );
  });

  test('talk errors map by variant', () {
    expect(
      mapTalkError(const ApiFailure.notFriends(dest: 'x')),
      Copy.notFriends,
    );
    expect(mapTalkError(const ApiFailure.blocked(dest: 'x')), Copy.blocked);
  });
}
