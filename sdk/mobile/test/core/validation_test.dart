import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/core/validation.dart';

void main() {
  test('sanitizePassword drops CR LF injected by macOS NSTextView', () {
    expect(sanitizePassword('secret123\n'), 'secret123');
    expect(sanitizePassword('secret123\r\n'), 'secret123');
    expect(sanitizePassword('sec\rret123'), 'secret123');
    expect(sanitizePassword('secret123'), 'secret123');
  });

  test(
    'password with only a trailing newline still meets length after sanitize',
    () {
      expect(validatePassword(sanitizePassword('secret123\n')), isNull);
    },
  );
}
