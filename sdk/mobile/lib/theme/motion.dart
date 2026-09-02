/// Motion tokens for the KIM shell. Built-in implicit animations + `animations`.
/// Not a SwiftUI spring clone.
library;

import 'package:flutter/animation.dart';

abstract final class KimMotion {
  static const Duration short = Duration(milliseconds: 180);
  static const Duration medium = Duration(milliseconds: 260);
  static const Duration long = Duration(milliseconds: 400);

  static const Curve standard = Curves.easeOutCubic;
  static const Curve enter = Curves.easeOutCubic;
  static const Curve exit = Curves.easeInCubic;
}
