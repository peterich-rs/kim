library;

import 'package:flutter/material.dart';
import 'package:flutter_chat_core/flutter_chat_core.dart';
import 'package:flutter_chat_ui/flutter_chat_ui.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:gap/gap.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';
import 'package:toastification/toastification.dart';

import '../../copy.dart';
import '../../core/errors.dart';
import '../../core/format.dart';
import '../../models/models.dart';
import '../../state/contacts.dart';
import '../../state/inbox.dart';
import '../../state/session.dart';
import '../../theme/kim_theme.dart';
import '../../widgets/empty_state.dart';
import '../../widgets/kim_avatar.dart';
import '../../widgets/kim_bubble.dart';
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

  void _sync(List<KimChatMsg> rows) {
    final have = {for (final m in _controller.messages) m.id};
    for (final m in rows) {
      if (!have.contains(m.key)) {
        _controller.insertMessage(_toFlyer(m));
      }
    }
  }

  Message _toFlyer(KimChatMsg m) {
    return Message.text(
      id: m.key,
      authorId: m.sender,
      createdAt: (dateTimeFromEpoch(m.at) ?? DateTime.now()).toUtc(),
      text: m.body,
      status: m.failed ? MessageStatus.error : MessageStatus.sent,
    );
  }

  Future<void> _send(String text) async {
    final inbox = ref.read(inboxProvider.notifier);
    try {
      final msg = await inbox.send(widget.id, text);
      if (!mounted) {
        return;
      }
      await _controller.insertMessage(_toFlyer(msg));
    } catch (err) {
      final message = mapTalkError(err);
      if (!mounted) {
        return;
      }
      toastification.show(
        context: context,
        type: ToastificationType.error,
        style: ToastificationStyle.flatColored,
        title: Text(message),
        autoCloseDuration: const Duration(seconds: 3),
        alignment: Alignment.topCenter,
      );
    }
  }

  @override
  Widget build(BuildContext context) {
    final session = ref.watch(sessionProvider);
    final social = ref.watch(contactsProvider);
    final theme = Theme.of(context);
    final account = session.account;
    final gated =
        widget.kind == ThreadKind.user &&
        social.ready &&
        !social.isFriend(widget.id);

    ref.listen<List<KimChatMsg>>(
      inboxProvider.select((s) => s.messages[widget.id] ?? const []),
      (prev, next) => _sync(next),
    );

    return Scaffold(
      appBar: AppBar(
        titleSpacing: 0,
        title: Row(
          children: [
            KimAvatar(
              name: widget.title,
              size: KimAvatarSize.sm,
              heroTag: 'avatar-${widget.id}',
            ),
            const SizedBox(width: 10),
            Expanded(
              child: Text(
                widget.title,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: theme.textTheme.titleMedium?.copyWith(
                  fontWeight: FontWeight.w600,
                ),
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
            onRetry: () => ref.read(sessionProvider.notifier).connect(),
          ),
          Expanded(
            child: Chat(
              chatController: _controller,
              currentUserId: account,
              resolveUser: (id) async =>
                  User(id: id, name: id == account ? Copy.you : id),
              onMessageSend: _send,
              theme: KimTheme.chat(theme),
              backgroundColor: theme.colorScheme.surfaceContainerLowest,
              builders: Builders(
                emptyChatListBuilder: (_) => const EmptyState(
                  icon: LucideIcons.messageCircle,
                  title: Copy.noMessages,
                  subtitle: Copy.noMessagesHint,
                ),
                textMessageBuilder: kimTextMessage,
                chatMessageBuilder: kimChatMessage,
                composerBuilder: (_) {
                  if (gated) {
                    return _FriendGate(
                      dest: widget.id,
                      title: widget.title,
                      incoming: social.isIncoming(widget.id),
                      outgoing: social.isOutgoing(widget.id),
                    );
                  }
                  return const KimComposer(hintText: Copy.messagePlaceholder);
                },
              ),
            ),
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

    return SafeArea(
      child: Padding(
        padding: const EdgeInsets.fromLTRB(16, 8, 16, 16),
        child: DecoratedBox(
          decoration: BoxDecoration(
            color: theme.colorScheme.surfaceContainerHighest,
            borderRadius: BorderRadius.circular(16),
          ),
          child: Padding(
            padding: const EdgeInsets.fromLTRB(16, 16, 16, 16),
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: [
                KimAvatar(name: title),
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
                      () => ref.read(contactsProvider.notifier).accept(dest),
                      Copy.friendAccepted,
                    ),
                    child: const Text(Copy.accept),
                  )
                else
                  FilledButton.tonal(
                    onPressed: () => run(
                      () => ref.read(contactsProvider.notifier).request(dest),
                      Copy.requestSent,
                    ),
                    child: const Text(Copy.addFriend),
                  ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}
