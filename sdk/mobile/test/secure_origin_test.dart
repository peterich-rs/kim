import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/core/secure_origin.dart';

void main() {
  test('allows https and loopback http', () {
    expect(isSecureAuthOrigin('https://kim.ainexc.com'), isTrue);
    expect(isSecureAuthOrigin('http://127.0.0.1:8080'), isTrue);
    expect(isSecureAuthOrigin('http://localhost:8080'), isTrue);
    expect(isSecureAuthOrigin('http://evil.example'), isFalse);
    expect(isSecureAuthOrigin('http://192.168.1.1:8080'), isFalse);
  });
}
