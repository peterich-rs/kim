library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:kim_mobile/design/agent_action_bubble.dart';
import 'package:kim_mobile/design/kim_avatar.dart';
import 'package:kim_mobile/design/kim_typing_bars.dart';
import 'package:kim_mobile/features/agent/agent_permission.dart';
import 'package:kim_mobile/features/chats/providers/chat_view.dart';
import 'package:kim_mobile/features/session/typing.dart';

/// Permission cards and the typing row. Isolated so a typing tick does not
/// rebuild the message list.
class ChatListFooter extends ConsumerWidget {
  const ChatListFooter({super.key, required this.dest});

  final String dest;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final showTyping = ref.watch(
      chatChromeProvider(dest).select((c) => c.showTyping),
    );
    final agentThread = ref.watch(
      chatChromeProvider(dest).select((c) => c.agentThread),
    );
    final title = ref.watch(
      chatChromeProvider(dest).select((c) => c.liveTitle),
    );
    final avatarUrl = ref.watch(
      chatChromeProvider(dest).select((c) => c.avatarUrl),
    );
    final typing = showTyping && ref.watch(peerTypingProvider(dest));
    final prompts = agentThread
        ? ref.watch(agentPermissionHubProvider).of(dest)
        : const <AgentPermissionPrompt>[];
    if (prompts.isEmpty && !typing) {
      return const SizedBox.shrink();
    }
    return Column(
      mainAxisSize: MainAxisSize.min,
      children: [
        for (final prompt in prompts)
          AgentActionBubble(
            key: Key('agent-ask-${prompt.callId}'),
            dest: dest,
            callId: prompt.callId,
            name: prompt.name,
            preview: prompt.preview,
          ),
        if (typing && prompts.isEmpty)
          KimTypingRow(
            key: const Key('typing-row'),
            name: title,
            avatar: KimAvatar(
              name: title,
              url: avatarUrl,
              size: KimAvatarSize.sm,
              shape: KimAvatarShape.squircle,
            ),
          ),
      ],
    );
  }
}
