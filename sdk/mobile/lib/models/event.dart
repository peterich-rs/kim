library;

import 'thread.dart';

enum KimEventKind {
  talk,
  kick,
  friend,
  friendAccepted,
  profileUpdated,
  presence,
  typing,
  receiptRead,
  group,
  token,
  closed,
  link,
  inbox,
  syncProgress,
  syncDone,
  syncFailed,
  authExpired,
  syncPage,
}

class KimEvent {
  const KimEvent({
    required this.kind,
    this.dest = '',
    this.sender = '',
    this.body = '',
    this.extra = '',
    this.messageId = 0,
    this.sendTime = 0,
    this.token = '',
    this.exp = 0,
    this.state = '',
    this.attempt = 0,
    this.inbox = const [],
    this.pulled = 0,
    this.pagePending = false,
    this.error = '',
    this.msgType = 0,
    this.nickname = '',
    this.talks = const [],
    this.pageId = 0,
  });

  final KimEventKind kind;
  final String dest;
  final String sender;
  final String body;
  final String extra;
  final int messageId;
  final int sendTime;
  final String token;
  final int exp;
  final String state;
  final int attempt;
  final List<KimThread> inbox;
  final int pulled;
  final bool pagePending;
  final String error;
  final int msgType;
  final String nickname;
  final List<KimEvent> talks;
  final int pageId;
}
