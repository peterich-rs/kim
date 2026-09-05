library;

import 'dart:async';
import 'dart:io';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:kim_media_picker/kim_media_picker.dart';
import 'package:uuid/uuid.dart';

import '../copy.dart';
import '../core/format.dart';
import '../core/haptics.dart';
import '../core/image_extra.dart';
import '../core/media.dart';
import '../core/validation.dart';
import '../models/models.dart';
import 'contacts.dart';
import 'inbox.dart';
import 'link.dart';
import 'messages.dart';
import 'providers.dart';
import 'session.dart';

class OutboxNotifier extends Notifier<int> {
  static const _uuid = Uuid();
  var _pumping = false;

  @override
  int build() {
    ref.listen(linkProvider.select((s) => s.status), (prev, next) {
      if (next == ConnStatus.online) {
        unawaited(replay());
      }
    });
    return 0;
  }

  Future<KimChatMsg> enqueue(
    String dest,
    KimOutgoingContent content, {
    String? batchId,
    ThreadKind kind = ThreadKind.user,
  }) async {
    _assertCanQueue(dest, kind);
    final session = ref.read(sessionProvider);
    ref.read(threadsProvider.notifier).ensureThread(id: dest, kind: kind);
    final msg = _draft(dest, session.account, content, batchId: batchId);
    await _persist(msg, fromSelf: true);
    unawaited(_pump());
    return msg;
  }

  Future<List<KimChatMsg>> enqueueBatch(
    String dest,
    List<KimOutgoingContent> contents, {
    ThreadKind kind = ThreadKind.user,
  }) async {
    if (contents.isEmpty) {
      throw StateError(Copy.required);
    }
    _assertCanQueue(dest, kind);
    final batchId = _uuid.v4();
    final out = <KimChatMsg>[];
    for (final content in contents) {
      out.add(await enqueue(dest, content, batchId: batchId, kind: kind));
    }
    return out;
  }

  Future<KimChatMsg> sendText(String dest, String text) {
    final body = text.trim();
    if (body.isEmpty) {
      throw StateError(Copy.required);
    }
    return enqueue(dest, KimOutgoingContent.text(body));
  }

  Future<List<KimChatMsg>> sendImages(
    String dest,
    List<KimMediaAsset> assets,
  ) async {
    if (assets.isEmpty) {
      throw StateError(Copy.required);
    }
    final contents = <KimOutgoingContent>[];
    for (final asset in assets) {
      if (asset.path.isEmpty) {
        continue;
      }
      if (asset.isVideo) {
        contents.add(KimOutgoingContent.video(url: asset.path));
      } else {
        contents.add(
          KimOutgoingContent.image(
            url: asset.path,
            width: asset.width,
            height: asset.height,
          ),
        );
      }
    }
    if (contents.isEmpty) {
      throw StateError(Copy.required);
    }
    return enqueueBatch(dest, contents);
  }

  Future<void> retry(String dest, String key) async {
    final msgs = ref.read(threadMessagesProvider(dest)).items;
    KimChatMsg? target;
    for (final m in msgs) {
      if (m.key == key) {
        target = m;
        break;
      }
    }
    if (target == null || !target.isFailed) {
      return;
    }
    final next = target.copyWith(failed: false, status: KimSendStatus.sending);
    await _persist(next, fromSelf: true);
    unawaited(_pump());
  }

  Future<void> replay() async {
    final account = ref.read(sessionProvider).account;
    final store = ref.read(conversationStoreProvider);
    final pending = [
      ...store.loadPending(account),
      ...store.loadFailed(account),
    ];
    for (final msg in pending) {
      if (msg.isFailed) {
        await _persist(
          msg.copyWith(status: KimSendStatus.sending, failed: false),
          fromSelf: true,
        );
      }
    }
    unawaited(_pump());
  }

  Future<void> _pump() async {
    if (_pumping) {
      return;
    }
    _pumping = true;
    try {
      while (ref.mounted) {
        if (ref.read(linkProvider).status != ConnStatus.online) {
          return;
        }
        final account = ref.read(sessionProvider).account;
        final pending = ref
            .read(conversationStoreProvider)
            .loadPending(account);
        if (pending.isEmpty) {
          return;
        }
        final keys = {for (final m in pending) m.key};
        for (final msg in pending) {
          if (!ref.mounted) {
            return;
          }
          if (ref.read(linkProvider).status != ConnStatus.online) {
            return;
          }
          await _sendOne(msg);
        }
        final leftover = ref
            .read(conversationStoreProvider)
            .loadPending(account)
            .any((m) => keys.contains(m.key));
        if (leftover) {
          return;
        }
      }
    } finally {
      _pumping = false;
    }
  }

