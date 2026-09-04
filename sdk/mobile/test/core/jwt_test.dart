import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/core/jwt.dart';

import '../support/jwt.dart';

void main() {
  test('reads acc and exp from compact JWT', () {
    final token = testJwt(acc: 'alice', exp: 4_000_000_000);
    expect(JwtPeek.account(token), 'alice');
    expect(JwtPeek.exp(token), 4_000_000_000);
    expect(JwtPeek.isExpired(token), isFalse);
  });

  test('expired exp is expired', () {
    final token = testJwt(acc: 'alice', exp: 1);
    expect(JwtPeek.isExpired(token), isTrue);
  });

  test('exp within skew is expired', () {
    final now = DateTime.fromMillisecondsSinceEpoch(1_700_000_000 * 1000);
    final token = testJwt(acc: 'alice', exp: 1_700_000_000 + 10);
    expect(JwtPeek.isExpired(token, now: now), isTrue);
  });

  test('garbage and missing exp are not treated as expired', () {
    expect(JwtPeek.isExpired('tok.jwt'), isFalse);
    expect(JwtPeek.isExpired('not-a-jwt'), isFalse);
    expect(JwtPeek.account('tok.jwt'), isNull);
    expect(JwtPeek.exp('a.b.c'), isNull);
  });
}
