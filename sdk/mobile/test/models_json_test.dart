import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/models/models.dart';

void main() {
  test('KimThread.fromJson roundtrip and rejects bad payload', () {
    const thread = KimThread(
      id: 'bob',
      kind: ThreadKind.user,
      title: 'Bob',
      lastBody: 'hi',
      lastAt: 12,
      unread: 3,
      avatar: 'a',
    );
    final got = KimThread.fromJson(thread.toJson());
    expect(got.id, 'bob');
    expect(got.title, 'Bob');
    expect(got.unread, 3);
    expect(KimThread.tryFromJson(null), isNull);
    expect(KimThread.tryFromJson({'title': 'x'}), isNull);
    expect(
      () => KimThread.fromJson({'title': 'x'}),
      throwsA(isA<FormatException>()),
    );
  });

  test('KimChatMsg.fromJson roundtrip and rejects bad payload', () {
    const msg = KimChatMsg(
      key: 'k1',
      dest: 'bob',
      sender: 'alice',
      body: 'hi',
      at: 9,
      kind: KimMsgKind.image,
      width: 10,
      height: 8,
    );
    final got = KimChatMsg.fromJson(msg.toJson());
    expect(got.key, 'k1');
    expect(got.kind, KimMsgKind.image);
    expect(got.width, 10);
    expect(KimChatMsg.tryFromJson('nope'), isNull);
    expect(
      () => KimChatMsg.fromJson({'key': 1}),
      throwsA(isA<FormatException>()),
    );
  });
}
