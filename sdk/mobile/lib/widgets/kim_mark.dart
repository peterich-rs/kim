library;

import 'package:flutter/material.dart';

import '../theme/kim_theme.dart';

class KimMark extends StatelessWidget {
  const KimMark({super.key, this.size = 56});

  final double size;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    return Container(
      width: size,
      height: size,
      alignment: Alignment.center,
      decoration: BoxDecoration(
        color: scheme.primary,
        borderRadius: BorderRadius.circular(KimTheme.radiusField),
      ),
      child: Text(
        'K',
        style: TextStyle(
          color: scheme.onPrimary,
          fontSize: size * 0.44,
          fontWeight: FontWeight.w700,
          height: 1,
        ),
      ),
    );
  }
}
