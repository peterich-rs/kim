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
    final avatar = Container(
      width: _px,
      height: _px,
      alignment: Alignment.center,
      decoration: BoxDecoration(
        color: Color(avatarColor(name)),
        shape: BoxShape.circle,
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
