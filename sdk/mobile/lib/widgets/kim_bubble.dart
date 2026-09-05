/// v2 message row: Discord grouping, Telegram own-bubble, send-state icons.
library;

import 'package:flutter/material.dart';
import 'package:gap/gap.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import '../copy.dart';
import '../core/format.dart';
import '../models/models.dart';
import '../theme/kim_theme.dart';
import 'kim_avatar.dart';
import 'kim_hairline.dart';
import 'kim_image_viewer.dart';
import 'kim_network_image.dart';

const _groupWindow = Duration(minutes: 5);

bool kimIsGroupStart(KimChatMsg msg, KimChatMsg? previous) {
  return !_sameGroup(previous, msg);
}

bool kimIsGroupEnd(KimChatMsg msg, KimChatMsg? next) {
  return !_sameGroup(msg, next);
}

/// Consecutive grouping is millisecond-based on every platform. Wire
/// `sendTime` may be ns/µs/s; [dateTimeFromEpoch] normalizes first.
bool _sameGroup(KimChatMsg? earlier, KimChatMsg? later) {
  if (earlier == null || later == null || earlier.sys || later.sys) {
    return false;
  }
  if (earlier.sender != later.sender) {
    return false;
  }
  final a = dateTimeFromEpoch(earlier.at);
  final b = dateTimeFromEpoch(later.at);
  if (a == null || b == null) {
    return false;
  }
  final localA = a.toLocal();
  final localB = b.toLocal();
  if (!sameCalendarDay(localA, localB)) {
    return false;
  }
  return b.difference(a).abs() <= _groupWindow;
}

bool kimSameBatch(KimChatMsg msg, KimChatMsg? previous) {
  final id = msg.batchId;
  return id != null && id.isNotEmpty && previous?.batchId == id;
}

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
    this.onRetry,
    this.onLongPress,
  });

  final KimChatMsg message;
  final bool isSentByMe;
  final KimChatMsg? previous;
  final KimChatMsg? next;
  final String? displayName;
  final String avatarUrl;
  final bool unreadAnchor;
  final VoidCallback? onRetry;
  final void Function(LongPressStartDetails details)? onLongPress;

  @override
  Widget build(BuildContext context) {
    if (message.sys) {
      return Padding(
        padding: const EdgeInsets.fromLTRB(24, 12, 24, 8),
        child: Center(
          child: Text(
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

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        if (divider != null) _DateRule(label: divider),
        if (unreadAnchor) const _UnreadRule(),
        GestureDetector(
          onLongPressStart: onLongPress,
          behavior: HitTestBehavior.opaque,
          child: Padding(
            padding: EdgeInsets.fromLTRB(12, topGap, 12, last ? 4 : 0),
            child: isSentByMe
                ? _OwnBlock(
                    message: message,
                    first: first,
                    last: last,
                    onRetry: onRetry,
                  )
                : _PeerBlock(
                    message: message,
                    first: first,
                    last: last,
                    displayName: displayName ?? message.sender,
                    avatarUrl: avatarUrl,
                    onRetry: onRetry,
                  ),
          ),
        ),
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

class _PeerBlock extends StatelessWidget {
  const _PeerBlock({
    required this.message,
    required this.first,
    required this.last,
    required this.displayName,
    required this.avatarUrl,
    this.onRetry,
  });

  final KimChatMsg message;
  final bool first;
  final bool last;
  final String displayName;
  final String avatarUrl;
  final VoidCallback? onRetry;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final scheme = theme.colorScheme;
    return Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        first
            ? KimAvatar(
                name: displayName,
                url: avatarUrl,
                size: KimAvatarSize.sm,
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
              _Bubble(message: message, own: false, last: last),
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
    this.onRetry,
  });

  final KimChatMsg message;
  final bool first;
  final bool last;
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
      ],
    );
  }
}

class _Bubble extends StatelessWidget {
  const _Bubble({required this.message, required this.own, required this.last});

  final KimChatMsg message;
  final bool own;
  final bool last;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    final radius = BorderRadius.only(
      topLeft: const Radius.circular(KimTheme.radiusBubble),
      topRight: const Radius.circular(KimTheme.radiusBubble),
      bottomRight: Radius.circular(
        own && last ? KimTheme.radiusBubble : KimTheme.radiusBubble,
      ),
      bottomLeft: Radius.circular(
        own && last ? KimTheme.radiusBubbleTail : KimTheme.radiusBubble,
      ),
    );
    final media = message.isImage || message.isVideo;
    final child = media
        ? _MediaBody(message: message)
        : Padding(
            padding: const EdgeInsets.fromLTRB(12, 8, 12, 8),
            child: Text(
              message.body,
              style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                fontSize: KimTheme.fontBody,
                height: 1.35,
                color: own ? Colors.white : scheme.onSurface,
              ),
            ),
          );
    final bubble = DecoratedBox(
      decoration: BoxDecoration(
        gradient: own && !media
            ? const LinearGradient(
                begin: Alignment.topLeft,
                end: Alignment.bottomRight,
                colors: [KimTheme.bubbleOwnStart, KimTheme.bubbleOwnEnd],
              )
            : null,
        color: own ? null : scheme.surfaceContainer,
        borderRadius: radius,
      ),
      child: ClipRRect(borderRadius: radius, child: child),
    );
    if (!own || !last) {
      return bubble;
    }
    return Stack(
      clipBehavior: Clip.none,
      children: [
        bubble,
        Positioned(
          left: -1,
          bottom: 0,
          child: CustomPaint(
            size: const Size(8, 10),
            painter: _TailPainter(
              color: media ? scheme.surfaceContainer : KimTheme.bubbleOwnEnd,
            ),
          ),
        ),
      ],
    );
  }
}

