library;

import 'package:flutter/material.dart';

import '../theme/kim_theme.dart';

class KimHairline extends StatelessWidget {
  const KimHairline({super.key, this.indent = 0});

  final double indent;

  @override
  Widget build(BuildContext context) {
    return Divider(
      height: 0.5,
      thickness: 0.5,
      indent: indent,
      color: KimTheme.hairlineOf(context),
    );
  }
}
