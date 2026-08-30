library;

import 'package:flutter/material.dart';

import '../theme/motion.dart';

class KimLogLine {
  KimLogLine(this.text) : id = _nextId++;

  static int _nextId = 0;

  final int id;
  final String text;
}

/// Selectable log. New lines fade in once; old lines stay put.
class KimLogView extends StatelessWidget {
  const KimLogView({super.key, required this.lines});

  final List<KimLogLine> lines;

  @override
  Widget build(BuildContext context) {
    final style = Theme.of(context).textTheme.bodySmall
        ?.copyWith(fontFamily: 'monospace', height: 1.35);
    if (lines.isEmpty) {
      return Text(
        'no log yet',
        style: style?.copyWith(color: Theme.of(context).colorScheme.outline),
      );
    }
    return SelectionArea(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          for (final line in lines)
            TweenAnimationBuilder<double>(
              key: ValueKey(line.id),
              tween: Tween(begin: 0, end: 1),
              duration: KimMotion.medium,
              curve: KimMotion.enter,
              builder: (context, value, child) =>
                  Opacity(opacity: value, child: child),
              child: Text(line.text, style: style),
            ),
        ],
      ),
    );
  }
}
