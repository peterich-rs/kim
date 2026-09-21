library;

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:kim_mobile/copy.dart';
import 'package:kim_mobile/design/kim_composer.dart';
import 'package:kim_mobile/features/chats/chat_chrome.dart';
import 'package:kim_mobile/features/chats/chat_session.dart';
import 'package:kim_mobile/features/chats/chat_view.dart';

class ChatComposerBar extends ConsumerWidget {
  const ChatComposerBar({
    super.key,
    required this.dest,
    required this.composer,
    required this.onSend,
  });

  final String dest;
  final GlobalKey<KimComposerState> composer;
  final Future<void> Function(String text) onSend;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = AppLocalizations.of(context);
    final gated = ref.watch(chatChromeProvider(dest).select((c) => c.gated));
    final readOnly = ref.watch(
      chatChromeProvider(dest).select((c) => c.readOnly),
    );
    final agentChat = ref.watch(
      chatChromeProvider(dest).select((c) => c.agentChat),
    );
    final userThread = ref.watch(
      chatChromeProvider(dest).select((c) => c.userThread),
    );
    final title = ref.watch(
      chatChromeProvider(dest).select((c) => c.liveTitle),
    );
    final incoming = ref.watch(
      chatChromeProvider(dest).select((c) => c.incoming),
    );
    final outgoing = ref.watch(
      chatChromeProvider(dest).select((c) => c.outgoing),
    );
    final session = ref.read(chatSessionProvider(dest).notifier);
    if (gated) {
      return FriendGate(
        dest: dest,
        title: title,
        incoming: incoming,
        outgoing: outgoing,
      );
    }
    if (readOnly) {
      return SafeArea(
        top: false,
        child: Padding(
          padding: const EdgeInsets.fromLTRB(24, 12, 24, 24),
          child: Text(
            l10n.agentDeletedReadOnly,
            key: const Key('agent-deleted-readonly'),
            textAlign: TextAlign.center,
            style: Theme.of(context).textTheme.bodyMedium?.copyWith(
              color: Theme.of(context).colorScheme.onSurfaceVariant,
            ),
          ),
        ),
      );
    }
    return KeyedSubtree(
      key: const Key('chat-composer'),
      child: KimComposer(
        key: composer,
        hintText: agentChat ? l10n.agentComposerDirect : l10n.composerHint,
        onSend: (text) => unawaited(onSend(text)),
        onPickAlbum: () => unawaited(session.pickAlbum()),
        onTakePhoto: () => unawaited(session.takePhoto()),
        onTypingChanged: userThread ? session.onComposerTyping : null,
      ),
    );
  }
}
