library;

import 'package:flutter/material.dart';
import 'package:flutter_chat_core/flutter_chat_core.dart';
import 'package:flutter_chat_ui/flutter_chat_ui.dart';

import 'kim_avatar.dart';

BorderRadius kimBubbleRadius({
  required bool sentByMe,
  MessageGroupStatus? group,
}) {
  const sharp = 4.0;
  const round = 16.0;
  final last = group?.isLast ?? true;
  if (sentByMe) {
    return BorderRadius.only(
      topLeft: const Radius.circular(round),
      topRight: const Radius.circular(round),
      bottomLeft: const Radius.circular(round),
      bottomRight: Radius.circular(last ? sharp : round),
    );
  }
  return BorderRadius.only(
    topLeft: const Radius.circular(round),
    topRight: const Radius.circular(round),
    bottomRight: const Radius.circular(round),
    bottomLeft: Radius.circular(last ? sharp : round),
  );
}

Widget kimTextMessage(
  BuildContext context,
  TextMessage message,
  int index, {
  required bool isSentByMe,
  MessageGroupStatus? groupStatus,
}) {
  return SimpleTextMessage(
    message: message,
    index: index,
    padding: const EdgeInsets.fromLTRB(12, 8, 12, 8),
    borderRadius: kimBubbleRadius(sentByMe: isSentByMe, group: groupStatus),
    showTime: groupStatus?.isLast ?? true,
    showStatus: false,
    timeAndStatusPosition: TimeAndStatusPosition.end,
  );
}

Widget kimChatMessage(
  BuildContext context,
  Message message,
  int index,
  Animation<double> animation,
  Widget child, {
  bool? isRemoved,
  required bool isSentByMe,
  MessageGroupStatus? groupStatus,
}) {
  final first = groupStatus?.isFirst ?? true;
  Widget? leading;
  if (!isSentByMe) {
    leading = first
        ? KimAvatar(name: message.authorId, size: KimAvatarSize.sm)
        : const SizedBox(width: 36);
  }
  return ChatMessage(
    message: message,
    index: index,
    animation: animation,
    isRemoved: isRemoved,
    groupStatus: groupStatus,
    horizontalPadding: 12,
    verticalPadding: 8,
    verticalGroupedPadding: 2,
    sentMessageRowAlignment: CrossAxisAlignment.end,
    receivedMessageRowAlignment: CrossAxisAlignment.end,
    leadingWidget: leading,
    topWidget: !isSentByMe && first
        ? Padding(
            padding: const EdgeInsets.only(left: 4, bottom: 2),
            child: Text(
              message.authorId,
              style: Theme.of(context).textTheme.labelSmall?.copyWith(
                color: Theme.of(context).colorScheme.onSurfaceVariant,
                fontWeight: FontWeight.w600,
              ),
            ),
          )
        : null,
    child: child,
  );
}

class KimComposer extends StatelessWidget {
  const KimComposer({super.key, this.hintText});

  final String? hintText;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final scheme = theme.colorScheme;
    return Composer(
      hintText: hintText,
      filled: true,
      handleSafeArea: true,
      sendIcon: Icon(Icons.send_rounded, size: 18, color: scheme.onPrimary),
      sendButtonVisibilityMode: SendButtonVisibilityMode.disabled,
      sendOnEnter: true,
      gap: 8,
      padding: const EdgeInsets.fromLTRB(12, 8, 12, 8),
      backgroundColor: scheme.surface,
      inputFillColor: scheme.surfaceContainerHighest.withValues(alpha: 0.72),
      inputBorder: OutlineInputBorder(
        borderRadius: BorderRadius.circular(22),
        borderSide: BorderSide.none,
      ),
      sendIconColor: scheme.onPrimary,
      emptyFieldSendIconColor: scheme.onSurfaceVariant,
    );
  }
}
