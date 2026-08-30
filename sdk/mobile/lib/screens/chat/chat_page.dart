library;

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_chat_core/flutter_chat_core.dart';
import 'package:flutter_chat_ui/flutter_chat_ui.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:gap/gap.dart';
import 'package:kim_media_picker/kim_media_picker.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';
import 'package:toastification/toastification.dart';
import 'package:wolt_modal_sheet/wolt_modal_sheet.dart';

import '../../copy.dart';
import '../../core/format.dart';
import '../../core/haptics.dart';
import '../../models/models.dart';
import '../../state/contacts.dart';
import '../../state/gateway.dart';
import '../../state/inbox.dart';
import '../../state/mutations.dart';
import '../../state/profile.dart';
import '../../state/session.dart';
import '../../theme/kim_theme.dart';
import '../../widgets/empty_state.dart';
import '../../widgets/kim_avatar.dart';
import '../../widgets/kim_bubble.dart';
import '../../widgets/kim_composer.dart';
import '../../widgets/kim_hairline.dart';
import '../../widgets/status_chip.dart';

class ChatPage extends ConsumerStatefulWidget {
  const ChatPage({
    super.key,
    required this.id,
    required this.title,
    required this.kind,
  });

  final String id;
  final String title;
  final ThreadKind kind;

  @override
  ConsumerState<ChatPage> createState() => _ChatPageState();
}

class _ChatPageState extends ConsumerState<ChatPage> {
  late final InMemoryChatController _controller;

  @override
  void initState() {
    super.initState();
    final rows = ref.read(inboxProvider.notifier).messagesFor(widget.id);
    _controller = InMemoryChatController(
      messages: [for (final m in rows) _toFlyer(m)],
    );
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  Future<void> _sync(List<KimChatMsg> rows) async {
    final have = {for (final m in _controller.messages) m.id: m};
    for (final m in rows) {
      final next = _toFlyer(m);
      final old = have[m.key];
      if (old == null) {
        await _controller.insertMessage(next);
      } else if (old.status != next.status) {
        await _controller.updateMessage(old, next);
      }
    }
  }

  Message _toFlyer(KimChatMsg m) {
    final at = (dateTimeFromEpoch(m.at) ?? DateTime.now()).toUtc();
    if (m.sys) {
      return Message.system(
        id: m.key,
        authorId: m.sender,
        createdAt: at,
        text: m.body,
      );
    }
    if (m.isVideo) {
      return Message.video(
        id: m.key,
        authorId: m.sender,
        createdAt: at,
        source: m.body,
        width: m.width > 0 ? m.width.toDouble() : null,
        height: m.height > 0 ? m.height.toDouble() : null,
        status: m.failed ? MessageStatus.error : MessageStatus.sent,
      );
    }
    if (m.isImage) {
      return Message.image(
        id: m.key,
        authorId: m.sender,
        createdAt: at,
        source: m.body,
        width: m.width > 0 ? m.width.toDouble() : null,
        height: m.height > 0 ? m.height.toDouble() : null,
        status: m.failed ? MessageStatus.error : MessageStatus.sent,
      );
    }
    return Message.text(
      id: m.key,
      authorId: m.sender,
      createdAt: at,
      text: m.body,
      status: m.failed ? MessageStatus.error : MessageStatus.sent,
    );
  }

  Future<void> _send(String text) async {
    final send = sendMessageMutation(widget.id);
    try {
      await send.run(ref, (tsx) {
        return tsx.get(inboxProvider.notifier).send(widget.id, text);
      });
    } on StateError catch (err) {
      if (mounted) {
        toastification.show(
          context: context,
          type: ToastificationType.error,
          style: ToastificationStyle.flatColored,
          title: Text(err.message),
          autoCloseDuration: const Duration(seconds: 3),
          alignment: Alignment.topCenter,
        );
      }
    } catch (_) {
      // Talk failures stay on the row with a retry control.
    }
    if (!mounted) {
      return;
    }
    await _sync(ref.read(inboxProvider).messages[widget.id] ?? const []);
  }

  Future<void> _pickAlbum() async {
    try {
      final assets = await KimMediaPicker.instance.pickMultiple();
      if (assets.isEmpty) {
        return;
      }
      await _sendImages(assets);
    } on MissingPluginException {
      return;
    } on KimMediaPickerException catch (err) {
      _toastMedia(err);
    }
  }

  Future<void> _takePhoto() async {
    try {
      final shot = await KimMediaPicker.instance.capture();
      if (shot == null) {
        return;
      }
      await _sendImages([shot]);
    } on MissingPluginException {
      return;
    } on KimMediaPickerException catch (err) {
      _toastMedia(err);
    }
  }

  Future<void> _sendImages(List<KimMediaAsset> assets) async {
    try {
      await sendImagesMutation(widget.id).run(ref, (tsx) {
        return tsx.get(inboxProvider.notifier).sendImages(widget.id, assets);
      });
    } on StateError catch (err) {
      if (mounted) {
        toastification.show(
          context: context,
          type: ToastificationType.error,
          style: ToastificationStyle.flatColored,
          title: Text(err.message),
          autoCloseDuration: const Duration(seconds: 3),
          alignment: Alignment.topCenter,
        );
      }
    } catch (_) {
      if (mounted) {
        toastification.show(
          context: context,
          type: ToastificationType.error,
          style: ToastificationStyle.flatColored,
          title: const Text(Copy.sendFailed),
          autoCloseDuration: const Duration(seconds: 3),
          alignment: Alignment.topCenter,
        );
      }
    }
    if (!mounted) {
      return;
    }
    await _sync(ref.read(inboxProvider).messages[widget.id] ?? const []);
  }

  void _toastMedia(KimMediaPickerException err) {
    if (!mounted) {
      return;
    }
    toastification.show(
      context: context,
      type: ToastificationType.error,
      style: ToastificationStyle.flatColored,
      title: Text(
        err.code == 'permission_denied'
            ? Copy.mediaPermission
            : Copy.mediaFailed,
      ),
      autoCloseDuration: const Duration(seconds: 3),
      alignment: Alignment.topCenter,
    );
  }

  Future<void> _retry(String key) async {
    try {
      await ref.read(inboxProvider.notifier).retry(widget.id, key);
    } on StateError catch (err) {
      if (!mounted) {
        return;
      }
      toastification.show(
        context: context,
        type: ToastificationType.error,
        style: ToastificationStyle.flatColored,
        title: Text(err.message),
        autoCloseDuration: const Duration(seconds: 3),
        alignment: Alignment.topCenter,
      );
    } catch (_) {
      // Stay failed; the retry control remains on the row.
    }
    if (!mounted) {
      return;
    }
    await _sync(ref.read(inboxProvider).messages[widget.id] ?? const []);
  }

  Future<void> _onLongPress(
    BuildContext context,
    Message message, {
    required int index,
    required LongPressStartDetails details,
  }) async {
    final text = switch (message) {
      TextMessage(:final text) => text,
      SystemMessage(:final text) => text,
      _ => '',
    };
    if (text.isEmpty) {
      return;
    }
    await KimHaptics.light();
    if (!context.mounted) {
      return;
    }
    final stamp = formatMessageStampAt(message.createdAt);
    await WoltModalSheet.show<void>(
      context: context,
      showDragHandle: true,
      pageListBuilder: (sheetContext) => [
        WoltModalSheetPage(
          hasSabGradient: false,
          navBarHeight: 28,
          child: Padding(
            padding: const EdgeInsets.fromLTRB(8, 0, 8, 28),
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: [
                if (stamp.isNotEmpty) ListTile(dense: true, title: Text(stamp)),
                ListTile(
                  leading: const Icon(LucideIcons.copy, size: 18),
                  title: const Text(Copy.copy),
                  onTap: () async {
                    await Clipboard.setData(ClipboardData(text: text));
                    if (sheetContext.mounted) {
                      Navigator.of(sheetContext).pop();
                    }
                    if (!context.mounted) {
                      return;
                    }
                    toastification.show(
                      context: context,
                      type: ToastificationType.success,
                      style: ToastificationStyle.flatColored,
                      title: const Text(Copy.copied),
                      autoCloseDuration: const Duration(seconds: 2),
                      alignment: Alignment.topCenter,
                    );
                  },
                ),
              ],
            ),
          ),
        ),
      ],
    );
  }

