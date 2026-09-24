import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/copy.dart';
import 'package:kim_mobile/core/errors.dart';
import 'package:kim_mobile/core/failures.dart';
import 'package:kim_mobile/features/agent/agent_profiles.dart';
import 'package:kim_mobile/features/contacts/contacts.dart';
import 'package:kim_mobile/features/session/retry.dart';
import 'package:kim_mobile/src/rust/api/failure.dart';

void main() {
  test('retryable matches SdkError::retryable', () {
    expect(const ApiFailure.notConnected().retryable, isTrue);
    expect(const ApiFailure.busy(queue: 'outbox').retryable, isTrue);
    expect(const ApiFailure.sqliteBusy().retryable, isTrue);
    expect(const ApiFailure.rateLimited(retryAfterMs: 1000).retryable, isTrue);
    expect(const ApiFailure.unauthorized().retryable, isFalse);
    expect(const ApiFailure.authExpired().retryable, isFalse);
    expect(const ApiFailure.storageFull().retryable, isFalse);
    expect(const ApiFailure.notFriends(dest: 'bob').retryable, isFalse);
    expect(const ApiFailure.internal(message: 'x').retryable, isFalse);
  });

  test('retryableSend includes protocol 3 and 3xx', () {
    expect(const ApiFailure.protocol(status: 3).retryableSend, isTrue);
    expect(const ApiFailure.protocol(status: 350).retryableSend, isTrue);
    expect(const ApiFailure.protocol(status: 113).retryableSend, isFalse);
    expect(const ApiFailure.protocol(status: 101).retryableSend, isFalse);
    expect(const ApiFailure.notConnected().retryableSend, isTrue);
    expect(const ApiFailure.unauthorized().retryableSend, isFalse);
  });

  test(
    'mapTalkError switches on variant, not a status string in another field',
    () {
      expect(
        mapTalkError(const ApiFailure.notFriends(dest: 'status 110')),
        Copy.notFriends,
      );
      expect(
        mapTalkError(const ApiFailure.blocked(dest: 'status 109')),
        Copy.blocked,
      );
      expect(
        mapTalkError(const ApiFailure.userNotFound(dest: 'status 101')),
        Copy.userNotFound,
      );
      expect(
        mapTalkError(const ApiFailure.cannotChatSelf()),
        Copy.cannotAddSelf,
      );
      expect(
        mapTalkError(const ApiFailure.protocol(status: 109)),
        Copy.sendFailed,
      );
    },
  );

  test('mapUserError uses auth variants', () {
    expect(mapUserError(const ApiFailure.unauthorized()), Copy.badCredentials);
    expect(
      mapUserError(const ApiFailure.invalidAccount()),
      Copy.invalidAccount,
    );
    expect(
      mapUserError(const ApiFailure.invalidPassword()),
      Copy.invalidPassword,
    );
    expect(mapUserError(const ApiFailure.accountExists()), Copy.accountExists);
    expect(mapUserError(const ApiFailure.notConnected()), Copy.network);
    expect(mapUserError(const ApiFailure.http(status: 503)), Copy.unavailable);
  });

  test('kimRetry uses ApiFailure.retryable', () {
    expect(kimRetry(0, const ApiFailure.unauthorized()), isNull);
    expect(kimRetry(0, const ApiFailure.accountExists()), isNull);
    expect(kimRetry(0, const ApiFailure.notConnected()), isNotNull);
    expect(kimRetry(0, const ApiFailure.notFriends(dest: 'bob')), isNull);
  });

  test('socialError maps protocol 113 by status field', () {
    expect(
      socialError(const ApiFailure.userNotFound(dest: 'x')),
      Copy.userNotFound,
    );
    expect(socialError(const ApiFailure.blocked(dest: 'x')), Copy.blocked);
    expect(socialError(const ApiFailure.cannotChatSelf()), Copy.cannotAddSelf);
    expect(
      socialError(const ApiFailure.notFriends(dest: 'x')),
      Copy.notFriends,
    );
    expect(
      socialError(const ApiFailure.protocol(status: 113)),
      Copy.botSocialDenied,
    );
    expect(
      socialError(const ApiFailure.protocol(status: 109)),
      Copy.sendFailed,
    );
  });

  test('bot gone and register copy follow variants', () {
    expect(
      isBotAlreadyGone(const ApiFailure.userNotFound(dest: 'bot')),
      isTrue,
    );
    expect(isBotAlreadyGone(const ApiFailure.protocol(status: 108)), isFalse);
    expect(
      agentRegisterError(const ApiFailure.protocol(status: 2)),
      Copy.agentRegisterFailed,
    );
    expect(agentRegisterError(StateError('need a name')), 'need a name');
  });
}
