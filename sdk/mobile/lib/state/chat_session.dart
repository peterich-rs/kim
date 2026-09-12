library;

import 'dart:async';

import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:kim_media_picker/kim_media_picker.dart';

import '../agent/mention.dart';
import '../copy.dart';
import '../kim_bridge.dart';
import '../models/models.dart';
import 'agent_profiles.dart';
import 'auth.dart';
import 'chat_agent.dart';
import 'inbox.dart';
import 'messages.dart';
import 'mutations.dart';
import 'outbox.dart';
import 'presence.dart';
import 'providers.dart';
import 'session.dart';

class ChatSessionState {
  const ChatSessionState({
    this.toast,
    this.toastError = false,
    this.redirectDest,
  });

  final String? toast;
  final bool toastError;
  final String? redirectDest;
}

class ChatSessionNotifier extends Notifier<ChatSessionState> {
  ChatSessionNotifier(this.dest);

  final String dest;

  Timer? _typingIdle;
  Timer? _typingSendGate;
  var _typingActive = false;
  KimClientPort? _client;
  VoidCallback? _leaveRoom;

  ThreadKind get kind =>
      ref.read(threadsProvider).thread(dest)?.kind ?? ThreadKind.user;

  bool get isAgent => isAgentDest(dest);

  bool get isUserThread => kind == ThreadKind.user && !isAgent;

  @override
  ChatSessionState build() {
    ref.onDispose(_disposeSession);
    Future<void>.microtask(_start);
    return const ChatSessionState();
  }

  Future<void> _start() async {
    _client = ref.read(clientPortProvider);
    final unread = ref.read(threadsProvider).thread(dest)?.unread ?? 0;
    final messages = ref.read(threadMessagesProvider(dest).notifier);
    messages.captureUnreadAnchor(
      unread: unread,
      self: ref.read(sessionProvider).account,
    );
    unawaited(messages.reconcile());
    unawaited(messages.markRead());
    await _maybeRegisterLocalAgent();
    if (!ref.mounted) {
      return;
    }
    if (state.redirectDest != null) {
      return;
    }
    await _enterRoom();
  }

  Future<void> _maybeRegisterLocalAgent() async {
    if (!isAgentDest(dest)) {
      return;
    }
    final store = ref.read(agentProfilesProvider.notifier);
    if (!store.serverIdentity || !ref.read(authProvider).signedIn) {
      return;
    }
    await store.ensureLoaded();
    if (!ref.mounted) {
      return;
    }
    AgentProfile? profile;
    final canon = canonicalAgentDest(dest);
    for (final p in ref.read(agentProfilesProvider)) {
      if (canonicalAgentDest(p.dest) == canon) {
        profile = p;
        break;
      }
    }
    profile ??= store.goose;
    if (profile == null) {
      return;
    }
    if (profile.serverAccount.isEmpty) {
      profile = await store.ensureBotIdentity(profile);
    }
    if (!ref.mounted) {
      return;
    }
    if (profile.serverAccount.isNotEmpty && profile.serverAccount != dest) {
      state = ChatSessionState(redirectDest: profile.serverAccount);
    }
  }

  Future<void> _enterRoom() async {
    if (!isUserThread) {
      return;
    }
    final KimClientPort client = ref.read(clientPortProvider);
    _client = client;
    final id = dest;
    try {
      final rows = await client.roomEnter(id, kind: 0);
      void leave() {
        unawaited(() async {
          try {
            await client.roomLeave(id, kind: 0);
          } catch (_) {}
        }());
      }

      if (!ref.mounted) {
        leave();
        return;
      }
      ref.read(presenceProvider.notifier).applySnapshot(rows);
      _leaveRoom = leave;
    } catch (_) {}
  }

  void _disposeSession() {
    _typingIdle?.cancel();
    _typingSendGate?.cancel();
    final client = _client;
    if (_typingActive && isUserThread && client != null) {
      _typingActive = false;
      unawaited(() async {
        try {
          await client.sendTyping(dest, kind: 0, active: false);
        } catch (_) {}
      }());
    }
    final leave = _leaveRoom;
    _leaveRoom = null;
    leave?.call();
  }

  void onComposerTyping(String text) {
    if (!isUserThread) {
      return;
    }
    final has = text.trim().isNotEmpty;
    _typingIdle?.cancel();
    if (!has) {
      stopTyping();
      return;
    }
    if (!_typingActive) {
      _typingActive = true;
      unawaited(_emitTyping(true));
    } else if (_typingSendGate?.isActive != true) {
      _typingSendGate = Timer(const Duration(milliseconds: 1200), () {
        if (_typingActive) {
          unawaited(_emitTyping(true));
        }
      });
    }
    _typingIdle = Timer(const Duration(milliseconds: 2500), stopTyping);
  }

  void stopTyping() {
    _typingIdle?.cancel();
    _typingIdle = null;
    _typingSendGate?.cancel();
    _typingSendGate = null;
    if (!_typingActive) {
      return;
    }
    _typingActive = false;
    unawaited(_emitTyping(false));
  }

  Future<void> _emitTyping(bool active) async {
    final client = _client;
    if (client == null) {
      return;
    }
    try {
      await client.sendTyping(dest, kind: 0, active: active);
    } catch (_) {}
  }

  Future<bool> sendText(String text) async {
    stopTyping();
    try {
      if (isAgent) {
        await ref.read(chatAgentProvider).sendDirect(dest: dest, text: text);
      } else {
        await sendMessageMutation(dest).run(ref, (tsx) {
          return tsx.get(outboxProvider.notifier).sendText(dest, text);
        });
        unawaited(
          ref.read(chatAgentProvider).onOutgoingText(dest: dest, text: text),
        );
      }
      return true;
    } on StateError catch (err) {
      _toast(err.message, error: true);
      return false;
    } catch (_) {
      return false;
    }
  }

  Future<void> pickAlbum() async {
    try {
      final assets = await KimMediaPicker.instance.pickMultiple();
      if (assets.isEmpty) {
        return;
      }
      await sendImages(assets);
    } on MissingPluginException {
      return;
    } on KimMediaPickerException catch (err) {
      _toastMedia(err);
    }
  }

  Future<void> takePhoto() async {
    try {
      final shot = await KimMediaPicker.instance.capture();
      if (shot == null) {
        return;
      }
      await sendImages([shot]);
    } on MissingPluginException {
      return;
    } on KimMediaPickerException catch (err) {
      _toastMedia(err);
    }
  }

  Future<void> sendImages(List<KimMediaAsset> assets) async {
    try {
      await sendImagesMutation(dest).run(ref, (tsx) {
        return tsx.get(outboxProvider.notifier).sendImages(dest, assets);
      });
    } on StateError catch (err) {
      _toast(err.message, error: true);
    } catch (_) {
      _toast(Copy.sendFailed, error: true);
    }
  }

  Future<void> retry(String key) async {
    try {
      await ref.read(outboxProvider.notifier).retry(dest, key);
    } on StateError catch (err) {
      _toast(err.message, error: true);
    } catch (_) {}
  }

  void consumeToast() {
    if (state.toast != null) {
      state = ChatSessionState(redirectDest: state.redirectDest);
    }
  }

  void _toast(String message, {bool error = false}) {
    state = ChatSessionState(toast: message, toastError: error);
  }

  void _toastMedia(KimMediaPickerException err) {
    _toast(
      err.code == 'permission_denied' ? Copy.mediaPermission : Copy.mediaFailed,
      error: true,
    );
  }
}

final chatSessionProvider =
    NotifierProvider.family<ChatSessionNotifier, ChatSessionState, String>(
      ChatSessionNotifier.new,
    );