  Future<void> _sendOne(KimChatMsg msg) async {
    try {
      var content = _contentOf(msg);
      if (content is KimImageContent && !isRemoteUrl(content.url)) {
        final url = await _upload(content.url);
        if (!ref.mounted) {
          return;
        }
        content = KimImageContent(
          url: url,
          width: msg.width,
          height: msg.height,
        );
        final patched = msg.copyWith(body: url, status: KimSendStatus.sending);
        await _persist(patched, fromSelf: true);
      }
      final result = await ref
          .read(clientPortProvider)
          .sendMessage(msg.dest, ThreadKind.user, content, clientId: msg.key);
      if (!ref.mounted) {
        return;
      }
      final sent = msg.copyWith(
        body: switch (content) {
          KimImageContent(:final url) => url,
          KimVideoContent(:final url) => url,
          _ => msg.body,
        },
        status: KimSendStatus.sent,
        failed: false,
        messageId: result.messageId,
        at: result.sendTime == 0 ? msg.at : sendTimeMs(result.sendTime),
      );
      await _persist(sent, fromSelf: true);
      await KimHaptics.light();
    } catch (_) {
      if (ref.mounted) {
        await _persist(
          msg.copyWith(status: KimSendStatus.failed, failed: true),
          fromSelf: true,
        );
      }
      await KimHaptics.error();
    }
  }

  KimOutgoingContent _contentOf(KimChatMsg msg) {
    if (msg.isVideo) {
      return KimOutgoingContent.video(url: msg.body);
    }
    if (msg.isImage) {
      return KimOutgoingContent.image(
        url: msg.body,
        width: msg.width,
        height: msg.height,
      );
    }
    return KimOutgoingContent.text(msg.body);
  }

  Future<String> _upload(String path) async {
    final file = File(path);
    if (!file.existsSync()) {
      throw StateError(Copy.imageFailed);
    }
    final bytes = await file.readAsBytes();
    if (bytes.length > KimMediaClient.maxBytes) {
      throw StateError(Copy.imageTooLarge);
    }
    final token = ref.read(runtimeProvider).settings.token;
    final uploaded = await ref
        .read(mediaPortProvider)
        .uploadImage(
          token: token,
          bytes: bytes,
          contentType: KimImageTypes.sniff(bytes) ?? KimImageTypes.jpeg,
        );
    return uploaded.url;
  }

  KimChatMsg _draft(
    String dest,
    String account,
    KimOutgoingContent content, {
    String? batchId,
  }) {
    final now = DateTime.now().millisecondsSinceEpoch;
    return switch (content) {
      KimTextContent(:final text) => KimChatMsg(
        key: _uuid.v4(),
        dest: dest,
        sender: account,
        body: text,
        at: now,
        status: KimSendStatus.sending,
        batchId: batchId,
      ),
      KimImageContent(:final url, :final width, :final height) => KimChatMsg(
        key: _uuid.v4(),
        dest: dest,
        sender: account,
        body: url,
        at: now,
        kind: KimMsgKind.image,
        width: width,
        height: height,
        status: KimSendStatus.sending,
        batchId: batchId,
      ),
      KimVideoContent(:final url) => KimChatMsg(
        key: _uuid.v4(),
        dest: dest,
        sender: account,
        body: url,
        at: now,
        kind: KimMsgKind.video,
        status: KimSendStatus.sending,
        batchId: batchId,
      ),
    };
  }

  Future<void> _persist(KimChatMsg msg, {required bool fromSelf}) async {
    final account = ref.read(sessionProvider).account;
    ref.read(threadsProvider.notifier).applyTalk(msg, fromSelf: fromSelf);
    ref.read(threadMessagesProvider(msg.dest).notifier).receive(msg);
    await ref.read(conversationStoreProvider).upsertMessages(
      account,
      msg.dest,
      [msg],
    );
    final thread = ref.read(threadsProvider).thread(msg.dest);
    if (thread != null) {
      await ref.read(conversationStoreProvider).upsertThread(account, thread);
    }
  }

  void _assertCanQueue(String dest, ThreadKind kind) {
    final accountErr = validateAccount(dest);
    if (accountErr != null) {
      throw StateError(accountErr);
    }
    final session = ref.read(sessionProvider);
    if (dest == session.account) {
      throw StateError(Copy.cannotChatSelf);
    }
    final social = ref.read(contactsProvider);
    if (kind != ThreadKind.group && social.ready && !social.isFriend(dest)) {
      throw StateError(Copy.notFriends);
    }
  }
}

final outboxProvider = NotifierProvider<OutboxNotifier, int>(
  OutboxNotifier.new,
);
