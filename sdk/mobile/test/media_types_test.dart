import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/copy.dart';
import 'package:kim_mobile/core/image_extra.dart';
import 'package:kim_mobile/core/media.dart';
import 'package:kim_mobile/models/models.dart';

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

  test('image extra round-trips and classifies media URLs', () {
    expect(
      parseImageExtra(encodeImageExtra(width: 1200, height: 800)),
      isA<ImageSize>().having((s) => s.width, 'w', 1200),
    );
    expect(isMediaUrl('https://media.kim.ainexc.com/alice/a.png'), isTrue);
    expect(isMediaUrl('https://example.com/readme'), isFalse);
    expect(
      kindFromWire(body: 'https://media.kim.ainexc.com/a.png', extra: ''),
      KimMsgKind.image,
    );
    expect(
      previewBody(
        const KimChatMsg(
          key: '1',
          dest: 'bob',
          sender: 'alice',
          body: 'https://media.kim.ainexc.com/a.png',
          at: 1,
          kind: KimMsgKind.image,
        ),
      ),
      Copy.imageMessage,
    );
    expect(
      previewSnippet('https://media.kim.ainexc.com/alice/a.png'),
      Copy.imageMessage,
    );
    expect(previewSnippet('/tmp/photo.jpg'), Copy.imageMessage);
    expect(previewSnippet('/tmp/clip.mp4'), Copy.videoMessage);
    expect(previewSnippet(Copy.imageMessage), Copy.imageMessage);
    expect(previewSnippet('hello'), 'hello');
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
