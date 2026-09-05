library;

import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../copy.dart';
import '../core/connectivity.dart';
import '../core/haptics.dart';
import '../core/permissions.dart';
import '../core/user_agent.dart';
import '../models/models.dart';
import 'auth.dart';
import 'contacts.dart';
import 'inbox.dart';
import 'location.dart';
import 'messages.dart';
import 'providers.dart';

/// Mirrors [SessionSupervisor] link state. Replaces gateway + 8s ping probe.
final linkProvider = NotifierProvider<LinkNotifier, KimLinkState>(
  LinkNotifier.new,
);

class LinkNotifier extends Notifier<KimLinkState> {
  var _sessionGen = 0;
  var _startedFor = '';
  var _syncing = false;
  var _askedNotes = false;
  var _radioWasUp = false;
  StreamSubscription<KimEvent>? _events;
  var _disposeBound = false;
  KimLinkState _snapshot = const KimLinkState();

  @override
  KimLinkState build() {
    final signedIn = ref.watch(authProvider.select((s) => s.signedIn));
    final account = ref.watch(authProvider.select((s) => s.account));
    final radio = ref.watch(radioOnlineProvider);
    if (!signedIn) {
      _startedFor = '';
      _radioWasUp = false;
      _snapshot = const KimLinkState();
      unawaited(_stop());
      return _snapshot;
    }
    if (_startedFor != account) {
      _startedFor = account;
      _snapshot = const KimLinkState(status: ConnStatus.connecting);
      unawaited(_start());
    } else if (radio && !_radioWasUp) {
      unawaited(_radioUp());
    }
    _radioWasUp = radio;
    if (!radio) {
      return KimLinkState(
        status: ConnStatus.offline,
        attempt: _snapshot.attempt,
        error: _snapshot.error,
      );
    }
    return _snapshot;
  }

  Future<void> retry() async {
    if (_snapshot.status == ConnStatus.offline ||
        _snapshot.status == ConnStatus.connecting) {
      await _start();
      return;
    }
    try {
      await ref.read(clientPortProvider).notifyRadioUp();
    } catch (_) {
      await _start();
    }
  }

  Future<void> _radioUp() async {
    if (_events == null) {
      await _start();
      return;
    }
    try {
      await ref.read(clientPortProvider).notifyRadioUp();
    } catch (_) {
      await _start();
    }
  }

  Future<void> _stop() async {
    _sessionGen += 1;
    _syncing = false;
    await _events?.cancel();
    _events = null;
    try {
      await ref.read(clientPortProvider).stopSession();
    } catch (_) {}
  }

  Future<void> _start() async {
    final gen = ++_sessionGen;
    final runtime = ref.read(runtimeProvider);
    final token = runtime.settings.token;
    if (token.isEmpty) {
      return;
    }
    if (loopbackUnreachableOnThisDevice(runtime.settings.url)) {
      _set(
        const KimLinkState(
          status: ConnStatus.reconnecting,
          error: Copy.loopbackUnreachable,
        ),
      );
    }
    try {
      await ref
          .read(clientPortProvider)
          .startSession(
            runtime.settings.url,
            token,
            userAgent: kimUserAgent(runtime),
          );
    } catch (err) {
      if (!ref.mounted || gen != _sessionGen) {
        return;
      }
      _set(KimLinkState(status: ConnStatus.offline, error: err.toString()));
      return;
    }
    if (!ref.mounted || gen != _sessionGen) {
      return;
    }
    _set(ref.read(clientPortProvider).linkState());
    _listen(gen);
  }

  void _set(KimLinkState next) {
    final error = next.status == ConnStatus.online
        ? null
        : (next.error ?? _snapshot.error);
    next = KimLinkState(
      status: next.status,
      attempt: next.attempt,
      error: error,
    );
    _snapshot = next;
    if (!ref.read(radioOnlineProvider) && next.status != ConnStatus.offline) {
      state = KimLinkState(
        status: ConnStatus.offline,
        attempt: next.attempt,
        error: next.error,
      );
      return;
    }
    state = next;
  }

  void _listen(int gen) {
    unawaited(_events?.cancel());
    final client = ref.read(clientPortProvider);
    _events = client.sessionEvents().listen(
      (event) => unawaited(_onEvent(event, gen)),
      onError: (_) {
        if (ref.mounted && gen == _sessionGen) {
          _set(const KimLinkState(status: ConnStatus.reconnecting));
        }
      },
      onDone: () {
        if (ref.mounted && gen == _sessionGen) {
          _events = null;
        }
      },
    );
    if (!_disposeBound) {
      _disposeBound = true;
      ref.onDispose(() {
        unawaited(_events?.cancel());
        _events = null;
      });
    }
  }

