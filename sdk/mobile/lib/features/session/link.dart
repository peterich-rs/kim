library;

import 'dart:async';

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:kim_mobile/copy.dart';
import 'package:kim_mobile/core/connectivity.dart';
import 'package:kim_mobile/core/haptics.dart';
import 'package:kim_mobile/core/logger.dart';
import 'package:kim_mobile/core/permissions.dart';
import 'package:kim_mobile/core/user_agent.dart';
import 'package:kim_mobile/models/models.dart';
import 'package:kim_mobile/src/rust/api/types.dart';
import 'package:kim_mobile/features/auth/auth.dart';
import 'package:kim_mobile/features/session/kim_session.dart';
import 'package:kim_mobile/features/session/panic.dart';
import 'package:kim_mobile/features/session/presence.dart';
import 'package:kim_mobile/features/session/providers.dart';
import 'package:kim_mobile/features/session/receipts.dart';
import 'package:kim_mobile/features/session/typing.dart';
import 'package:kim_mobile/features/chats/conversation_visibility.dart';

final linkProvider = NotifierProvider<LinkNotifier, KimLinkState>(
  LinkNotifier.new,
);

class LinkNotifier extends Notifier<KimLinkState> with WidgetsBindingObserver {
  var _sessionGen = 0;
  var _startedFor = '';
  var _askedNotes = false;
  var _radioWasUp = false;
  var _specSynced = false;
  var _specSyncing = false;
  var _lifecycleBound = false;
  StreamSubscription<SessionUpdateDto>? _events;

  @override
  KimLinkState build() {
    if (!_lifecycleBound) {
      _lifecycleBound = true;
      WidgetsBinding.instance.addObserver(this);
      ref.onDispose(() {
        WidgetsBinding.instance.removeObserver(this);
        _lifecycleBound = false;
        unawaited(_events?.cancel());
        _events = null;
      });
    }
    final signedIn = ref.watch(authProvider.select((s) => s.signedIn));
    final account = ref.watch(authProvider.select((s) => s.account));
    final radio = ref.watch(radioOnlineProvider);
    final snap = ref.watch(kimSessionProvider);
    if (!signedIn) {
      _startedFor = '';
      _radioWasUp = false;
      _specSynced = false;
      unawaited(_stop());
      return const KimLinkState();
    }
    if (_startedFor != account) {
      _startedFor = account;
      _specSynced = false;
      unawaited(_start());
    } else if (radio && !_radioWasUp) {
      unawaited(_radioUp());
    }
    _radioWasUp = radio;
    _listenEvents();
    ref.watch(conversationVisibilityProvider);
    final mapped = kimLinkFromDto(snap.link, snap.lastError);
    if (mapped.status == ConnStatus.online) {
      _askNotifications();
      if (!_specSynced && !_specSyncing) {
        unawaited(_syncAgentSpecs());
      }
    } else {
      _specSynced = false;
    }
    return mapped;
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    if (state == AppLifecycleState.resumed) {
      unawaited(_foreground());
    }
  }

  Future<void> retry() async {
    if (_startedFor.isNotEmpty) {
      try {
        await ref.read(clientPortProvider).notifyRadioUp();
        return;
      } catch (e, st) {
        KimLogger.warn('retry notifyRadioUp', e, st);
      }
    }
    await _start();
  }

  Future<void> _syncAgentSpecs() async {
    if (_specSyncing || _specSynced) {
      return;
    }
    _specSyncing = true;
    final gen = _sessionGen;
    final account = _startedFor;
    try {
      await ref.read(clientPortProvider).syncAgentSpecs();
      if (ref.mounted && gen == _sessionGen && account == _startedFor) {
        _specSynced = true;
      }
    } catch (e, st) {
      KimLogger.warn('agent spec sync', e, st);
    } finally {
      _specSyncing = false;
    }
  }

