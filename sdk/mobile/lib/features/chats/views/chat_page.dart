library;

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:toastification/toastification.dart';

import 'package:kim_mobile/design/chat/chat_list.dart';
import 'package:kim_mobile/design/kim_composer.dart';
import 'package:kim_mobile/design/kim_theme.dart';
import 'package:kim_mobile/features/agent/agent_presence.dart';
import 'package:kim_mobile/features/agent/mention.dart';
import 'package:kim_mobile/features/chats/providers/chat_session.dart';
import 'package:kim_mobile/features/chats/providers/messages.dart';
import 'package:kim_mobile/features/chats/views/widgets/chat_composer_bar.dart';
import 'package:kim_mobile/features/chats/views/widgets/chat_header.dart';
import 'package:kim_mobile/features/chats/views/widgets/chat_message_list.dart';
import 'package:kim_mobile/features/session/session.dart';
import 'package:kim_mobile/router/app_routes.dart';

/// Assembles the conversation. Subscriptions live in the child widgets.
class ChatPage extends ConsumerStatefulWidget {
  const ChatPage({super.key, required this.id});

  final String id;

  @override
  ConsumerState<ChatPage> createState() => _ChatPageState();
}

class _ChatPageState extends ConsumerState<ChatPage> {
  final _list = ChatListController();
  final _composer = GlobalKey<KimComposerState>();

  bool get _agentThread =>
      isAgentDest(widget.id) || isServerBotAccount(widget.id);

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted) {
        return;
      }
      _markOpenedIfAgent();
    });
  }

  @override
  void didUpdateWidget(ChatPage oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.id != widget.id) {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (!mounted) {
          return;
        }
        _markOpenedIfAgent();
      });
    }
  }

  void _markOpenedIfAgent() {
    if (!_agentThread) {
      return;
    }
    ref.read(agentRunStatusProvider.notifier).markOpened(widget.id);
  }

  @override
  Widget build(BuildContext context) {
    ref.listen(chatSessionProvider(widget.id), (prev, next) {
      final redirect = next.redirectDest;
      if (redirect != null &&
          redirect.isNotEmpty &&
          redirect != widget.id &&
          redirect != prev?.redirectDest) {
        context.replace(AppRoutes.chat(redirect));
      }
      final toast = next.toast;
      if (toast == null || toast == prev?.toast) {
        return;
      }
      _showToast(toast, error: next.toastError);
      ref.read(chatSessionProvider(widget.id).notifier).consumeToast();
    });
    ref.listen(threadMessagesProvider(widget.id), (prev, next) {
      if (!_agentThread || prev == null || prev.items.isEmpty) {
        return;
      }
      final prevKeys = {for (final m in prev.items) m.key};
      final added = next.items.where((m) => !prevKeys.contains(m.key)).toList();
      if (added.length != 1 || added.single.sys) {
        return;
      }
      final me = ref.read(sessionProvider).account;
      if (me.isNotEmpty && added.single.sender == me) {
        return;
      }
      ref.read(agentRunStatusProvider.notifier).reviewPulse(widget.id);
    });

    return Scaffold(
      extendBody: true,
      resizeToAvoidBottomInset: true,
      backgroundColor: KimTheme.chatCanvasOf(context),
      body: Builder(
        builder: (context) {
          final inset = MediaQuery.viewInsetsOf(context).bottom;
          return Stack(
            fit: StackFit.expand,
            children: [
              ChatMessageList(
                dest: widget.id,
                controller: _list,
                composer: _composer,
                onCopied: _showToast,
              ),
              Positioned(
                top: 0,
                left: 0,
                right: 0,
                child: ChatHeader(dest: widget.id),
              ),
              Positioned(
                left: 0,
                right: 0,
                bottom: inset,
                child: ChatComposerBar(
                  dest: widget.id,
                  composer: _composer,
                  onSend: _send,
                ),
              ),
            ],
          );
        },
      ),
    );
  }

  Future<void> _send(String text) async {
    final ok = await ref
        .read(chatSessionProvider(widget.id).notifier)
        .sendText(text);
    if (ok) {
      _composer.currentState?.clear();
      if (_list.atBottomEdge) {
        unawaited(_list.scrollToBottom(animated: true));
      }
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
