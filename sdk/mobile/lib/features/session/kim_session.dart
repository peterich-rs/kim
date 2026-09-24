library;

import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:kim_mobile/copy.dart';
import 'package:kim_mobile/core/session_fault.dart';
import 'package:kim_mobile/models/models.dart';
import 'package:kim_mobile/src/rust/api/types.dart';
import 'package:kim_mobile/features/auth/providers/auth.dart';
import 'package:kim_mobile/features/session/providers.dart';

SessionSnapshot emptySessionSnapshot() => const SessionSnapshot(
  link: LinkState.offline(),
  threads: [],
  unreadTotal: 0,
);

KimLinkState kimLinkFrom(LinkState link, String? lastError) {
  final error = sessionFaultIsIdentity(lastError) ? null : lastError;
  return switch (link) {
    LinkState_Connecting() => KimLinkState(
      status: ConnStatus.connecting,
      error: error,
    ),
    LinkState_Online() => const KimLinkState(status: ConnStatus.online),
    LinkState_Reconnecting(:final attempt) => KimLinkState(
      status: ConnStatus.reconnecting,
      attempt: attempt,
      error: error,
    ),
    LinkState_Offline() => KimLinkState(
      status: ConnStatus.offline,
      error: error,
    ),
  };
}

KimThread kimThreadFrom(ThreadView t) {
  return KimThread(
    id: t.id,
    kind: t.kind == 1 ? ThreadKind.group : ThreadKind.user,
    title: t.title.isEmpty ? t.id : t.title,
    lastBody: t.lastBody,
    lastAt: t.lastAt.toInt(),
    unread: t.unread,
    avatar: t.avatar,
  );
}

KimChatMsg kimChatFrom(MessageView m) {
  final status = switch (m.sendStatus) {
    SendStatus.failed || SendStatus.cancelled => KimSendStatus.failed,
    SendStatus.sent => KimSendStatus.sent,
    _ => KimSendStatus.sending,
  };
  final kind = switch (m.kind) {
    2 => KimMsgKind.image,
    4 => KimMsgKind.video,
    _ => KimMsgKind.text,
  };
  return KimChatMsg(
    key: m.key,
    dest: m.dest,
    sender: m.sender,
    body: m.body,
    at: m.at.toInt(),
    sys: m.sys,
    failed: status == KimSendStatus.failed,
    kind: kind,
    width: m.width,
    height: m.height,
    messageId: m.messageId.toInt(),
    batchId: m.batchId,
    status: status,
    localPath: m.localPath,
  );
}

class KimSessionNotifier extends Notifier<SessionSnapshot> {
  StreamSubscription<SessionSnapshot>? _sub;

  @override
  SessionSnapshot build() {
    ref.listen<bool>(authProvider.select((s) => s.signedIn), (prev, next) {
      if (next) {
        _listen();
      } else {
        unawaited(_sub?.cancel());
        _sub = null;
        state = emptySessionSnapshot();
      }
    });
    if (ref.read(authProvider).signedIn) {
      _listen();
    }
    ref.onDispose(() {
      unawaited(_sub?.cancel());
      _sub = null;
    });
    return emptySessionSnapshot();
  }

  void _listen() {
    unawaited(_sub?.cancel());
    _sub = ref.read(clientPortProvider).watchSessionSnapshot().listen((snap) {
      if (!ref.mounted) {
        return;
      }
      state = snap;
      unawaited(_applyIdentity(snap.lastError));
    });
  }

  Future<void> _applyIdentity(String? lastError) async {
    if (!ref.read(authProvider).signedIn) {
      return;
    }
    switch (classifySessionFault(lastError)) {
      case SessionFault.identityExpired:
        await ref.read(authProvider.notifier).signOut(expired: true);
      case SessionFault.kicked:
        await ref.read(authProvider.notifier).signOut(notice: Copy.kicked);
      case null:
        break;
    }
  }
}

final kimSessionProvider =
    NotifierProvider<KimSessionNotifier, SessionSnapshot>(
      KimSessionNotifier.new,
    );
