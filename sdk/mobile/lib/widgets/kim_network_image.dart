/// Disk + memory cached remote images. Local file paths skip the network.
library;

import 'dart:io';

import 'package:cached_network_image/cached_network_image.dart';
import 'package:flutter/material.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import '../core/image_extra.dart';

ImageProvider kimImageProvider(String src) {
  if (isRemoteUrl(src)) {
    return CachedNetworkImageProvider(src);
  }
  return FileImage(File(src));
}

class KimNetworkImage extends StatelessWidget {
  const KimNetworkImage({
    super.key,
    required this.src,
    this.width,
    this.height,
    this.fit = BoxFit.cover,
    this.placeholder,
    this.memCacheWidth,
    this.memCacheHeight,
  });

  final String src;
  final double? width;
  final double? height;
  final BoxFit fit;
  final Widget? placeholder;
  final int? memCacheWidth;
  final int? memCacheHeight;

  @override
  Widget build(BuildContext context) {
    final fallback =
        placeholder ??
        ColoredBox(
          color: Theme.of(context).colorScheme.surfaceContainerLow,
          child: const Center(child: Icon(LucideIcons.image, size: 28)),
        );
    if (src.isEmpty) {
      return SizedBox(width: width, height: height, child: fallback);
    }
    if (!isRemoteUrl(src)) {
      return Image.file(
        File(src),
        width: width,
        height: height,
        fit: fit,
        cacheWidth: memCacheWidth,
        cacheHeight: memCacheHeight,
        errorBuilder: (_, _, _) => fallback,
      );
    }
    return CachedNetworkImage(
      imageUrl: src,
      width: width,
      height: height,
      fit: fit,
      memCacheWidth: memCacheWidth,
      memCacheHeight: memCacheHeight,
      fadeInDuration: const Duration(milliseconds: 120),
      placeholder: (_, _) => fallback,
      errorWidget: (_, _, _) => fallback,
    );
  }
}
