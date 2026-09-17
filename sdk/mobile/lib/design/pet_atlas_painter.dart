library;

import 'dart:ui' as ui;

import 'package:flutter/material.dart';

import 'package:kim_mobile/design/kim_avatar.dart';

class PetAtlasPainter extends CustomPainter {
  PetAtlasPainter({
    required this.image,
    required this.src,
    required this.shape,
  });

  final ui.Image image;
  final Rect src;
  final KimAvatarShape shape;

  @override
  void paint(Canvas canvas, Size size) {
    final dst = Offset.zero & size;
    canvas.save();
    final rrect = switch (shape) {
      KimAvatarShape.circle => RRect.fromRectAndRadius(
        dst,
        Radius.circular(size.width / 2),
      ),
      KimAvatarShape.squircle => RRect.fromRectAndRadius(
        dst,
        Radius.circular(size.width * 0.32),
      ),
    };
    canvas.clipRRect(rrect);
    canvas.drawImageRect(
      image,
      src,
      dst,
      Paint()..filterQuality = FilterQuality.none,
    );
    canvas.restore();
  }

  @override
  bool shouldRepaint(PetAtlasPainter old) =>
      old.image != image || old.src != src || old.shape != shape;
}
