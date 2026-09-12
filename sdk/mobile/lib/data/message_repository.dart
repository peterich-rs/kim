/// Single write seam for send, push, offline pages, and history.
library;

import '../core/format.dart';
import '../core/image_extra.dart';
import '../kim_bridge.dart';
import '../models/models.dart';
import 'conversation_store.dart';
import 'message_identity.dart';

class MessageRepository {
  MessageRepository(this._store, {this._client});

  final ConversationStore _store;
  final KimClientPort? _client;

  ConversationStore get store => _store;

  KimChatMsg fromTalk({
    required String dest,
    required String sender,
    required String body,
    required String extra,
    required int messageId,
    required int sendTime,
    int msgType = 0,
  }) {
    final parsed = parseImageExtra(extra);
    return KimChatMsg(
      key: incomingMessageKey(
        messageId: messageId,
        sendTime: sendTime,
        sender: sender.isEmpty ? dest : sender,
      ),
      dest: dest,
      sender: sender.isEmpty ? dest : sender,
      body: body,
      at: sendTimeMs(sendTime),
      kind: kindFromWire(body: body, extra: extra, type: msgType),
      width: parsed?.width ?? 0,
      height: parsed?.height ?? 0,
      messageId: messageId,
    );
  }

  KimChatMsg fromHistory(
    KimHistoryMsg row, {
    required String dest,
    required String account,
  }) {
    final extra = parseImageExtra(row.extra);
    final sender = row.sender.isEmpty
        ? (row.direction == 1 ? account : dest)
        : row.sender;
    return KimChatMsg(
      key: incomingMessageKey(
        messageId: row.messageId,
        sendTime: row.sendTime,
        sender: sender,
      ),
      dest: dest,
      sender: sender,
      body: row.body,
      at: sendTimeMs(row.sendTime),
      kind: kindFromWire(body: row.body, extra: row.extra, type: row.msgType),
      width: extra?.width ?? 0,
      height: extra?.height ?? 0,
      messageId: row.messageId,
    );
  }

  Future<List<ApplyResult>> applyOwn(
    String account,
    Iterable<KimChatMsg> msgs, {
    String? viewingDest,
  }) {
    return _apply(
      account,
      msgs,
      policy: UnreadPolicy.keep,
      viewingDest: viewingDest,
    );
  }

  Future<List<ApplyResult>> applyLive(
    String account,
    Iterable<KimChatMsg> msgs, {
    String? viewingDest,
  }) {
    return _apply(
      account,
      msgs,
      policy: UnreadPolicy.ifInserted,
      viewingDest: viewingDest,
    );
  }

  Future<List<ApplyResult>> applySync(
    String account,
    Iterable<KimChatMsg> msgs, {
    String? viewingDest,
  }) {
    return _apply(
      account,
      msgs,
      policy: UnreadPolicy.keep,
      viewingDest: viewingDest,
    );
  }

  Future<List<ApplyResult>> _apply(
    String account,
    Iterable<KimChatMsg> msgs, {
    required UnreadPolicy policy,
    String? viewingDest,
  }) async {
    final client = _client;
    if (client != null && client.rustStoreAttached) {
      // PersistHook / enqueue_message own kim-cache.db. Do not write own
      // rows as incoming sent talks via persistTalks.
      return [
        for (final m in msgs)
          ApplyResult(
            message: m,
            inserted: true,
            unreadDelta: 0,
            thread: KimThread(
              id: m.dest,
              kind: ThreadKind.user,
              title: m.dest,
              lastBody: m.body,
              lastAt: m.at,
            ),
          ),
      ];
    }
    return _store.applyMessages(
      account,
      msgs,
      policy: policy,
      viewingDest: viewingDest,
    );
  }
}
