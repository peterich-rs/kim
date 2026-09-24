library;

import 'dart:async';

import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:kim_media_picker/kim_media_picker.dart';
import 'package:uuid/uuid.dart';

import 'package:kim_mobile/features/agent/mention.dart';
import 'package:kim_mobile/copy.dart';
import 'package:kim_mobile/bridge/conversation_port.dart';
import 'package:kim_mobile/bridge/kim_bridge.dart';
import 'package:kim_mobile/models/models.dart';
import 'package:kim_mobile/features/agent/agent_profiles.dart';
import 'package:kim_mobile/features/auth/providers/auth.dart';

import 'package:kim_mobile/features/contacts/contacts.dart';
import 'package:kim_mobile/features/chats/providers/inbox.dart';
import 'package:kim_mobile/features/chats/providers/messages.dart';
import 'package:kim_mobile/features/session/mutations.dart';
import 'package:kim_mobile/features/session/presence.dart';
import 'package:kim_mobile/features/session/providers.dart';
import 'package:kim_mobile/core/logger.dart';
import 'package:kim_mobile/features/session/session.dart';

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
  ConversationPort? _room;
  VoidCallback? _leaveRoom;

  ThreadKind get kind =>
      ref.read(threadsProvider).thread(dest)?.kind ?? ThreadKind.user;

  bool get isAgent => isAgentDest(dest);

  bool get isUserThread =>
      kind == ThreadKind.user && !isAgent && !isServerBotAccount(dest);

  @override
  ChatSessionState build() {
    ref.onDispose(_disposeSession);
    ref.listen(agentProfilesProvider, (prev, next) {
      unawaited(_maybeRegisterLocalAgent());
    });
    Future<void>.microtask(_start);
    return const ChatSessionState();
  }

  Future<void> _start() async {
    _room = ref.read(clientPortProvider).conversation(dest);
    final unread = ref.read(threadsProvider).thread(dest)?.unread ?? 0;
    final messages = ref.read(threadMessagesProvider(dest).notifier);
    messages.captureUnreadAnchor(
      unread: unread,
      self: ref.read(sessionProvider).account,
    );
    unawaited(messages.markConversationRead());
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
    await store.ensureLoaded();
    if (!ref.mounted) {
      return;
    }
    if (!store.serverIdentity || !ref.read(authProvider).signedIn) {
      return;
    }
    var profile = profileForChatDest(dest, ref.read(agentProfilesProvider));
    if (profile == null) {
      return;
    }
    if (profile.serverAccount.isEmpty) {
      try {
        profile = await store.ensureBotIdentity(profile);
      } catch (err) {
        if (ref.mounted) {
          _toast(agentRegisterError(err), error: true);
        }
        return;
      }
    }
    if (!ref.mounted) {
      return;
    }
    if (profile.serverAccount.isNotEmpty && profile.serverAccount != dest) {
      state = ChatSessionState(redirectDest: profile.serverAccount);
      unawaited(ref.read(contactsProvider.notifier).refresh());
    }
  }

  Future<void> _enterRoom() async {
    if (!isUserThread) {
      return;
    }
    final KimClientPort client = ref.read(clientPortProvider);
    final id = dest;
    final port = client.conversation(id);
    _room = port;
    try {
      final rows = await port.enter();
      void leave() {
        unawaited(() async {
          try {
            await port.leave();
          } catch (_) {}
        }());
      }

      if (!ref.mounted) {
        leave();
        return;
      }
      ref.read(presenceProvider.notifier).applySnapshot(rows);
      _leaveRoom = leave;
    } catch (err, stack) {
      KimLogger.warn('room enter failed dest=$id', err, stack);
    }
  }

  void _disposeSession() {
    _typingIdle?.cancel();
    _typingSendGate?.cancel();
    final room = _room;
    if (_typingActive && isUserThread && room != null) {
      _typingActive = false;
      unawaited(() async {
        try {
          await room.setTyping(false);
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
    final room = _room;
    if (room == null) {
      return;
    }
    try {
      await room.setTyping(active);
    } catch (_) {}
  }

  Future<bool> sendText(String text) async {
    stopTyping();
    try {
      if (isAgent) {
        await _maybeRegisterLocalAgent();
        if (!ref.mounted) {
          return false;
        }
        final serverDest = state.redirectDest;
        if (serverDest != null && serverDest.isNotEmpty) {
          await sendMessageMutation(serverDest).run(ref, (tsx) {
            return _enqueueText(tsx.get(clientPortProvider), serverDest, text);
          });
          return true;
        }
        await sendMessageMutation(dest).run(ref, (tsx) {
          return _enqueueText(tsx.get(clientPortProvider), dest, text);
        });
        return true;
      }
      await sendMessageMutation(dest).run(ref, (tsx) {
        return _enqueueText(tsx.get(clientPortProvider), dest, text);
      });
      return true;
    } on StateError catch (err) {
      _toast(err.message, error: true);
      return false;
    } catch (err, st) {
      KimLogger.warn('sendText', err, st);
      _toast(agentRegisterError(err), error: true);
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
        return _enqueueImages(tsx.get(clientPortProvider), dest, assets);
      });
    } on StateError catch (err) {
      _toast(err.message, error: true);
    } catch (e, st) {
      KimLogger.warn('sendImages', e, st);
      _toast(Copy.sendFailed, error: true);
    }
  }

  Future<void> retry(String key) async {
    KimLogger.info('retry send clientId=$key');
    try {
      await ref.read(clientPortProvider).retrySend(key);
    } on StateError catch (err) {
      _toast(err.message, error: true);
    } catch (e, st) {
      KimLogger.warn('retry send', e, st);
    }
  }

  void consumeToast() {
    if (state.toast != null) {
      state = ChatSessionState(redirectDest: state.redirectDest);
    }
  }

  Future<KimCommandReceipt> _enqueueText(
    KimClientPort client,
    String dest,
    String text,
  ) {
    final body = text.trim();
    if (body.isEmpty) {
      throw StateError(Copy.required);
    }
    final id = const Uuid().v4();
    KimLogger.info('enqueue text dest=$dest clientId=$id kind=$kind');
    return client
        .conversation(dest)
        .sendText(kind: kind, text: body, clientId: id);
  }

  Future<List<KimCommandReceipt>> _enqueueImages(
    KimClientPort client,
    String dest,
    List<KimMediaAsset> assets,
  ) async {
    final out = <KimCommandReceipt>[];
    for (final asset in assets) {
      if (asset.path.isEmpty) {
        continue;
      }
      final id = const Uuid().v4();
      KimLogger.info(
        'enqueue media dest=$dest clientId=$id kind=$kind video=${asset.isVideo}',
      );
      final content = asset.isVideo
          ? KimOutgoingContent.video(url: asset.path)
          : KimOutgoingContent.image(
              url: asset.path,
              width: asset.width,
              height: asset.height,
            );
      out.add(
        await client.enqueueMessage(
          dest: dest,
          kind: kind,
          content: content,
          clientId: id,
          localPath: asset.path,
          width: asset.width,
          height: asset.height,
        ),
      );
    }
    if (out.isEmpty) {
      throw StateError(Copy.required);
    }
    return out;
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

final chatSessionProvider = NotifierProvider.autoDispose
    .family<ChatSessionNotifier, ChatSessionState, String>(
      ChatSessionNotifier.new,
    );