  Future<void> _radioUp() async {
    if (_startedFor.isEmpty) {
      await _start();
      return;
    }
    try {
      await ref.read(clientPortProvider).notifyRadioUp();
    } catch (e, st) {
      KimLogger.warn('radioUp', e, st);
      await _start();
    }
  }

  Future<void> _foreground() async {
    if (_startedFor.isEmpty) {
      return;
    }
    try {
      await ref.read(clientPortProvider).notifyForeground();
    } catch (e, st) {
      KimLogger.warn('foreground', e, st);
    }
  }

  Future<void> _stop() async {
    _sessionGen += 1;
    await _events?.cancel();
    _events = null;
    try {
      await ref.read(clientPortProvider).stopSession();
    } catch (e, st) {
      KimLogger.warn('stopSession', e, st);
    }
  }

  Future<void> _start() async {
    final gen = ++_sessionGen;
    final runtime = ref.read(runtimeProvider);
    final token = runtime.settings.token;
    if (token.isEmpty) {
      return;
    }
    if (loopbackUnreachableOnThisDevice(runtime.settings.url)) {
      KimLogger.warn('loopback unreachable');
    }
    try {
      await ref
          .read(clientPortProvider)
          .startSession(
            runtime.settings.url,
            token,
            userAgent: kimUserAgent(runtime),
          );
    } catch (err, st) {
      KimLogger.warn('startSession', err, st);
      return;
    }
    if (!ref.mounted || gen != _sessionGen) {
      return;
    }
    _listenEvents();
  }

  void _listenEvents() {
    if (_events != null) {
      return;
    }
    _events = ref.read(clientPortProvider).watchSessionEvents().listen((event) {
      if (!ref.mounted) {
        return;
      }
      switch (event) {
        case SessionUpdateDto_Kickout():
          unawaited(
            ref.read(authProvider.notifier).signOut(notice: Copy.kicked),
          );
        case SessionUpdateDto_AuthExpired():
          unawaited(ref.read(authProvider.notifier).signOut(expired: true));
        case SessionUpdateDto_TokenRenew(:final token):
          unawaited(ref.read(authProvider.notifier).savePushedToken(token));
        case SessionUpdateDto_FriendRequest():
          unawaited(KimHaptics.light());
        case SessionUpdateDto_FriendAccepted():
          unawaited(KimHaptics.success());
        case SessionUpdateDto_Presence(
          :final account,
          :final status,
          :final lastSeen,
        ):
          ref
              .read(presenceProvider.notifier)
              .applyPush(
                account: account,
                status: status,
                lastSeen: lastSeen.toInt(),
              );
        case SessionUpdateDto_Typing(:final typer, :final dest, :final active):
          ref
              .read(typingProvider.notifier)
              .applyPush(
                typer: typer,
                dest: dest,
                active: active,
                me: ref.read(authProvider).account,
              );
        case SessionUpdateDto_AgentTurn(:final dest, :final state):
          final busy = switch (state) {
            AgentTurnStateDto.queued ||
            AgentTurnStateDto.running ||
            AgentTurnStateDto.waitingPermission => true,
            AgentTurnStateDto.done || AgentTurnStateDto.error => false,
          };
          ref
              .read(typingProvider.notifier)
              .applyPush(
                typer: dest,
                dest: dest,
                active: busy,
                me: ref.read(authProvider).account,
              );
        case SessionUpdateDto_ReceiptRead(
          :final reader,
          :final dest,
          :final kind,
          :final messageId,
        ):
          ref
              .read(receiptsProvider.notifier)
              .applyPush(
                reader: reader,
                dest: dest,
                kind: kind,
                messageId: messageId.toInt(),
              );
        case SessionUpdateDto_RustPanic(:final message):
          ref.read(rustPanicProvider.notifier).setMessage(message);
        case SessionUpdateDto_Link():
        case SessionUpdateDto_Inbox():
        case SessionUpdateDto_ThreadUpsert():
        case SessionUpdateDto_ProfileUpdated():
        case SessionUpdateDto_ContactsChanged():
          break;
        default:
          break;
      }
    });
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
