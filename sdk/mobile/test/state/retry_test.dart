import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/core/errors.dart';
import 'package:kim_mobile/state/retry.dart';

void main() {
  test('permanent auth errors are not retried', () {
    expect(isPermanentClientError(Exception('http 401: 账号或密码错误')), isTrue);
    expect(kimRetry(0, Exception('http 401: 账号或密码错误')), isNull);
    expect(kimRetry(0, Exception('http 409: 账号已存在')), isNull);
  });

  test('transient network errors use default backoff', () {
    final delay = kimRetry(0, Exception('Connection refused'));
    expect(delay, const Duration(milliseconds: 200));
    expect(
      kimRetry(1, Exception('Connection refused')),
      const Duration(milliseconds: 400),
    );
  });
}