  String _nameOf(String authorId, String me, ContactsState social) {
    if (authorId == me) {
      return Copy.you;
    }
    return social.person(authorId)?.title ?? authorId;
  }

  @override
  Widget build(BuildContext context) {
    final session = ref.watch(sessionProvider);
    final social = ref.watch(contactsProvider);
    final me = ref.watch(profileProvider);
    final theme = Theme.of(context);
    final account = session.account;
    final gated =
        widget.kind == ThreadKind.user &&
        social.ready &&
        !social.isFriend(widget.id);

    ref.listen<List<KimChatMsg>>(
      inboxProvider.select((s) => s.messages[widget.id] ?? const []),
      (prev, next) => unawaited(_sync(next)),
    );

    return Scaffold(
      backgroundColor: KimTheme.chatCanvasOf(context),
      appBar: AppBar(
        centerTitle: true,
        title: Text(widget.title, maxLines: 1, overflow: TextOverflow.ellipsis),
      ),
      body: Column(
        children: [
          ConnectionBanner(
            status: session.status,
            error: session.connectError,
            onRetry: () => ref.invalidate(gatewayProvider),
          ),
          Expanded(
            child: Chat(
              chatController: _controller,
              currentUserId: account,
              resolveUser: (id) async =>
                  User(id: id, name: _nameOf(id, account, social)),
              onMessageSend: _send,
              onMessageLongPress: _onLongPress,
              theme: KimTheme.chat(theme),
              backgroundColor: KimTheme.chatCanvasOf(context),
              builders: Builders(
                emptyChatListBuilder: (_) => const EmptyState(
                  icon: LucideIcons.messageCircle,
                  title: Copy.noMessages,
                  subtitle: Copy.noMessagesHint,
                ),
                textMessageBuilder: kimTextMessage,
                imageMessageBuilder: kimImageMessage,
                videoMessageBuilder: kimVideoMessage,
                systemMessageBuilder: kimSystemMessage,
                chatAnimatedListBuilder: (context, itemBuilder) {
                  return ChatAnimatedList(
                    itemBuilder: itemBuilder,
                    handleSafeArea: false,
                    bottomPadding: 8,
                  );
                },
                composerBuilder: (_) => const SizedBox.shrink(),
                chatMessageBuilder:
                    (
                      context,
                      message,
                      index,
                      animation,
                      child, {
                      isRemoved,
                      required isSentByMe,
                      groupStatus,
                    }) {
                      DateTime? previousCreatedAt;
                      final rows = _controller.messages;
                      if (index > 0 && index < rows.length) {
                        previousCreatedAt = rows[index - 1].createdAt;
                      }
                      return kimChatMessage(
                        context,
                        message,
                        index,
                        animation,
                        child,
                        isRemoved: isRemoved,
                        isSentByMe: isSentByMe,
                        groupStatus: groupStatus,
                        displayName: _nameOf(message.authorId, account, social),
                        avatarUrl: avatarFor(me, social, message.authorId),
                        onRetry: message.status == MessageStatus.error
                            ? () => unawaited(_retry(message.id))
                            : null,
                        previousCreatedAt: previousCreatedAt,
                        onLongPress: (details) {
                          unawaited(
                            _onLongPress(
                              context,
                              message,
                              index: index,
                              details: details,
                            ),
                          );
                        },
                      );
                    },
              ),
            ),
          ),
          if (gated)
            _FriendGate(
              dest: widget.id,
              title: widget.title,
              incoming: social.isIncoming(widget.id),
              outgoing: social.isOutgoing(widget.id),
            )
          else
            KimComposer(
              key: const Key('chat-composer'),
              hintText: Copy.messagePlaceholder,
              onSend: (text) => unawaited(_send(text)),
              onPickAlbum: () => unawaited(_pickAlbum()),
              onTakePhoto: () => unawaited(_takePhoto()),
            ),
        ],
      ),
    );
  }
}

