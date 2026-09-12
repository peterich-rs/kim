library;

import 'dart:async';

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../agent/host_support.dart';
import '../copy.dart';
import '../core/connectivity.dart';
import '../core/haptics.dart';
import '../core/image_extra.dart';
import '../core/permissions.dart';
import '../core/user_agent.dart';
import '../models/models.dart';
import 'auth.dart';
import 'chat_agent.dart';
import 'contacts.dart';
import 'presence.dart';
import 'receipts.dart';
import 'typing.dart';
import 'inbox.dart';
import 'location.dart';
import 'messages.dart';
import 'providers.dart';

/// Mirrors [SessionSupervisor] link state. Replaces gateway + 8s ping probe.
final linkProvider = NotifierProvider<LinkNotifier, KimLinkState>(
  LinkNotifier.new,
);

class LinkNotifier extends Notifier<KimLinkState> with WidgetsBindingObserver {
  var _sessionGen = 0;
  var _startedFor = '';
  var _syncing = false;
  var _askedNotes = false;
  var _radioWasUp = false;
  StreamSubscription<KimEvent>? _events;
  Future<void> _eventChain = Future<void>.value();
  var _disposeBound = false;
  var _lifecycleBound = false;
  KimLinkState _snapshot = const KimLinkState();

  @override
  KimLinkState build() {
    if (!_lifecycleBound) {
      _lifecycleBound = true;
      WidgetsBinding.instance.addObserver(this);
      ref.onDispose(() {
        WidgetsBinding.instance.removeObserver(this);
        _lifecycleBound = false;
      });
    }
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
    return _snapshot;
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    if (state == AppLifecycleState.resumed) {
      unawaited(_foreground());
    }
  }

