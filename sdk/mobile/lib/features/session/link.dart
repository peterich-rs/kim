library;

import 'dart:async';

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:kim_mobile/copy.dart';
import 'package:kim_mobile/core/connectivity.dart';
import 'package:kim_mobile/core/haptics.dart';
import 'package:kim_mobile/core/failures.dart';
import 'package:kim_mobile/core/logger.dart';
import 'package:kim_mobile/core/permissions.dart';
import 'package:kim_mobile/core/user_agent.dart';
import 'package:kim_mobile/models/models.dart';
import 'package:kim_mobile/src/rust/api/types.dart';
import 'package:kim_mobile/features/auth/providers/auth.dart';
import 'package:kim_mobile/features/session/kim_session.dart';
import 'package:kim_mobile/features/session/panic.dart';
import 'package:kim_mobile/features/session/presence.dart';
import 'package:kim_mobile/features/session/providers.dart';
import 'package:kim_mobile/features/session/receipts.dart';
import 'package:kim_mobile/features/session/typing.dart';
import 'package:kim_mobile/features/chats/providers/conversation_visibility.dart';
import 'package:kim_mobile/features/agent/host_support.dart';
import 'package:kim_mobile/features/agent/mention.dart';

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
  StreamSubscription<SessionUpdate>? _events;

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
    final mapped = kimLinkFrom(snap.link, snap.lastError);
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
    KimLogger.info('link retry');
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
      KimLogger.info('agent spec sync');
      await ref.read(clientPortProvider).syncAgentSpecs();
      if (ref.mounted && gen == _sessionGen && account == _startedFor) {
        _specSynced = true;
        KimLogger.info('agent spec sync ok');
      }
    } catch (e, st) {
      final failure = apiFailureOf(e);
      if (failure != null) {
        KimLogger.warn('agent spec sync $failure', e, st);
        // Permanent protocol / validation failures must not spin with every
        // session snapshot rebuild (was flooding FRB + UI every 1–4s).
        if (ref.mounted &&
            gen == _sessionGen &&
            account == _startedFor &&
            !failure.retryableSend) {
          _specSynced = true;
        }
      } else {
        KimLogger.warn('agent spec sync', e, st);
      }
    } finally {
      _specSyncing = false;
    }
  }

  Future<void> _radioUp() async {
    if (_startedFor.isEmpty) {
      await _start();
      return;
    }
    KimLogger.info('radioUp');
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
    KimLogger.info('foreground');
    try {
      await ref.read(clientPortProvider).notifyForeground();
    } catch (e, st) {
      KimLogger.warn('foreground', e, st);
    }
  }

  Future<void> _stop() async {
    KimLogger.info('stopSession');
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
    KimLogger.info('startSession url=${runtime.settings.url}');
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
    KimLogger.info('startSession ok');
    if (!ref.mounted || gen != _sessionGen) {
      return;
    }
    ref.read(typingProvider.notifier).clear();
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
        case SessionUpdate_Kickout():
          KimLogger.warn('kickout');
          unawaited(
            ref.read(authProvider.notifier).signOut(notice: Copy.kicked),
          );
        case SessionUpdate_AuthExpired():
          KimLogger.warn('auth expired');
          unawaited(ref.read(authProvider.notifier).signOut(expired: true));
        case SessionUpdate_TokenRenew(:final token):
          unawaited(ref.read(authProvider.notifier).savePushedToken(token));
        case SessionUpdate_FriendRequest():
          unawaited(KimHaptics.light());
        case SessionUpdate_FriendAccepted():
          unawaited(KimHaptics.success());
        case SessionUpdate_Presence(
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
        case SessionUpdate_Typing(:final typer, :final dest, :final active):
          final agentWire =
              isAgentDest(typer) ||
              isServerBotAccount(typer) ||
              isAgentDest(dest) ||
              isServerBotAccount(dest);
          // Owner desktop: Goose AgentTurn is the only typing source.
          if (agentHostSupported && agentWire) {
            break;
          }
          ref
              .read(typingProvider.notifier)
              .applyPush(
                typer: typer,
                dest: dest,
                active: active,
                me: ref.read(authProvider).account,
              );
        case SessionUpdate_AgentTurn(:final dest, :final state):
          final busy = switch (state) {
            AgentTurnState.running => true,
            AgentTurnState.queued ||
            AgentTurnState.waitingPermission ||
            AgentTurnState.done ||
            AgentTurnState.error ||
            AgentTurnState.empty => false,
          };
          ref
              .read(typingProvider.notifier)
              .applyAgentTurn(
                dest: dest,
                busy: busy,
                me: ref.read(authProvider).account,
              );
        case SessionUpdate_ReceiptRead(
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
        case SessionUpdate_RustPanic(:final message):
          KimLogger.error('rust panic', message);
          ref.read(rustPanicProvider.notifier).setMessage(message);
        case SessionUpdate_Link(:final state, :final lastError):
          KimLogger.info(
            'link ${_linkLabel(state)}${lastError == null || lastError.isEmpty ? '' : ' error=$lastError'}',
          );
        case SessionUpdate_Inbox():
        case SessionUpdate_ThreadUpsert():
        case SessionUpdate_ProfileUpdated():
        case SessionUpdate_ContactsChanged():
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

String _linkLabel(LinkState state) {
  return switch (state) {
    LinkState_Connecting() => 'connecting',
    LinkState_Online() => 'online',
    LinkState_Reconnecting(:final attempt) => 'reconnecting attempt=$attempt',
    LinkState_Offline() => 'offline',
  };
}
