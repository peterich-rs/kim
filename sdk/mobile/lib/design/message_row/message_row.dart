part of '../kim_bubble.dart';

class KimMessageRow extends StatelessWidget {
  const KimMessageRow({
    super.key,
    required this.message,
    required this.isSentByMe,
    this.previous,
    this.next,
    this.displayName,
    this.avatarUrl = '',
    this.unreadAnchor = false,
    this.showRead = false,
    this.onRetry,
    this.onLongPress,
    this.onAvatarTap,
  });

  final KimChatMsg message;
  final bool isSentByMe;
  final KimChatMsg? previous;
  final KimChatMsg? next;
  final String? displayName;
  final String avatarUrl;
  final bool unreadAnchor;

  /// DM peer has read up to/through this own message.
  final bool showRead;
  final VoidCallback? onRetry;
  final void Function(LongPressStartDetails details)? onLongPress;
  final VoidCallback? onAvatarTap;

  @override
  Widget build(BuildContext context) {
    if (message.isAgentCard) {
      final card = message.card;
      if (card == null || !card.actionRequired) {
        return const SizedBox.shrink();
      }
      return AgentActionBubble(
        dest: message.dest,
        callId: card.callId,
        name: card.name,
        preview: card.preview,
        pending: card.pending,
        confirmation: card.actionRequired,
      );
    }
    if (message.sys) {
      return Padding(
        padding: const EdgeInsets.fromLTRB(24, 12, 24, 8),
        child: Center(
          child: _MessageText(
            message.body,
            textAlign: TextAlign.center,
            style: Theme.of(context).textTheme.labelSmall?.copyWith(
              color: Theme.of(context).colorScheme.onSurfaceVariant,
              height: 1.4,
            ),
          ),
        ),
      );
    }

    final first = kimIsGroupStart(message, previous);
    final last = kimIsGroupEnd(message, next);
    final tight = kimSameBatch(message, previous);
    final divider = _dateDivider();
    final topGap = tight
        ? 1.0
        : first
        ? KimTheme.spaceUnit * 2.5
        : 2.0;
    final selectable = kimSelectsMessageText(Theme.of(context).platform);

    Widget body = Padding(
      padding: EdgeInsets.fromLTRB(12, topGap, 12, last ? 4 : 0),
      child: isSentByMe
          ? _OwnBlock(
              message: message,
              first: first,
              last: last,
              showRead: showRead,
              onRetry: onRetry,
            )
          : _PeerBlock(
              message: message,
              first: first,
              last: last,
              displayName: displayName ?? message.sender,
              avatarUrl: avatarUrl,
              onRetry: onRetry,
              onAvatarTap: onAvatarTap,
            ),
    );
    // Phone: opaque long-press opens copy/quote. Desktop: that recognizer
    // eats mouse drags, so selection lives on the text instead.
    if (onLongPress != null && !selectable) {
      body = GestureDetector(
        onLongPressStart: onLongPress,
        behavior: HitTestBehavior.opaque,
        child: body,
      );
    }

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        if (divider != null) _DateRule(label: divider),
        if (unreadAnchor) const _UnreadRule(),
        body,
      ],
    );
  }

  String? _dateDivider() {
    final created = dateTimeFromEpoch(message.at);
    if (created == null) {
      return null;
    }
    final prev = previous;
    if (prev != null) {
      final previousAt = dateTimeFromEpoch(prev.at);
      if (previousAt != null &&
          sameCalendarDay(previousAt.toLocal(), created.toLocal())) {
        return null;
      }
    }
    return formatDateDivider(message.at);
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
                fontSize: KimTheme.fontMeta,
              ),
            ),
          ),
          const Expanded(child: KimHairline()),
        ],
      ),
    );
  }
}

class _UnreadRule extends StatelessWidget {
  const _UnreadRule();

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Padding(
      padding: const EdgeInsets.fromLTRB(16, 12, 16, 8),
      child: Row(
        children: [
          const Expanded(child: KimHairline()),
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 10),
            child: Text(
              Copy.unreadBelow,
              key: const Key('unread-divider'),
              style: theme.textTheme.labelSmall?.copyWith(
                color: theme.colorScheme.primary,
                fontSize: KimTheme.fontMeta,
                fontWeight: FontWeight.w600,
              ),
            ),
          ),
          const Expanded(child: KimHairline()),
        ],
      ),
    );
  }
}
