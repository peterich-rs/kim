/// Product color/type/radius tokens as a [ThemeExtension].
library;

import 'package:flutter/material.dart';

@immutable
class KimTokens extends ThemeExtension<KimTokens> {
  const KimTokens({
    required this.outgoing,
    required this.bubbleOwnStart,
    required this.bubbleOwnEnd,
    required this.chatCanvas,
    required this.canvas,
    required this.chrome,
    required this.raised,
    required this.hairline,
    required this.frostFill,
    required this.fontTitle,
    required this.fontBody,
    required this.fontMeta,
    required this.radiusControl,
    required this.radiusField,
    required this.radiusBubble,
    required this.radiusBubbleTail,
    required this.radiusCard,
    required this.radiusSheet,
    required this.spaceUnit,
  });

  final Color outgoing;
  final Color bubbleOwnStart;
  final Color bubbleOwnEnd;
  final Color chatCanvas;
  final Color canvas;
  final Color chrome;
  final Color raised;
  final Color hairline;
  final Color frostFill;
  final double fontTitle;
  final double fontBody;
  final double fontMeta;
  final double radiusControl;
  final double radiusField;
  final double radiusBubble;
  final double radiusBubbleTail;
  final double radiusCard;
  final double radiusSheet;
  final double spaceUnit;

  static const Color seed = Color(0xFF0F766E);

  static KimTokens of(BuildContext context) {
    return Theme.of(context).extension<KimTokens>() ??
        fromScheme(Theme.of(context).colorScheme, Theme.of(context).brightness);
  }

  static KimTokens fromScheme(ColorScheme scheme, Brightness brightness) {
    final dark = brightness == Brightness.dark;
    return KimTokens(
      outgoing: const Color(0xFF0D9488),
      bubbleOwnStart: const Color(0xFF14B8A6),
      bubbleOwnEnd: const Color(0xFF0D9488),
      chatCanvas: dark ? const Color(0xFF0E1621) : const Color(0xFFF2F5F8),
      canvas: scheme.surfaceContainerLowest,
      chrome: scheme.surface,
      raised: scheme.surfaceContainerLow,
      hairline: scheme.outlineVariant.withValues(alpha: 0.72),
      frostFill: (dark ? const Color(0xFF000000) : const Color(0xFFFFFFFF))
          .withValues(alpha: dark ? 0.45 : 0.65),
      fontTitle: 17,
      fontBody: 15.5,
      fontMeta: 12.5,
      radiusControl: 8,
      radiusField: 12,
      radiusBubble: 14,
      radiusBubbleTail: 6,
      radiusCard: 16,
      radiusSheet: 24,
      spaceUnit: 4,
    );
  }

  @override
  KimTokens copyWith({
    Color? outgoing,
    Color? chatCanvas,
    Color? canvas,
    Color? chrome,
    Color? raised,
    Color? hairline,
  }) {
    return KimTokens(
      outgoing: outgoing ?? this.outgoing,
      bubbleOwnStart: bubbleOwnStart,
      bubbleOwnEnd: bubbleOwnEnd,
      chatCanvas: chatCanvas ?? this.chatCanvas,
      canvas: canvas ?? this.canvas,
      chrome: chrome ?? this.chrome,
      raised: raised ?? this.raised,
      hairline: hairline ?? this.hairline,
      frostFill: frostFill,
      fontTitle: fontTitle,
      fontBody: fontBody,
      fontMeta: fontMeta,
      radiusControl: radiusControl,
      radiusField: radiusField,
      radiusBubble: radiusBubble,
      radiusBubbleTail: radiusBubbleTail,
      radiusCard: radiusCard,
      radiusSheet: radiusSheet,
      spaceUnit: spaceUnit,
    );
  }

  @override
  KimTokens lerp(ThemeExtension<KimTokens>? other, double t) {
    if (other is! KimTokens) {
      return this;
    }
    return KimTokens(
      outgoing: Color.lerp(outgoing, other.outgoing, t) ?? outgoing,
      bubbleOwnStart:
          Color.lerp(bubbleOwnStart, other.bubbleOwnStart, t) ?? bubbleOwnStart,
      bubbleOwnEnd:
          Color.lerp(bubbleOwnEnd, other.bubbleOwnEnd, t) ?? bubbleOwnEnd,
      chatCanvas: Color.lerp(chatCanvas, other.chatCanvas, t) ?? chatCanvas,
      canvas: Color.lerp(canvas, other.canvas, t) ?? canvas,
      chrome: Color.lerp(chrome, other.chrome, t) ?? chrome,
      raised: Color.lerp(raised, other.raised, t) ?? raised,
      hairline: Color.lerp(hairline, other.hairline, t) ?? hairline,
      frostFill: Color.lerp(frostFill, other.frostFill, t) ?? frostFill,
      fontTitle: fontTitle,
      fontBody: fontBody,
      fontMeta: fontMeta,
      radiusControl: radiusControl,
      radiusField: radiusField,
      radiusBubble: radiusBubble,
      radiusBubbleTail: radiusBubbleTail,
      radiusCard: radiusCard,
      radiusSheet: radiusSheet,
      spaceUnit: spaceUnit,
    );
  }
}
