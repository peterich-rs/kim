/// Thin HapticFeedback wrappers for connect / talk success and error.
library;

import 'package:flutter/services.dart';

abstract final class KimHaptics {
  static Future<void> success() => _run(HapticFeedback.mediumImpact);

  static Future<void> error() => _run(HapticFeedback.heavyImpact);

  static Future<void> light() => _run(HapticFeedback.lightImpact);

  static Future<void> selection() => _run(HapticFeedback.selectionClick);

  /// Never block UI/tests on a vibrator that does not complete.
  static Future<void> _run(Future<void> Function() fn) {
    fn().ignore();
    return Future<void>.value();
  }
}
