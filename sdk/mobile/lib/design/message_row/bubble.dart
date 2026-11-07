part of '../kim_bubble.dart';

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
            child: _MessageText(
              message.body,
              style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                fontSize: KimTheme.fontBody,
                height: 1.35,
                color: own ? Colors.white : scheme.onSurface,
              ),
              selectionColor: own ? Colors.white.withValues(alpha: 0.35) : null,
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

class _MessageText extends StatelessWidget {
  const _MessageText(
    this.text, {
    this.style,
    this.textAlign,
    this.selectionColor,
  });

  final String text;
  final TextStyle? style;
  final TextAlign? textAlign;
  final Color? selectionColor;

  @override
  Widget build(BuildContext context) {
    if (!kimSelectsMessageText(Theme.of(context).platform)) {
      return Text(text, textAlign: textAlign, style: style);
    }
    return SelectableText(
      text,
      textAlign: textAlign,
      style: style,
      selectionColor: selectionColor,
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
