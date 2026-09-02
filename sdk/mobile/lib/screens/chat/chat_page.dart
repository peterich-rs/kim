library;

import 'dart:async';
import 'dart:ui';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
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
import '../../state/link.dart';
import '../../state/messages.dart';
import '../../state/outbox.dart';
import '../../state/mutations.dart';
import '../../state/profile.dart';
import '../../state/session.dart';
import '../../theme/kim_theme.dart';
import '../../widgets/chat/chat_list.dart';
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
    this.initialUnread = 0,
  });

  final String id;
  final String title;
  final ThreadKind kind;
  final int initialUnread;

  @override
  ConsumerState<ChatPage> createState() => _ChatPageState();
}

class _ChatPageState extends ConsumerState<ChatPage> {
  final _list = ChatListController();
  final _composer = GlobalKey<KimComposerState>();

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted) {
        return;
      }
      final messages = ref.read(threadMessagesProvider(widget.id).notifier);
      messages.captureUnreadAnchor(
        unread: widget.initialUnread,
        self: ref.read(sessionProvider).account,
      );
      unawaited(messages.reconcile());
      unawaited(messages.markRead());
    });
  }

  Future<void> _send(String text) async {
    try {
      await sendMessageMutation(widget.id).run(ref, (tsx) {
        return tsx.get(outboxProvider.notifier).sendText(widget.id, text);
      });
    } on StateError catch (err) {
      _toast(err.message, error: true);
    } catch (_) {
      // Talk failures stay on the row with a retry control.
    }
    if (_list.atBottomEdge) {
      unawaited(_list.scrollToBottom(animated: true));
    }
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
        return tsx.get(outboxProvider.notifier).sendImages(widget.id, assets);
      });
    } on StateError catch (err) {
      _toast(err.message, error: true);
    } catch (_) {
      _toast(Copy.sendFailed, error: true);
    }
    if (_list.atBottomEdge) {
      unawaited(_list.scrollToBottom(animated: true));
    }
  }

  void _toast(String message, {bool error = false}) {
    if (!mounted) {
      return;
    }
    toastification.show(
      context: context,
      type: error ? ToastificationType.error : ToastificationType.success,
      style: ToastificationStyle.flatColored,
      title: Text(message),
      autoCloseDuration: Duration(seconds: error ? 3 : 2),
      alignment: Alignment.topCenter,
    );
  }

  void _toastMedia(KimMediaPickerException err) {
    _toast(
      err.code == 'permission_denied' ? Copy.mediaPermission : Copy.mediaFailed,
      error: true,
    );
  }

  Future<void> _retry(String key) async {
    try {
      await ref.read(outboxProvider.notifier).retry(widget.id, key);
    } on StateError catch (err) {
      _toast(err.message, error: true);
    } catch (_) {
      // Stay failed; the retry control remains on the row.
    }
  }

  Future<void> _onLongPress(BuildContext context, KimChatMsg message) async {
    await KimHaptics.light();
    if (!context.mounted) {
      return;
    }
    final text = message.sys || message.isImage || message.isVideo
        ? ''
        : message.body;
    final stamp = formatMessageStamp(message.at);
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
                if (text.isNotEmpty)
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
                      _toast(Copy.copied);
                    },
                  ),
                if (text.isNotEmpty)
                  ListTile(
                    leading: const Icon(LucideIcons.quote, size: 18),
                    title: const Text(Copy.quote),
                    onTap: () {
                      Navigator.of(sheetContext).pop();
                      _composer.currentState?.quote(text);
                    },
                  ),
                if (message.isFailed)
                  ListTile(
                    leading: Icon(
                      LucideIcons.refreshCw,
                      size: 18,
                      color: Theme.of(sheetContext).colorScheme.error,
                    ),
                    title: const Text(Copy.retry),
                    onTap: () {
                      Navigator.of(sheetContext).pop();
                      unawaited(_retry(message.key));
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
    final thread = ref.watch(threadMessagesProvider(widget.id));
    final account = session.account;
    final gated =
        widget.kind == ThreadKind.user &&
        social.ready &&
        !social.isFriend(widget.id);

    return Scaffold(
      resizeToAvoidBottomInset: true,
      backgroundColor: KimTheme.chatCanvasOf(context),
      appBar: AppBar(
        centerTitle: false,
        leadingWidth: 52,
        leading: const _FrostedBack(),
        title: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(
              widget.title,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: Theme.of(context).textTheme.titleMedium?.copyWith(
                fontSize: KimTheme.fontTitle,
                fontWeight: FontWeight.w600,
              ),
            ),
            Text(
              session.statusLabel,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: Theme.of(context).textTheme.labelSmall?.copyWith(
                fontSize: KimTheme.fontMeta,
                color: Theme.of(context).colorScheme.onSurfaceVariant,
              ),
            ),
          ],
        ),
      ),
      body: Column(
        children: [
          ConnectionBanner(
            status: session.status,
            error: session.connectError,
            onRetry: () => ref.read(linkProvider.notifier).retry(),
          ),
          Expanded(
            child: ChatList(
              items: thread.items,
              controller: _list,
              loadingOlder: thread.loadingOlder,
              hasMore: thread.hasMore,
              onLoadOlder: () => unawaited(
                ref
                    .read(threadMessagesProvider(widget.id).notifier)
                    .loadOlder(),
              ),
              empty: const EmptyState(
                icon: LucideIcons.messageCircle,
                title: Copy.noMessages,
                subtitle: Copy.noMessagesHint,
              ),
              itemBuilder: (context, msg, index) {
                final prev = index > 0 ? thread.items[index - 1] : null;
                final next = index + 1 < thread.items.length
                    ? thread.items[index + 1]
                    : null;
                return KimMessageRow(
                  key: Key('msg-${msg.key}'),
                  message: msg,
                  previous: prev,
                  next: next,
                  isSentByMe: msg.sender == account,
                  displayName: _nameOf(msg.sender, account, social),
                  avatarUrl: avatarFor(me, social, msg.sender),
                  unreadAnchor: thread.unreadAnchorId == msg.key,
                  onRetry: msg.isFailed
                      ? () => unawaited(_retry(msg.key))
                      : null,
                  onLongPress: (_) => unawaited(_onLongPress(context, msg)),
                );
              },
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
            KeyedSubtree(
              key: const Key('chat-composer'),
              child: KimComposer(
                key: _composer,
                hintText: Copy.messagePlaceholder,
                onSend: (text) => unawaited(_send(text)),
                onPickAlbum: () => unawaited(_pickAlbum()),
                onTakePhoto: () => unawaited(_takePhoto()),
              ),
            ),
        ],
      ),
    );
  }
}

class _FrostedBack extends StatelessWidget {
  const _FrostedBack();

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    return Padding(
      padding: const EdgeInsets.only(left: 8, top: 6, bottom: 6),
      child: Material(
        color: scheme.surface.withValues(alpha: 0.55),
        shape: const CircleBorder(),
        clipBehavior: Clip.antiAlias,
        child: BackdropFilter(
          filter: ImageFilter.blur(sigmaX: 16, sigmaY: 16),
          child: const BackButton(),
        ),
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
