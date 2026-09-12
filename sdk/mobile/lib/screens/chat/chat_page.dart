library;

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:gap/gap.dart';
import 'package:go_router/go_router.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';
import 'package:toastification/toastification.dart';

import '../../agent/mention.dart';
import '../../copy.dart';
import '../../core/layout.dart';
import '../../models/models.dart';
import '../../state/chat_session.dart';
import '../../state/contacts.dart';
import '../../state/link.dart';
import '../../state/messages.dart';
import '../../state/presence.dart';
import '../../state/profile.dart';
import '../../state/receipts.dart';
import '../../state/session.dart';
import '../../state/typing.dart';
import '../../theme/kim_theme.dart';
import '../../widgets/chat/chat_list.dart';
import '../../widgets/empty_state.dart';
import '../../widgets/kim_avatar.dart';
import '../../widgets/kim_bubble.dart';
import '../../widgets/kim_composer.dart';
import '../../widgets/kim_typing_bars.dart';
import '../../widgets/status_chip.dart';
import 'chat_chrome.dart';

class ChatPage extends ConsumerStatefulWidget {
  const ChatPage({super.key, required this.id});

  final String id;

  @override
  ConsumerState<ChatPage> createState() => _ChatPageState();
}

class _ChatPageState extends ConsumerState<ChatPage> {
  final _list = ChatListController();
  final _composer = GlobalKey<KimComposerState>();

