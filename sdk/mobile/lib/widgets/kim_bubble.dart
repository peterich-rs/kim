library;

import 'package:flutter/material.dart';
import 'package:flutter_chat_core/flutter_chat_core.dart';
import 'package:flutter_chat_ui/flutter_chat_ui.dart';
import 'package:gap/gap.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import '../copy.dart';
import '../core/format.dart';
import '../theme/kim_theme.dart';
import '../theme/motion.dart';
import 'kim_avatar.dart';
import 'kim_hairline.dart';

Widget kimTextMessage(
  BuildContext context,
  TextMessage message,
  int index, {
  required bool isSentByMe,
  MessageGroupStatus? groupStatus,
}) {
  return Text(
    message.text,
    style: Theme.of(context).textTheme.bodyMedium?.copyWith(
      fontSize: 15,
      height: 1.375,
      color: Theme.of(context).colorScheme.onSurface,
    ),
  );
}

Widget kimSystemMessage(
  BuildContext context,
  SystemMessage message,
  int index, {
  required bool isSentByMe,
  MessageGroupStatus? groupStatus,
}) {
  return Text(
    message.text,
    textAlign: TextAlign.center,
    style: Theme.of(context).textTheme.labelSmall?.copyWith(
      color: Theme.of(context).colorScheme.onSurfaceVariant,
      height: 1.4,
    ),
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
  String? displayName,
  VoidCallback? onRetry,
  DateTime? previousCreatedAt,
  void Function(LongPressStartDetails details)? onLongPress,
}) {
  return KimMessageRow(
    message: message,
    index: index,
    animation: animation,
    isSentByMe: isSentByMe,
    groupStatus: groupStatus,
    displayName: displayName,
    onRetry: onRetry,
    previousCreatedAt: previousCreatedAt,
    onLongPress: onLongPress,
    child: child,
  );
}

class KimMessageRow extends StatelessWidget {
  const KimMessageRow({
    super.key,
    required this.message,
    required this.index,
    required this.animation,
    required this.isSentByMe,
    required this.child,
    this.groupStatus,
    this.displayName,
    this.onRetry,
    this.previousCreatedAt,
    this.onLongPress,
  });

  final Message message;
  final int index;
  final Animation<double> animation;
  final bool isSentByMe;
  final MessageGroupStatus? groupStatus;
  final String? displayName;
  final VoidCallback? onRetry;
  final DateTime? previousCreatedAt;
  final void Function(LongPressStartDetails details)? onLongPress;
  final Widget child;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final scheme = theme.colorScheme;
    final curved = CurvedAnimation(parent: animation, curve: KimMotion.enter);
    if (message is SystemMessage) {
      return FadeTransition(
        opacity: curved,
        child: Padding(
          padding: const EdgeInsets.fromLTRB(24, 12, 24, 8),
          child: Center(child: child),
        ),
      );
    }

    final first = groupStatus?.isFirst ?? true;
    final name = displayName ?? (isSentByMe ? Copy.you : message.authorId);
    final divider = _dateDivider();
    final failed = message.status == MessageStatus.error;

    return FadeTransition(
      opacity: curved,
      child: SizeTransition(
        sizeFactor: curved,
        alignment: Alignment.topCenter,
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            if (divider != null) _DateRule(label: divider),
            GestureDetector(
              onLongPressStart: onLongPress,
              behavior: HitTestBehavior.opaque,
              child: Padding(
                padding: EdgeInsets.fromLTRB(12, first ? 10 : 2, 36, 2),
                child: Row(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    first
                        ? KimAvatar(name: name, size: KimAvatarSize.sm)
                        : const SizedBox(width: 36),
                    const Gap(10),
                    Expanded(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          if (first)
                            Padding(
                              padding: const EdgeInsets.only(bottom: 2),
                              child: Row(
                                children: [
                                  Flexible(
                                    child: Text(
                                      name,
                                      maxLines: 1,
                                      overflow: TextOverflow.ellipsis,
                                      style: theme.textTheme.labelLarge
                                          ?.copyWith(
                                            fontWeight: FontWeight.w600,
                                            color: isSentByMe
                                                ? scheme.primary
                                                : scheme.onSurface,
                                          ),
                                    ),
                                  ),
                                  const Gap(8),
                                  Text(
                                    formatMessageStampAt(message.createdAt),
                                    style: theme.textTheme.labelSmall?.copyWith(
                                      color: scheme.onSurfaceVariant,
                                    ),
                                  ),
                                ],
                              ),
                            ),
                          child,
                          if (failed)
                            TextButton(
                              key: Key('retry-${message.id}'),
                              onPressed: onRetry,
                              style: TextButton.styleFrom(
                                visualDensity: VisualDensity.compact,
                                foregroundColor: scheme.error,
                                padding: EdgeInsets.zero,
                              ),
                              child: Text(Copy.retry),
                            ),
                        ],
                      ),
                    ),
                  ],
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }

  String? _dateDivider() {
    final created = message.createdAt;
    if (created == null) {
      return null;
    }
    final previous = previousCreatedAt;
    if (previous != null &&
        sameCalendarDay(previous.toLocal(), created.toLocal())) {
      return null;
    }
    return formatDateDivider(created.millisecondsSinceEpoch);
  }
}

class _DateRule extends StatelessWidget {
  const _DateRule({required this.label});

  final String label;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Padding(
      padding: const EdgeInsets.fromLTRB(16, 16, 16, 8),
      child: Row(
        children: [
          const Expanded(child: KimHairline()),
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 10),
            child: Text(
              label,
              style: theme.textTheme.labelSmall?.copyWith(
                color: theme.colorScheme.onSurfaceVariant,
              ),
            ),
          ),
          const Expanded(child: KimHairline()),
        ],
      ),
    );
  }
}

class KimComposer extends StatelessWidget {
  const KimComposer({super.key, this.hintText});

  final String? hintText;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final scheme = theme.colorScheme;
    final raised = KimTheme.raisedOf(context);
    return Composer(
      hintText: hintText,
      filled: true,
      handleSafeArea: true,
      sigmaX: 0,
      sigmaY: 0,
      sendIcon: Icon(LucideIcons.send, size: 18),
      sendButtonVisibilityMode: SendButtonVisibilityMode.hidden,
      sendOnEnter: true,
      gap: 8,
      padding: const EdgeInsets.fromLTRB(12, 10, 12, 10),
      backgroundColor: raised,
      inputFillColor: scheme.surface,
      topWidget: const KimHairline(),
      inputBorder: OutlineInputBorder(
        borderRadius: BorderRadius.circular(KimTheme.radiusField),
        borderSide: BorderSide(color: KimTheme.hairlineOf(context)),
      ),
      sendIconColor: scheme.primary,
      emptyFieldSendIconColor: scheme.onSurfaceVariant,
    );
  }
}
