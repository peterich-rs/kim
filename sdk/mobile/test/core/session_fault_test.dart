import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/core/session_fault.dart';

void main() {
  test('identity kinds latch session end', () {
    expect(classifySessionFault('auth_expired'), SessionFault.identityExpired);
    expect(classifySessionFault('unauthorized'), SessionFault.identityExpired);
    expect(classifySessionFault('auth-failed'), SessionFault.identityExpired);
    expect(classifySessionFault('kickout'), SessionFault.kicked);
  });

  test('transport last_error is not identity end', () {
    expect(classifySessionFault('connect-fail'), isNull);
    expect(classifySessionFault(null), isNull);
  });
}
