library;

import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../core/haptics.dart';
import '../models/models.dart';
import 'auth.dart';
import 'contacts.dart';
import 'gateway.dart';
import 'inbox.dart';
import 'providers.dart';

/// Subscribes to WGateway pushes while the gateway is online.
///
/// Watched from [KimApp] (above the shell), never from an IndexedStack tab:
/// Riverpod 3 pauses off-screen [TickerMode] listeners.
final liveEventsProvider = NotifierProvider<LiveEventsNotifier, int>(
  LiveEventsNotifier.new,
);

class LiveEventsNotifier extends Notifier<int> {
  @override
  int build() {
    final online = ref.watch(
      gatewayProvider.select((g) => g.value == ConnStatus.online),
    );
    if (!online) {
      return 0;
    }
    final client = ref.read(clientPortProvider);
    final inbox = ref.read(inboxProvider.notifier);
    final contacts = ref.read(contactsProvider.notifier);
    final auth = ref.read(authProvider.notifier);
    final sub = client.events().listen(
      (event) {
        switch (event.kind) {
          case KimEventKind.talk:
            inbox.receive(event);
            if (event.messageId != 0) {
              unawaited(client.ack(event.messageId));
            }
          case KimEventKind.kick:
            unawaited(auth.signOut());
          case KimEventKind.friend:
            contacts.onRequest(event.sender, event.extra);
            unawaited(KimHaptics.light());
          case KimEventKind.group:
            if (event.dest.isNotEmpty) {
              inbox.ensureThread(
                id: event.dest,
                kind: ThreadKind.group,
                title: event.dest,
              );
            }
          case KimEventKind.token:
            unawaited(auth.savePushedToken(event.token));
          case KimEventKind.closed:
            unawaited(ref.read(gatewayProvider.notifier).drop());
        }
      },
      onError: (_) => unawaited(ref.read(gatewayProvider.notifier).drop()),
      onDone: () => unawaited(ref.read(gatewayProvider.notifier).drop()),
    );
    ref.onDispose(sub.cancel);
    return 0;
  }
}

const _pingEvery = Duration(seconds: 8);
const _pingWait = Duration(seconds: 3);

/// Radio is already a [gatewayProvider] dependency. This probe catches stale
/// "wifi" while the socket is dead (common on iOS simulator).
final sessionLinkProvider = NotifierProvider<SessionLinkNotifier, int>(
  SessionLinkNotifier.new,
);

class SessionLinkNotifier extends Notifier<int> {
  @override
  int build() {
    final online = ref.watch(
      gatewayProvider.select((g) => g.value == ConnStatus.online),
    );
    if (!online) {
      return 0;
    }
    final client = ref.read(clientPortProvider);
    Future<void> probe() async {
      try {
        await client.ping().timeout(_pingWait);
      } catch (_) {
        if (!ref.mounted) {
          return;
        }
        await ref.read(gatewayProvider.notifier).drop();
        if (ref.mounted) {
          ref.invalidate(gatewayProvider);
        }
      }
    }

    unawaited(probe());
    final timer = Timer.periodic(_pingEvery, (_) => unawaited(probe()));
    ref.onDispose(timer.cancel);
    return 0;
  }
}
