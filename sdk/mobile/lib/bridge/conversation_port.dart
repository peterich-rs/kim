library;

import 'package:kim_mobile/bridge/kim_bridge.dart';
import 'package:kim_mobile/models/models.dart';
import 'package:kim_mobile/src/rust/api/types.dart' hide ThreadKind;

/// One conversation. The thread kind is fixed at construction.
class ConversationPort {
  ConversationPort(this.client, this.dest, this.kind);

  final KimClientPort client;
  final String dest;
  final ThreadKind kind;

  Future<List<RoomMember>> enter() => client.roomEnter(dest, kind);

  Future<void> leave() => client.roomLeave(dest, kind);

  Future<void> setTyping(bool active) =>
      client.sendTyping(dest, kind, active: active);

  Future<void> markRead() => client.markConversationRead(dest, kind);

  Future<KimCommandReceipt> sendText({required String text}) {
    return client.enqueueMessage(
      dest: dest,
      kind: kind,
      content: OutgoingContent.text(body: text),
    );
  }
}

extension KimClientConversation on KimClientPort {
  ConversationPort conversation(String dest, ThreadKind kind) =>
      ConversationPort(this, dest, kind);
}
