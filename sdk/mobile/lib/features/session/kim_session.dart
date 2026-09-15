library;

import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:kim_mobile/models/models.dart';
import 'package:kim_mobile/src/rust/api/types.dart';
import 'package:kim_mobile/features/auth/auth.dart';
import 'package:kim_mobile/features/session/providers.dart';

SessionSnapshotDto emptySessionSnapshot() => const SessionSnapshotDto(
  link: LinkStateDto.offline(),
  threads: [],
  unreadTotal: 0,
);

KimLinkState kimLinkFromDto(LinkStateDto link, String? lastError) {
  return switch (link) {
    LinkStateDto_Connecting() => KimLinkState(
      status: ConnStatus.connecting,
      error: lastError,
    ),
    LinkStateDto_Online() => const KimLinkState(status: ConnStatus.online),
    LinkStateDto_Reconnecting(:final attempt) => KimLinkState(
      status: ConnStatus.reconnecting,
      attempt: attempt,
      error: lastError,
    ),
    LinkStateDto_Offline() => KimLinkState(
      status: ConnStatus.offline,
      error: lastError,
    ),
  };
}

KimThread kimThreadFromDto(ThreadViewDto t) {
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

KimChatMsg kimChatFromDto(MessageViewDto m) {
  final status = switch (m.sendStatus) {
    SendStatusDto.failed || SendStatusDto.cancelled => KimSendStatus.failed,
    SendStatusDto.sent => KimSendStatus.sent,
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

class KimSessionNotifier extends Notifier<SessionSnapshotDto> {
  StreamSubscription<SessionSnapshotDto>? _sub;

  @override
  SessionSnapshotDto build() {
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
      if (ref.mounted) {
        state = snap;
      }
    });
  }
}

final kimSessionProvider =
    NotifierProvider<KimSessionNotifier, SessionSnapshotDto>(
      KimSessionNotifier.new,
    );
