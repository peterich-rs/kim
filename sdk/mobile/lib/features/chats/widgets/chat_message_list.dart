library;

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import 'package:kim_mobile/copy.dart';
import 'package:kim_mobile/design/chat/chat_list.dart';
import 'package:kim_mobile/design/empty_state.dart';
import 'package:kim_mobile/design/kim_bubble.dart';
import 'package:kim_mobile/design/kim_composer.dart';
import 'package:kim_mobile/features/agent/agent_permission.dart';
import 'package:kim_mobile/features/chats/chat_chrome.dart';
import 'package:kim_mobile/features/chats/chat_session.dart';
import 'package:kim_mobile/features/chats/chat_view.dart';
import 'package:kim_mobile/features/chats/messages.dart';
import 'package:kim_mobile/features/chats/widgets/chat_permission_strip.dart';
import 'package:kim_mobile/features/contacts/contacts.dart';
import 'package:kim_mobile/features/profile/profile.dart';
import 'package:kim_mobile/features/session/receipts.dart';
import 'package:kim_mobile/features/session/session.dart';
import 'package:kim_mobile/features/session/typing.dart';
import 'package:kim_mobile/models/models.dart';
import 'package:kim_mobile/router/open_peer.dart';

class ChatMessageList extends ConsumerWidget {
  const ChatMessageList({
    super.key,
    required this.dest,
    required this.controller,
    required this.composer,
    required this.onCopied,
  });

  final String dest;
  final ChatListController controller;
  final GlobalKey<KimComposerState> composer;
  final void Function(String message) onCopied;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = AppLocalizations.of(context);
    final thread = ref.watch(threadMessagesProvider(dest));
    final account = ref.watch(sessionProvider.select((s) => s.account));
    final social = ref.watch(contactsProvider);
    final me = ref.watch(profileProvider);
    final readOnly = ref.watch(
      chatChromeProvider(dest).select((c) => c.readOnly),
    );
    final userThread = ref.watch(
      chatChromeProvider(dest).select((c) => c.userThread),
    );
    final kind = ref.watch(chatChromeProvider(dest).select((c) => c.kind));
    final liveTitle = ref.watch(
      chatChromeProvider(dest).select((c) => c.liveTitle),
    );
    final readUpTo = userThread ? ref.watch(peerReadUpToProvider(dest)) : null;
    final showTyping = ref.watch(
      chatChromeProvider(dest).select((c) => c.showTyping),
    );
    final agentThread = ref.watch(
      chatChromeProvider(dest).select((c) => c.agentThread),
    );
    final typing = showTyping && ref.watch(peerTypingProvider(dest));
    final prompts = agentThread
        ? ref.watch(agentPermissionHubProvider).of(dest)
        : const <AgentPermissionPrompt>[];
    final footerBusy = typing || prompts.isNotEmpty;
    final media = MediaQuery.of(context);
    return ChatList(
      items: thread.items,
      controller: controller,
      padding: EdgeInsets.fromLTRB(
        0,
        72 + media.padding.bottom,
        0,
        64 + media.padding.top,
      ),
      loadingOlder: thread.loadingOlder,
      hasMore: thread.hasMore,
      onLoadOlder: () => unawaited(
        ref.read(threadMessagesProvider(dest).notifier).loadOlder(),
      ),
      empty: footerBusy
          ? null
          : EmptyState(
              icon: LucideIcons.messageCircle,
              title: readOnly ? l10n.agentDeletedReadOnly : l10n.noMessages,
              subtitle: readOnly ? '' : l10n.noMessagesHint,
            ),
      footer: footerBusy ? ChatListFooter(dest: dest) : null,
      itemBuilder: (context, msg, index) {
        final prev = index > 0 ? thread.items[index - 1] : null;
        final next = index + 1 < thread.items.length
            ? thread.items[index + 1]
            : null;
        final own = msg.sender == account;
        final mid = msg.messageId;
        final showRead = own && readUpTo != null && mid > 0 && mid <= readUpTo;
        final session = ref.read(chatSessionProvider(dest).notifier);
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
              ? () => unawaited(session.retry(msg.key))
              : null,
          onLongPress: (_) => unawaited(
            showChatMessageSheet(
              context: context,
              message: msg,
              composer: composer,
              onRetry: session.retry,
              onCopied: onCopied,
            ),
          ),
          onAvatarTap: !own && kind == ThreadKind.user
              ? () => openKimPeerProfile(
                  context,
                  ref,
                  id: msg.sender.isEmpty ? dest : msg.sender,
                  title:
                      social
                          .person(msg.sender.isEmpty ? dest : msg.sender)
                          ?.title ??
                      (msg.sender.isEmpty ? liveTitle : msg.sender),
                )
              : null,
        );
      },
    );
  }
}
