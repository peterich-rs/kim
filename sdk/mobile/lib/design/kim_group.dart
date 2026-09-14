library;

import 'package:flutter/material.dart';

import 'package:kim_mobile/design/kim_surface.dart';

/// Raised, opaque group with a hairline. Replaces alpha-washed cards.
class KimGroupCard extends StatelessWidget {
  const KimGroupCard({super.key, required this.children});

  final List<Widget> children;

  @override
  Widget build(BuildContext context) {
    return KimRaised(child: Column(children: children));
  }
}
