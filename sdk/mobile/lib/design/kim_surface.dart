/// Surfaces that read [KimTokens] instead of raw [Theme.of] colorScheme.
library;

import 'package:flutter/material.dart';

import 'package:kim_mobile/design/kim_tokens.dart';

class KimRaised extends StatelessWidget {
  const KimRaised({super.key, required this.child, this.border = true});

  final Widget child;
  final bool border;

  @override
  Widget build(BuildContext context) {
    final tokens = KimTokens.of(context);
    return Material(
      color: tokens.raised,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(tokens.radiusCard),
        side: border ? BorderSide(color: tokens.hairline) : BorderSide.none,
      ),
      clipBehavior: Clip.antiAlias,
      child: child,
    );
  }
}
