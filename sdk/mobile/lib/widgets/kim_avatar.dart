library;

import 'package:flutter/material.dart';

import '../core/format.dart';
import 'kim_network_image.dart';

enum KimAvatarSize { sm, md, lg }

/// Avatar silhouette. Default [circle] keeps chat bubble rows unchanged;
/// conversation list uses [squircle].
enum KimAvatarShape { circle, squircle }

class KimAvatar extends StatelessWidget {
  const KimAvatar({
    super.key,
    required this.name,
    this.url = '',
    this.size = KimAvatarSize.md,
    this.shape = KimAvatarShape.circle,
  });

  final String name;
  final String url;
  final KimAvatarSize size;
  final KimAvatarShape shape;

  double get _px => switch (size) {
    KimAvatarSize.sm => 36,
    KimAvatarSize.md => 48,
    KimAvatarSize.lg => 72,
  };

  double get _font => switch (size) {
    KimAvatarSize.sm => 14,
    KimAvatarSize.md => 18,
    KimAvatarSize.lg => 28,
  };

  @override
  Widget build(BuildContext context) {
    final base = Color(avatarColor(name));
    final initials = Text(
      initialOf(name),
      style: TextStyle(
        color: Colors.white,
        fontSize: _font,
        fontWeight: FontWeight.w600,
        height: 1,
      ),
    );
    final shapeDecoration = switch (shape) {
      KimAvatarShape.circle => const BoxDecoration(shape: BoxShape.circle),
      KimAvatarShape.squircle => BoxDecoration(
        borderRadius: BorderRadius.circular(_px * 0.32),
      ),
    };
    final fallback = Container(
      width: _px,
      height: _px,
      alignment: Alignment.center,
      decoration: shapeDecoration.copyWith(
        gradient: LinearGradient(
          begin: Alignment.topCenter,
          end: Alignment.bottomCenter,
          colors: [Color.lerp(base, Colors.white, 0.16) ?? base, base],
        ),
      ),
      child: initials,
    );
    final Widget avatar;
    if (url.isEmpty) {
      avatar = fallback;
    } else {
      final dpr = MediaQuery.devicePixelRatioOf(context);
      final image = KimNetworkImage(
        src: url,
        width: _px,
        height: _px,
        fit: BoxFit.cover,
        memCacheWidth: (_px * dpr).round(),
        placeholder: fallback,
      );
      avatar = switch (shape) {
        KimAvatarShape.circle => ClipOval(child: image),
        KimAvatarShape.squircle => ClipRRect(
          borderRadius: BorderRadius.circular(_px * 0.32),
          child: image,
        ),
      };
    }
    return avatar;
  }
}