class _TailPainter extends CustomPainter {
  const _TailPainter({required this.color});

  final Color color;

  @override
  void paint(Canvas canvas, Size size) {
    final path = Path()
      ..moveTo(size.width, 0)
      ..lineTo(0, size.height)
      ..lineTo(size.width, size.height)
      ..close();
    canvas.drawPath(path, Paint()..color = color);
  }

  @override
  bool shouldRepaint(covariant _TailPainter oldDelegate) =>
      oldDelegate.color != color;
}

class _MediaBody extends StatelessWidget {
  const _MediaBody({required this.message});

  final KimChatMsg message;

  @override
  Widget build(BuildContext context) {
    final w = (message.width > 0 ? message.width : 160).toDouble().clamp(
      48,
      240,
    );
    final h = (message.height > 0 ? message.height : 160).toDouble().clamp(
      48,
      320,
    );
    const maxW = 220.0;
    final height = (h * (maxW / w)).clamp(72, 280).toDouble();
    if (message.isVideo) {
      return SizedBox(
        width: maxW,
        height: height,
        child: const ColoredBox(
          color: Colors.black,
          child: Center(
            child: SizedBox(
              width: 44,
              height: 44,
              child: DecoratedBox(
                decoration: BoxDecoration(
                  color: Color(0x33FFFFFF),
                  shape: BoxShape.circle,
                ),
                child: Icon(LucideIcons.play, color: Colors.white, size: 22),
              ),
            ),
          ),
        ),
      );
    }
    final src = message.body;
    final dpr = MediaQuery.devicePixelRatioOf(context);
    final cacheW = (maxW * dpr).round();
    final tag = 'img-${message.key}';
    return GestureDetector(
      onTap: () => showKimImageViewer(context, src: src, heroTag: tag),
      child: Hero(
        tag: tag,
        child: SizedBox(
          width: maxW,
          height: height,
          child: KimNetworkImage(
            src: src,
            width: maxW,
            height: height,
            fit: BoxFit.cover,
            memCacheWidth: cacheW,
            placeholder: ColoredBox(
              color: Theme.of(context).colorScheme.surfaceContainerLow,
              child: const Center(child: Icon(LucideIcons.image, size: 28)),
            ),
          ),
        ),
      ),
    );
  }
}

class _SendState extends StatelessWidget {
  const _SendState({required this.message, this.onRetry});

  final KimChatMsg message;
  final VoidCallback? onRetry;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    if (message.isFailed) {
      return IconButton(
        key: Key('retry-${message.key}'),
        tooltip: Copy.retry,
        onPressed: onRetry,
        visualDensity: VisualDensity.compact,
        padding: EdgeInsets.zero,
        constraints: const BoxConstraints(minWidth: 28, minHeight: 28),
        icon: Icon(LucideIcons.circleAlert, size: 16, color: scheme.error),
      );
    }
    if (message.isSending) {
      return Icon(LucideIcons.clock, size: 14, color: scheme.onSurfaceVariant);
    }
    return Icon(LucideIcons.check, size: 14, color: scheme.primary);
  }
}

class _Retry extends StatelessWidget {
  const _Retry({required this.messageKey, this.onRetry});

  final String messageKey;
  final VoidCallback? onRetry;

  @override
  Widget build(BuildContext context) {
    return TextButton(
      key: Key('retry-label-$messageKey'),
      onPressed: onRetry,
      style: TextButton.styleFrom(
        visualDensity: VisualDensity.compact,
        foregroundColor: Theme.of(context).colorScheme.error,
        padding: EdgeInsets.zero,
      ),
      child: const Text(Copy.retry),
    );
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
