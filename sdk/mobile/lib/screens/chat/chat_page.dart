library;

import 'package:flutter/material.dart';
import 'package:flutter_chat_core/flutter_chat_core.dart';
import 'package:flutter_chat_ui/flutter_chat_ui.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';
import 'package:toastification/toastification.dart';

import '../../copy.dart';
import '../../models/models.dart';
import '../../state/inbox.dart';
import '../../state/session.dart';
import '../../theme/kim_theme.dart';
import '../../widgets/empty_state.dart';
import '../../widgets/kim_avatar.dart';
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

  Message _toFlyer(KimChatMsg m) {
    return Message.text(
      id: m.key,
      authorId: m.sender,
      createdAt: DateTime.fromMillisecondsSinceEpoch(m.at).toUtc(),
      text: m.body,
      status: m.failed ? MessageStatus.error : MessageStatus.sent,
    );
  }

  Future<void> _send(String text) async {
    try {
      final msg = await ref.read(inboxProvider.notifier).send(widget.id, text);
      await _controller.insertMessage(_toFlyer(msg));
    } catch (err) {
      final message = err.toString().contains(Copy.notConnected)
          ? Copy.notConnected
          : Copy.sendFailed;
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
    final theme = Theme.of(context);
    final account = session.account;

    return Scaffold(
      appBar: AppBar(
        centerTitle: true,
        title: Column(
          children: [
            KimAvatar(
              name: widget.title,
              size: KimAvatarSize.sm,
              heroTag: 'avatar-${widget.id}',
            ),
            const SizedBox(height: 2),
            Text(
              widget.title,
              style: theme.textTheme.labelLarge?.copyWith(
                fontWeight: FontWeight.w600,
              ),
            ),
          ],
        ),
        toolbarHeight: 72,
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
              resolveUser: (id) async => User(
                id: id,
                name: id == account ? Copy.you : id,
              ),
              onMessageSend: _send,
              theme: KimTheme.chat(theme),
              backgroundColor: theme.colorScheme.surface,
              builders: Builders(
                emptyChatListBuilder: (_) => const EmptyState(
                  icon: LucideIcons.messageCircle,
                  title: Copy.noMessages,
                  subtitle: Copy.noMessagesHint,
                ),
                composerBuilder: (_) => Composer(
                  hintText: Copy.messagePlaceholder,
                  filled: true,
                  handleSafeArea: true,
                  sendIcon: const Icon(LucideIcons.send, size: 18),
                  sendButtonVisibilityMode: SendButtonVisibilityMode.hidden,
                  sendOnEnter: true,
                  padding: const EdgeInsets.fromLTRB(10, 8, 10, 8),
                ),
              ),
            ),
          ),
        ],
      ),
    );
  }
}
