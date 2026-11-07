part of '../kim_bubble.dart';

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

class _PeerSendState extends StatelessWidget {
  const _PeerSendState({required this.message, this.onRetry});

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
      return _DelayedBusy(key: Key('send-busy-${message.key}'));
    }
    return const SizedBox.shrink();
  }
}

class _DelayedBusy extends StatefulWidget {
  const _DelayedBusy({super.key});

  @override
  State<_DelayedBusy> createState() => _DelayedBusyState();
}

class _DelayedBusyState extends State<_DelayedBusy> {
  Timer? _timer;
  var _show = false;

  @override
  void initState() {
    super.initState();
    _timer = Timer(kPeerSendBusyDelay, () {
      if (mounted) {
        setState(() => _show = true);
      }
    });
  }

  @override
  void dispose() {
    _timer?.cancel();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    if (!_show) {
      return const SizedBox(width: 14, height: 14);
    }
    return SizedBox(
      width: 14,
      height: 14,
      child: CircularProgressIndicator(
        strokeWidth: 2,
        color: Theme.of(context).colorScheme.onSurfaceVariant,
      ),
    );
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
      child: Text(Copy.retry),
    );
  }
}
