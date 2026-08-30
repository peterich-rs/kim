library;

import 'package:flutter/material.dart';

import '../core/format.dart';

enum KimAvatarSize { sm, md, lg }

class KimAvatar extends StatelessWidget {
  const KimAvatar({
    super.key,
    required this.name,
    this.size = KimAvatarSize.md,
    this.heroTag,
  });

  final String name;
  final KimAvatarSize size;
  final String? heroTag;

  double get _px => switch (size) {
    KimAvatarSize.sm => 36,
    KimAvatarSize.md => 52,
    KimAvatarSize.lg => 72,
  };

  double get _font => switch (size) {
    KimAvatarSize.sm => 14,
    KimAvatarSize.md => 20,
    KimAvatarSize.lg => 28,
  };

  @override
  Widget build(BuildContext context) {
    final base = Color(avatarColor(name));
    final avatar = Container(
      width: _px,
      height: _px,
      alignment: Alignment.center,
      decoration: BoxDecoration(
        shape: BoxShape.circle,
        gradient: LinearGradient(
          begin: Alignment.topCenter,
          end: Alignment.bottomCenter,
          colors: [Color.lerp(base, Colors.white, 0.16) ?? base, base],
        ),
      ),
      child: Text(
        initialOf(name),
        style: TextStyle(
          color: Colors.white,
          fontSize: _font,
          fontWeight: FontWeight.w600,
          height: 1,
        ),
      ),
    );
    final tag = heroTag;
    if (tag == null) {
      return avatar;
    }
    return Hero(tag: tag, child: avatar);
  }
}
