import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/data/conversation_store.dart';
import 'package:kim_mobile/data/message_repository.dart';
import 'package:kim_mobile/models/models.dart';

import 'support/fake_kim.dart';

class _RustStoreFake extends FakeKim {
  final persisted = <KimChatMsg>[];

  @override
  bool get rustStoreAttached => true;

  @override
  Future<void> persistTalks(
    Iterable<KimChatMsg> msgs, {
    required UnreadPolicy policy,
  }) async {
    persisted.addAll(msgs);
  }
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test('rust store apply does not write the dart sqlite isolate', () async {
    final dart = ConversationStore.memory();
    addTearDown(dart.close);
    final client = _RustStoreFake();
    final repo = MessageRepository(dart, client: client);
    const msg = KimChatMsg(
      key: 'm1',
      dest: 'bob',
      sender: 'bob',
      body: 'hi',
      at: 1,
      messageId: 1,
    );
    await repo.applyLive('alice', [msg]);
    expect(client.persisted, isEmpty);
    expect(dart.loadMessages('alice', 'bob'), isEmpty);
    expect(dart.isolateBacked, isFalse);
  });
}