  ChatSessionNotifier get _session =>
      ref.read(chatSessionProvider(widget.id).notifier);

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    final session = ref.watch(chatSessionProvider(widget.id));
    final redirectTo = session.redirectDest;
    if (redirectTo != null &&
        redirectTo.isNotEmpty &&
        redirectTo != widget.id) {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (!context.mounted) {
          return;
        }
        context.replace('/chat/$redirectTo');
      });
    }
    ref.listen(chatSessionProvider(widget.id), (prev, next) {
      final toast = next.toast;
      if (toast == null || toast == prev?.toast) {
        return;
      }
      _showToast(toast, error: next.toastError);
      _session.consumeToast();
    });

    final account = ref.watch(sessionProvider).account;
    final social = ref.watch(contactsProvider);
    final me = ref.watch(profileProvider);
    final thread = ref.watch(threadMessagesProvider(widget.id));
    final kind = _session.kind;
    final agentChat = isAgentDest(widget.id);
    final userThread = kind == ThreadKind.user && !agentChat;
    final peerTyping = userThread
        ? ref.watch(peerTypingProvider(widget.id))
        : false;
    final readUpTo = userThread
        ? ref.watch(peerReadUpToProvider(widget.id))
        : null;
    final liveTitle = kind == ThreadKind.user
        ? (social.person(widget.id)?.title ?? widget.id)
        : widget.id;
    final gated =
        !agentChat &&
        !isServerBotAccount(widget.id) &&
        kind == ThreadKind.user &&
        social.ready &&
        !social.isFriend(widget.id);
    final wide = kimIsWide(context);

    return Scaffold(
      extendBody: true,
      resizeToAvoidBottomInset: true,
      backgroundColor: KimTheme.chatCanvasOf(context),
      body: Builder(
        builder: (context) {
          final media = MediaQuery.of(context);
          final listPadding = EdgeInsets.fromLTRB(
            0,
            72 + media.padding.bottom,
            0,
            64 + media.padding.top,
          );
          return Stack(
            fit: StackFit.expand,
            children: [
              ChatList(
                items: thread.items,
                controller: _list,
                padding: listPadding,
                loadingOlder: thread.loadingOlder,
                hasMore: thread.hasMore,
                onLoadOlder: () => unawaited(
                  ref
                      .read(threadMessagesProvider(widget.id).notifier)
                      .loadOlder(),
                ),
                empty: peerTyping
                    ? null
                    : EmptyState(
                        icon: LucideIcons.messageCircle,
                        title: l10n.noMessages,
                        subtitle: l10n.noMessagesHint,
                      ),
                footer: peerTyping
                    ? KimTypingRow(
                        key: const Key('typing-row'),
                        name: liveTitle,
                        avatar: KimAvatar(
                          name: liveTitle,
                          url: avatarFor(me, social, widget.id),
                          size: KimAvatarSize.sm,
                          shape: KimAvatarShape.squircle,
                        ),
                      )
                    : null,
                itemBuilder: (context, msg, index) {
                  final prev = index > 0 ? thread.items[index - 1] : null;
                  final next = index + 1 < thread.items.length
                      ? thread.items[index + 1]
                      : null;
                  final own = msg.sender == account;
                  final mid = msg.messageId;
                  final showRead =
                      own && readUpTo != null && mid > 0 && mid <= readUpTo;
                  return KimMessageRow(
                    key: Key('msg-${msg.key}'),
                    message: msg,
                    previous: prev,
                    next: next,
                    isSentByMe: own,
                    displayName: msg.sender == account
                        ? l10n.you
                        : (social.person(msg.sender)?.title ?? msg.sender),
                    avatarUrl: avatarFor(me, social, msg.sender),
                    unreadAnchor: thread.unreadAnchorId == msg.key,
                    showRead: showRead,
                    onRetry: msg.isFailed
                        ? () => unawaited(_session.retry(msg.key))
                        : null,
                    onLongPress: (_) => unawaited(
                      showChatMessageSheet(
                        context: context,
                        message: msg,
                        composer: _composer,
                        onRetry: _session.retry,
                        onCopied: _showToast,
                      ),
                    ),
                  );
                },
              ),
              Positioned(
                top: 0,
                left: 0,
                right: 0,
                child: SafeArea(
                  bottom: false,
                  child: Column(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      Padding(
                        padding: const EdgeInsets.fromLTRB(12, 4, 12, 8),
                        child: Row(
                          children: [
                            if (!wide)
                              FrostedCircleButton(
                                key: const Key('chat-back'),
                                onTap: () {
                                  if (context.canPop()) {
                                    context.pop();
                                  } else {
                                    context.go('/');
                                  }
                                },
                                child: const Icon(
                                  LucideIcons.chevronLeft,
                                  size: 22,
                                ),
                              ),
                            if (!wide) const Gap(8),
                            ChatTitleChrome(
                              title: liveTitle,
                              avatarUrl: avatarFor(me, social, widget.id),
                              presence: ref.watch(
                                peerPresenceProvider(widget.id),
                              ),
                            ),
                            const Spacer(),
                            FrostedCircleButton(
                              key: const Key('chat-more'),
                              tooltip: l10n.more,
                              child: const Icon(LucideIcons.ellipsis, size: 20),
                            ),
                          ],
                        ),
                      ),
                      ConnectionBanner(
                        status: ref.watch(sessionProvider).status,
                        error: ref.watch(sessionProvider).connectError,
                        onRetry: () => ref.read(linkProvider.notifier).retry(),
                      ),
                    ],
                  ),
                ),
              ),
              Positioned(
                left: 0,
                right: 0,
                bottom: media.viewInsets.bottom,
                child: gated
                    ? FriendGate(
                        dest: widget.id,
                        title: liveTitle,
                        incoming: social.isIncoming(widget.id),
                        outgoing: social.isOutgoing(widget.id),
                      )
                    : KeyedSubtree(
                        key: const Key('chat-composer'),
                        child: KimComposer(
                          key: _composer,
                          hintText: agentChat
                              ? l10n.agentComposerDirect
                              : l10n.agentComposerHint,
                          onSend: (text) => unawaited(_send(text)),
                          onPickAlbum: () => unawaited(_session.pickAlbum()),
                          onTakePhoto: () => unawaited(_session.takePhoto()),
                          onTypingChanged: userThread
                              ? _session.onComposerTyping
                              : null,
                        ),
                      ),
              ),
            ],
          );
        },
      ),
    );
  }

  Future<void> _send(String text) async {
    final ok = await _session.sendText(text);
    if (ok && _list.atBottomEdge) {
      unawaited(_list.scrollToBottom(animated: true));
    }
  }

  void _showToast(String message, {bool error = false}) {
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
}
