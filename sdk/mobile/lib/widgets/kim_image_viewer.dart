/// Full-screen image preview.
///
/// Zoom/pan/double-tap: `photo_view`. Swipe-to-dismiss + transparent
/// route: `dismissible_page`. Hero tag matches the chat thumbnail.
library;

import 'package:dismissible_page/dismissible_page.dart';
import 'package:flutter/material.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';
import 'package:photo_view/photo_view.dart';

import '../copy.dart';
import '../theme/motion.dart';
import 'kim_network_image.dart';

Future<void> showKimImageViewer(
  BuildContext context, {
  required String src,
  Object? heroTag,
}) {
  return context.pushTransparentRoute<void>(
    KimImageViewer(src: src, heroTag: heroTag),
    transitionDuration: KimMotion.medium,
    reverseTransitionDuration: KimMotion.medium,
  );
}

class KimImageViewer extends StatefulWidget {
  const KimImageViewer({super.key, required this.src, this.heroTag});

  final String src;
  final Object? heroTag;

  @override
  State<KimImageViewer> createState() => _KimImageViewerState();
}

class _KimImageViewerState extends State<KimImageViewer> {
  var _zoomed = false;

  void _onScale(PhotoViewScaleState state) {
    final next =
        state == PhotoViewScaleState.zoomedIn ||
        state == PhotoViewScaleState.covering ||
        state == PhotoViewScaleState.originalSize;
    if (next == _zoomed) {
      return;
    }
    setState(() => _zoomed = next);
  }

  @override
  Widget build(BuildContext context) {
    final tag = widget.heroTag ?? widget.src;
    return DismissiblePage(
      onDismissed: () => Navigator.of(context).pop(),
      disabled: _zoomed,
      direction: DismissiblePageDismissDirection.vertical,
      backgroundColor: Colors.black,
      child: Stack(
        fit: StackFit.expand,
        children: [
          PhotoView(
            imageProvider: kimImageProvider(widget.src),
            heroAttributes: PhotoViewHeroAttributes(
              tag: tag,
              transitionOnUserGestures: true,
            ),
            minScale: PhotoViewComputedScale.contained,
            maxScale: PhotoViewComputedScale.covered * 3,
            initialScale: PhotoViewComputedScale.contained,
            backgroundDecoration: const BoxDecoration(
              color: Colors.transparent,
            ),
            loadingBuilder: (_, _) => const Center(
              child: CircularProgressIndicator(color: Colors.white),
            ),
            errorBuilder: (_, _, _) => const Center(
              child: Icon(
                LucideIcons.imageOff,
                color: Colors.white54,
                size: 48,
              ),
            ),
            scaleStateChangedCallback: _onScale,
            semanticLabel: Copy.viewImage,
          ),
          SafeArea(
            child: Align(
              alignment: Alignment.topLeft,
              child: IconButton(
                icon: const Icon(LucideIcons.x, color: Colors.white),
                tooltip: Copy.closeViewer,
                onPressed: () => Navigator.of(context).pop(),
              ),
            ),
          ),
        ],
      ),
    );
  }
}
