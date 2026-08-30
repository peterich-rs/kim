/// Thin HapticFeedback wrappers for connect / talk success and error.
library;

import 'package:flutter/services.dart';

abstract final class KimHaptics {
  static Future<void> success() => HapticFeedback.mediumImpact();

  static Future<void> error() => HapticFeedback.heavyImpact();

  static Future<void> light() => HapticFeedback.lightImpact();

  static Future<void> selection() => HapticFeedback.selectionClick();
}
