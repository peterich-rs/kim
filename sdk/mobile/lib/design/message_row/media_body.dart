part of '../kim_bubble.dart';

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
    final src = message.displaySrc;
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
