library;

import 'package:kim_mobile/bridge/kim_bridge.dart';
import 'package:kim_mobile/models/models.dart';

/// One conversation. Production and tests share this port; the opaque
/// `ConversationHandle` is what [KimBridge] uses underneath.
class ConversationPort {
  ConversationPort(this.client, this.dest);

  final KimClientPort client;
  final String dest;

  Future<List<Map<String, dynamic>>> enter() => client.roomEnter(dest, kind: 0);

  Future<void> leave() async {
    await client.roomLeave(dest, kind: 0);
  }

  Future<void> setTyping(bool active) =>
      client.sendTyping(dest, kind: 0, active: active);

  Future<void> markRead() => client.markConversationRead(dest, ThreadKind.user);

  Future<KimCommandReceipt> sendText({
    required ThreadKind kind,
    required String text,
    required String clientId,
  }) {
    return client.enqueueMessage(
      dest: dest,
      kind: kind,
      content: KimOutgoingContent.text(text),
      clientId: clientId,
    );
  }
}

extension KimClientConversation on KimClientPort {
  ConversationPort conversation(String dest) => ConversationPort(this, dest);
}
