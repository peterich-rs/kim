import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/core/media.dart';

void main() {
  test('sniffs png jpeg gif webp magic bytes', () {
    expect(
      KimImageTypes.sniff([0x89, 0x50, 0x4E, 0x47, 0, 0, 0, 0]),
      'image/png',
    );
    expect(KimImageTypes.sniff([0xFF, 0xD8, 0xFF, 0, 0]), 'image/jpeg');
    expect(
      KimImageTypes.sniff([0x47, 0x49, 0x46, 0x38, 0x39, 0x61]),
      'image/gif',
    );
    expect(
      KimImageTypes.sniff([
        0x52,
        0x49,
        0x46,
        0x46,
        0,
        0,
        0,
        0,
        0x57,
        0x45,
        0x42,
        0x50,
      ]),
      'image/webp',
    );
  });

  test('normalize maps jpg and falls back to sniff', () {
    expect(KimImageTypes.normalize('image/jpg', const []), 'image/jpeg');
    expect(
      KimImageTypes.normalize('image/heic', [
        0x89,
        0x50,
        0x4E,
        0x47,
        0,
        0,
        0,
        0,
      ]),
      'image/png',
    );
    expect(KimImageTypes.normalize('image/png', const [1, 2, 3]), 'image/png');
  });

  test('normalize rejects unknown types', () {
    expect(
      () => KimImageTypes.normalize('image/heic', const [1, 2, 3]),
      throwsA(
        isA<StateError>().having(
          (e) => e.message,
          'message',
          'unsupported media type',
        ),
      ),
    );
  });
}