  Future<void> retry() async {
    if (_events != null && _startedFor.isNotEmpty) {
      try {
        await ref.read(clientPortProvider).notifyRadioUp();
        return;
      } catch (_) {}
    }
    await _start();
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

  Future<void> _foreground() async {
    if (_events == null) {
      return;
    }
    try {
      await ref.read(clientPortProvider).notifyForeground();
    } catch (_) {}
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
        KimLinkState(
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
    state = next;
  }

  void _listen(int gen) {
    unawaited(_events?.cancel());
    _eventChain = Future<void>.value();
    final client = ref.read(clientPortProvider);
    _events = client.sessionEvents().listen(
      (event) {
        _eventChain = _eventChain.then((_) async {
          try {
            await _onEvent(event, gen);
          } catch (_) {}
        });
      },
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
          if (agentHostSupported) {
            unawaited(ref.read(chatAgentProvider).catchUpPending());
          }
        }
      case KimEventKind.inbox:
        ref.read(threadsProvider.notifier).mergeInbox(event.inbox);
      case KimEventKind.talk:
        await _onTalk(
          event,
          gen,
          ack: !_syncing && !ref.read(runtimeProvider).rustStore,
        );
      case KimEventKind.syncPage:
        await _onSyncPage(event, gen);
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
        unawaited(ref.read(authProvider.notifier).signOut(notice: Copy.kicked));
      case KimEventKind.authExpired:
        unawaited(ref.read(authProvider.notifier).signOut(expired: true));
      case KimEventKind.friend:
        unawaited(KimHaptics.light());
        unawaited(_friendPush(event, accepted: false));
      case KimEventKind.friendAccepted:
        unawaited(KimHaptics.success());
        unawaited(_friendPush(event, accepted: true));
      case KimEventKind.profileUpdated:
        _onProfileUpdated(event);
      case KimEventKind.presence:
        _onPresence(event);
      case KimEventKind.typing:
        _onTyping(event);
      case KimEventKind.receiptRead:
        _onReceiptRead(event);
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
        return;
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

  void _onProfileUpdated(KimEvent event) {
    final account = event.sender.isNotEmpty ? event.sender : event.dest;
    if (account.isEmpty) {
      return;
    }
    final nickname = event.nickname.isEmpty ? account : event.nickname;
    final avatar = event.extra;
    ref
        .read(contactsProvider.notifier)
        .onProfileUpdated(account, nickname, avatar);
    ref
        .read(threadsProvider.notifier)
        .patchPeerProfile(account, title: nickname, avatar: avatar);
  }

  void _onPresence(KimEvent event) {
    final account = event.sender.isNotEmpty ? event.sender : event.dest;
    if (account.isEmpty) {
      return;
    }
    ref
        .read(presenceProvider.notifier)
        .applyPush(
          account: account,
          status: event.msgType,
          lastSeen: event.sendTime,
        );
  }

  Future<void> _onSyncPage(KimEvent event, int gen) async {
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
      if (ref.read(runtimeProvider).rustStore) {
        if (!ref.mounted || gen != _sessionGen) {
          return;
        }
        final byDest = <String, List<KimChatMsg>>{};
        for (final m in msgs) {
          byDest.putIfAbsent(m.dest, () => []).add(m);
        }
        for (final entry in byDest.entries) {
          ref
              .read(threadMessagesProvider(entry.key).notifier)
              .receiveAll(entry.value);
        }
        return;
      }
      final results = await repo.applySync(account, msgs, viewingDest: viewing);
      if (!ref.mounted || gen != _sessionGen) {
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
    if (!ref.mounted || gen != _sessionGen) {
      return;
    }
    if (ref.read(runtimeProvider).rustStore) {
      return;
    }
    if (event.pageId != 0) {
      try {
        await ref.read(clientPortProvider).syncConfirm(event.pageId);
      } catch (_) {}
    }
  }

  Future<void> _onTalk(KimEvent event, int gen, {required bool ack}) async {
    final dest = event.dest.isNotEmpty ? event.dest : event.sender;
    if (dest.isEmpty || event.body.isEmpty) {
      return;
    }
    if (event.sender.isNotEmpty) {
      ref.read(typingProvider.notifier).clearDest(event.sender);
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
    if (ref.read(runtimeProvider).rustStore) {
      if (!ref.mounted || gen != _sessionGen) {
        return;
      }
      ref.read(threadMessagesProvider(dest).notifier).receiveAll([msg]);
      if (event.sender == account &&
          event.messageId != 0 &&
          kindFromWire(
                body: event.body,
                extra: event.extra,
                type: event.msgType,
              ) ==
              KimMsgKind.text) {
        unawaited(
          ref
              .read(chatAgentProvider)
              .onIncomingEcho(
                dest: dest,
                sender: event.sender,
                text: event.body,
                messageId: event.messageId,
              ),
        );
      }
      return;
    }
    final results = await repo.applyLive(account, [msg], viewingDest: viewing);
    if (!ref.mounted || gen != _sessionGen) {
      return;
    }
    ref.read(threadsProvider.notifier).ingestAll(results);
    ref.read(threadMessagesProvider(dest).notifier).receiveAll([
      for (final r in results) r.message,
    ]);
    if (viewing == dest) {
      unawaited(ref.read(threadMessagesProvider(dest).notifier).markRead());
    }
    if (event.sender == account &&
        event.messageId != 0 &&
        kindFromWire(
              body: event.body,
              extra: event.extra,
              type: event.msgType,
            ) ==
            KimMsgKind.text) {
      unawaited(
        ref
            .read(chatAgentProvider)
            .onIncomingEcho(
              dest: dest,
              sender: event.sender,
              text: event.body,
              messageId: event.messageId,
            ),
      );
    }
    if (!ack || event.messageId == 0) {
      return;
    }
    try {
      await ref.read(clientPortProvider).ack(event.messageId);
    } catch (_) {}
  }

  void _onTyping(KimEvent event) {
    final typer = event.sender;
    if (typer.isEmpty) {
      return;
    }
    ref
        .read(typingProvider.notifier)
        .applyPush(typer: typer, dest: event.dest, active: event.pagePending);
  }

  void _onReceiptRead(KimEvent event) {
    if (event.msgType != 0) {
      return;
    }
    ref
        .read(receiptsProvider.notifier)
        .applyPush(
          reader: event.sender,
          dest: event.dest,
          kind: event.msgType,
          messageId: event.messageId,
        );
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
