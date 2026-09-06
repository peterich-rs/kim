library;

import 'dart:math' as math;

import 'package:flutter/material.dart';

import '../theme/kim_theme.dart';

/// Equalizer-style typing indicator: 3–5 rounded vertical bars with staggered
/// height animation. Theme-aware; no third-party waveform packages.
class KimTypingBars extends StatefulWidget {
  const KimTypingBars({
    super.key,
    this.barCount = 4,
    this.height = 18,
    this.color,
  });

  final int barCount;
  final double height;
  final Color? color;

  @override
  State<KimTypingBars> createState() => _KimTypingBarsState();
}

class _KimTypingBarsState extends State<KimTypingBars>
    with SingleTickerProviderStateMixin {
  late final AnimationController _ctrl;

  @override
  void initState() {
    super.initState();
    _ctrl = AnimationController(
      vsync: this,
      duration: const Duration(milliseconds: 900),
    )..repeat();
  }

  @override
  void dispose() {
    _ctrl.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    final color =
        widget.color ?? scheme.onSurfaceVariant.withValues(alpha: 0.75);
    final n = widget.barCount.clamp(3, 5);
    return AnimatedBuilder(
      animation: _ctrl,
      builder: (context, _) {
        return SizedBox(
          height: widget.height,
          child: Row(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.end,
            children: [
              for (var i = 0; i < n; i++) ...[
                if (i > 0) const SizedBox(width: 3),
                _Bar(
                  progress: _ctrl.value,
                  phase: i / n,
                  maxHeight: widget.height,
                  color: color,
                ),
              ],
            ],
          ),
        );
      },
    );
  }
}

class _Bar extends StatelessWidget {
  const _Bar({
    required this.progress,
    required this.phase,
    required this.maxHeight,
    required this.color,
  });

  final double progress;
  final double phase;
  final double maxHeight;
  final Color color;

  @override
  Widget build(BuildContext context) {
    final t = (progress + phase) % 1.0;
    // Smooth equalizer wave between 35% and 100% height.
    final wave = 0.35 + 0.65 * (0.5 + 0.5 * math.sin(t * math.pi * 2));
    final h = maxHeight * wave;
    return Container(
      width: 3.5,
      height: h,
      decoration: BoxDecoration(
        color: color,
        borderRadius: BorderRadius.circular(999),
      ),
    );
  }
}

/// Peer-side typing row for the chat list (avatar + bars).
class KimTypingRow extends StatelessWidget {
  const KimTypingRow({super.key, required this.avatar, required this.name});

  final Widget avatar;
  final String name;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.fromLTRB(12, 8, 12, 4),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.end,
        children: [
          avatar,
          const SizedBox(width: 10),
          Container(
            padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
            decoration: BoxDecoration(
              color: Theme.of(context).colorScheme.surfaceContainer,
              borderRadius: BorderRadius.circular(KimTheme.radiusBubble),
            ),
            child: const KimTypingBars(barCount: 4, height: 16),
          ),
        ],
      ),
    );
  }
}
