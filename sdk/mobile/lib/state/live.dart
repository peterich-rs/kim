library;

import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../copy.dart';
import '../core/haptics.dart';
import '../models/models.dart';
import 'contacts.dart';
import 'inbox.dart';
import 'providers.dart';
import 'session.dart';

/// Subscribes to WGateway pushes while the session is online.
///
/// This is a provider, not a widget `initState`/`dispose`. Riverpod owns the
/// [StreamSubscription] and cancels it in [Ref.onDispose].
final liveEventsProvider = Provider<void>((ref) {
  final status = ref.watch(sessionProvider.select((s) => s.status));
  if (status != ConnStatus.online) {
    return;
  }
  final client = ref.read(clientPortProvider);
  final inbox = ref.read(inboxProvider.notifier);
  final contacts = ref.read(contactsProvider.notifier);
  final session = ref.read(sessionProvider.notifier);
  final sub = client.events().listen(
    (event) {
      switch (event.kind) {
        case KimEventKind.talk:
          inbox.receive(event);
          if (event.messageId != 0) {
            unawaited(client.ack(event.messageId));
          }
        case KimEventKind.kick:
          unawaited(session.signOut());
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
          if (event.token.isNotEmpty) {
            unawaited(
              ref.read(runtimeProvider).settings.saveToken(event.token),
            );
          }
        case KimEventKind.closed:
          session.markOffline();
      }
    },
    onError: (_) => session.markOffline(),
    onDone: session.markOffline,
  );
  ref.onDispose(sub.cancel);
});

const _pingEvery = Duration(seconds: 8);
const _pingWait = Duration(seconds: 3);

/// Radio down → drop the WS. Radio up while signed-out-of-socket → reconnect.
/// Periodic ping so airplane mode still shows offline when connectivity_plus
/// stays on `wifi` / `other` (common on iOS simulator).
final sessionLinkProvider = Provider<void>((ref) {
  final radio = ref.watch(runtimeProvider).connectivity.online;

  void onRadio() {
    final session = ref.read(sessionProvider);
    final n = ref.read(sessionProvider.notifier);
    if (!radio.value) {
      n.dropLink(error: Copy.offline);
      return;
    }
    if (session.signedIn && session.status == ConnStatus.offline) {
      unawaited(n.connect());
    }
  }

  radio.addListener(onRadio);
  ref.onDispose(() => radio.removeListener(onRadio));

  final status = ref.watch(sessionProvider.select((s) => s.status));
  if (status != ConnStatus.online) {
    return;
  }

  final client = ref.read(clientPortProvider);
  Future<void> probe() async {
    try {
      await client.ping().timeout(_pingWait);
    } catch (_) {
      ref.read(sessionProvider.notifier).dropLink();
    }
  }

  unawaited(probe());
  final timer = Timer.periodic(_pingEvery, (_) => unawaited(probe()));
  ref.onDispose(timer.cancel);
});