class _FriendGate extends ConsumerWidget {
  const _FriendGate({
    required this.dest,
    required this.title,
    required this.incoming,
    required this.outgoing,
  });

  final String dest;
  final String title;
  final bool incoming;
  final bool outgoing;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final theme = Theme.of(context);
    final avatarUrl = avatarFor(
      ref.watch(profileProvider),
      ref.watch(contactsProvider),
      dest,
    );
    Future<void> run(Future<void> Function() action, String ok) async {
      try {
        await action();
        if (context.mounted) {
          toastification.show(
            context: context,
            type: ToastificationType.success,
            style: ToastificationStyle.flatColored,
            title: Text(ok),
            autoCloseDuration: const Duration(seconds: 2),
            alignment: Alignment.topCenter,
          );
        }
      } catch (err) {
        if (context.mounted) {
          toastification.show(
            context: context,
            type: ToastificationType.error,
            style: ToastificationStyle.flatColored,
            title: Text(socialError(err)),
            autoCloseDuration: const Duration(seconds: 3),
            alignment: Alignment.topCenter,
          );
        }
      }
    }

    return Material(
      color: KimTheme.raisedOf(context),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          const KimHairline(),
          SafeArea(
            child: Padding(
              padding: const EdgeInsets.fromLTRB(16, 8, 16, 16),
              child: DecoratedBox(
                decoration: BoxDecoration(
                  color: theme.colorScheme.surface,
                  borderRadius: BorderRadius.circular(KimTheme.radiusCard),
                  border: Border.all(color: KimTheme.hairlineOf(context)),
                ),
                child: Padding(
                  padding: const EdgeInsets.fromLTRB(16, 16, 16, 16),
                  child: Column(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      KimAvatar(name: title, url: avatarUrl),
                      const Gap(10),
                      Text(Copy.notFriends, style: theme.textTheme.titleSmall),
                      const Gap(4),
                      Text(
                        outgoing
                            ? Copy.waitingAccept
                            : incoming
                            ? Copy.friendRequestToast
                            : Copy.addFriendToChat,
                        textAlign: TextAlign.center,
                        style: theme.textTheme.bodySmall,
                      ),
                      const Gap(12),
                      if (outgoing)
                        Text(Copy.requested, style: theme.textTheme.labelMedium)
                      else if (incoming)
                        FilledButton(
                          onPressed: () => run(
                            () => friendAcceptMutation.run(ref, (tsx) {
                              return tsx
                                  .get(contactsProvider.notifier)
                                  .accept(dest);
                            }),
                            Copy.friendAccepted,
                          ),
                          child: const Text(Copy.accept),
                        )
                      else
                        FilledButton.tonal(
                          onPressed: () => run(
                            () => friendRequestMutation.run(ref, (tsx) {
                              return tsx
                                  .get(contactsProvider.notifier)
                                  .request(dest);
                            }),
                            Copy.requestSent,
                          ),
                          child: const Text(Copy.addFriend),
                        ),
                    ],
                  ),
                ),
              ),
            ),
          ),
        ],
      ),
    );
  }
}