  Future<void> _onEvent(KimEvent event, int gen) async {
    if (!ref.mounted || gen != _sessionGen) {
      return;
    }
    switch (event.kind) {
      case KimEventKind.link:
        final status = KimLinkState.statusFromLabel(event.state);
        _set(
          KimLinkState(
            status: status,
            attempt: event.attempt,
            error: status == ConnStatus.online
                ? null
                : (event.error.isNotEmpty ? event.error : _snapshot.error),
          ),
        );
        if (_snapshot.status == ConnStatus.online) {
          _askNotifications();
        }
      case KimEventKind.inbox:
        ref.read(threadsProvider.notifier).mergeInbox(event.inbox);
      case KimEventKind.talk:
        await _onTalk(event, ack: !_syncing);
      case KimEventKind.syncPage:
        await _onSyncPage(event);
      case KimEventKind.syncProgress:
        _syncing = event.pagePending;
      case KimEventKind.syncDone:
        _syncing = false;
      case KimEventKind.syncFailed:
        _set(
          KimLinkState(
            status: _snapshot.status,
            attempt: _snapshot.attempt,
            error: event.error,
          ),
        );
      case KimEventKind.kick:
        unawaited(ref.read(authProvider.notifier).signOut());
      case KimEventKind.authExpired:
        unawaited(ref.read(authProvider.notifier).signOut(expired: true));
      case KimEventKind.friend:
        unawaited(KimHaptics.light());
        unawaited(_friendPush(event, accepted: false));
      case KimEventKind.friendAccepted:
        unawaited(KimHaptics.success());
        unawaited(_friendPush(event, accepted: true));
      case KimEventKind.group:
        if (event.dest.isNotEmpty) {
          ref
              .read(threadsProvider.notifier)
              .ensureThread(
                id: event.dest,
                kind: ThreadKind.group,
                title: event.dest,
              );
        }
      case KimEventKind.token:
        unawaited(ref.read(authProvider.notifier).savePushedToken(event.token));
      case KimEventKind.closed:
        if (ref.mounted && gen == _sessionGen) {
          _set(const KimLinkState(status: ConnStatus.reconnecting));
        }
    }
  }

  Future<void> _friendPush(KimEvent event, {required bool accepted}) async {
    await Future<void>.microtask(() {});
    if (!ref.mounted) {
      return;
    }
    final contacts = ref.read(contactsProvider.notifier);
    final name = event.nickname.isEmpty ? event.extra : event.nickname;
    if (accepted) {
      contacts.onAccepted(event.sender, name);
    } else {
      contacts.onRequest(event.sender, name);
    }
  }

  Future<void> _onSyncPage(KimEvent event) async {
    _syncing = true;
    final account = ref.read(authProvider).account;
    final viewing = chatIdFromPath(ref.read(locationProvider));
    final repo = ref.read(messageRepositoryProvider);
    final msgs = [
      for (final talk in event.talks)
        repo.fromTalk(
          dest: talk.dest.isNotEmpty ? talk.dest : talk.sender,
          sender: talk.sender,
          body: talk.body,
          extra: talk.extra,
          messageId: talk.messageId,
          sendTime: talk.sendTime,
          msgType: talk.msgType,
        ),
    ].where((m) => m.dest.isNotEmpty && m.body.isNotEmpty).toList();
    if (msgs.isNotEmpty) {
      final results = await repo.applySync(
        account,
        msgs,
        viewingDest: viewing,
      );
      if (!ref.mounted) {
        return;
      }
      ref.read(threadsProvider.notifier).ingestAll(results);
      final byDest = <String, List<KimChatMsg>>{};
      for (final r in results) {
        byDest.putIfAbsent(r.message.dest, () => []).add(r.message);
      }
      for (final entry in byDest.entries) {
        ref
            .read(threadMessagesProvider(entry.key).notifier)
            .receiveAll(entry.value);
        if (viewing == entry.key) {
          unawaited(
            ref.read(threadMessagesProvider(entry.key).notifier).markRead(),
          );
        }
      }
    }
    if (!ref.mounted) {
      return;
    }
    if (event.pageId != 0) {
      try {
        await ref.read(clientPortProvider).syncConfirm(event.pageId);
      } catch (_) {}
    }
  }

  Future<void> _onTalk(KimEvent event, {required bool ack}) async {
    final dest = event.dest.isNotEmpty ? event.dest : event.sender;
    if (dest.isEmpty || event.body.isEmpty) {
      return;
    }
    final account = ref.read(authProvider).account;
    final viewing = chatIdFromPath(ref.read(locationProvider));
    final repo = ref.read(messageRepositoryProvider);
    final msg = repo.fromTalk(
      dest: dest,
      sender: event.sender,
      body: event.body,
      extra: event.extra,
      messageId: event.messageId,
      sendTime: event.sendTime,
      msgType: event.msgType,
    );
    final results = await repo.applyLive(
      account,
      [msg],
      viewingDest: viewing,
    );
    if (!ref.mounted) {
      return;
    }
    ref.read(threadsProvider.notifier).ingestAll(results);
    ref.read(threadMessagesProvider(dest).notifier).receiveAll([
      for (final r in results) r.message,
    ]);
    if (viewing == dest) {
      unawaited(ref.read(threadMessagesProvider(dest).notifier).markRead());
    }
    if (!ack || event.messageId == 0) {
      return;
    }
    try {
      await ref.read(clientPortProvider).ack(event.messageId);
    } catch (_) {}
  }

  void _askNotifications() {
    if (_askedNotes) {
      return;
    }
    _askedNotes = true;
    unawaited(
      KimPermissions.requestNotificationsOnce(
        ref.read(runtimeProvider).settings,
      ),
    );
  }
}
