part of '../kim_bubble.dart';

class _PeerBlock extends StatelessWidget {
  const _PeerBlock({
    required this.message,
    required this.first,
    required this.last,
    required this.displayName,
    required this.avatarUrl,
    this.onRetry,
    this.onAvatarTap,
  });

  final KimChatMsg message;
  final bool first;
  final bool last;
  final String displayName;
  final String avatarUrl;
  final VoidCallback? onRetry;
  final VoidCallback? onAvatarTap;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final scheme = theme.colorScheme;
    return Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        first
            ? GestureDetector(
                onTap: onAvatarTap,
                child: KimAvatar(
                  name: displayName,
                  url: avatarUrl,
                  size: KimAvatarSize.sm,
                  shape: KimAvatarShape.squircle,
                ),
              )
            : const SizedBox(width: 36),
        const Gap(10),
        Flexible(
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
                          displayName,
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          style: theme.textTheme.labelLarge?.copyWith(
                            fontWeight: FontWeight.w600,
                            fontSize: KimTheme.fontMeta,
                            color: scheme.onSurface,
                          ),
                        ),
                      ),
                      const Gap(8),
                      Text(
                        formatClock(message.at),
                        style: theme.textTheme.labelSmall?.copyWith(
                          color: scheme.onSurfaceVariant,
                          fontSize: KimTheme.fontMeta,
                        ),
                      ),
                    ],
                  ),
                ),
              Row(
                crossAxisAlignment: CrossAxisAlignment.end,
                children: [
                  Flexible(
                    child: _Bubble(message: message, own: false, last: last),
                  ),
                  if (message.isSending || message.isFailed) ...[
                    const Gap(6),
                    _PeerSendState(message: message, onRetry: onRetry),
                  ],
                ],
              ),
              if (message.isFailed)
                _Retry(messageKey: message.key, onRetry: onRetry),
            ],
          ),
        ),
      ],
    );
  }
}

class _OwnBlock extends StatelessWidget {
  const _OwnBlock({
    required this.message,
    required this.first,
    required this.last,
    this.showRead = false,
    this.onRetry,
  });

  final KimChatMsg message;
  final bool first;
  final bool last;
  final bool showRead;
  final VoidCallback? onRetry;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.end,
      children: [
        if (first)
          Padding(
            padding: const EdgeInsets.only(bottom: 2, right: 4),
            child: Text(
              formatClock(message.at),
              style: theme.textTheme.labelSmall?.copyWith(
                color: theme.colorScheme.onSurfaceVariant,
                fontSize: KimTheme.fontMeta,
              ),
            ),
          ),
        Row(
          mainAxisAlignment: MainAxisAlignment.end,
          crossAxisAlignment: CrossAxisAlignment.end,
          children: [
            _SendState(message: message, onRetry: onRetry),
            const Gap(6),
            Flexible(
              child: _Bubble(message: message, own: true, last: last),
            ),
          ],
        ),
        if (message.isFailed) _Retry(messageKey: message.key, onRetry: onRetry),
        if (showRead && last && !message.isFailed)
          Padding(
            padding: const EdgeInsets.only(top: 2, right: 4),
            child: Text(
              Copy.readReceipt,
              style: theme.textTheme.labelSmall?.copyWith(
                color: theme.colorScheme.onSurfaceVariant,
                fontSize: KimTheme.fontMeta,
              ),
            ),
          ),
      ],
    );
  }
}
