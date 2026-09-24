library;

import 'package:flutter/foundation.dart';

abstract final class KimLogger {
  static void info(String message) {
    debugPrint(_line('info', message, null));
  }

  static void error(String message, [Object? error, StackTrace? stack]) {
    debugPrint(_line('error', message, error));
    if (stack != null) {
      debugPrint('$stack');
    }
  }

  static void warn(String message, [Object? error, StackTrace? stack]) {
    debugPrint(_line('warn', message, error));
    if (stack != null) {
      debugPrint('$stack');
    }
  }

  static String _line(String level, String message, Object? error) {
    if (error == null) {
      return '[kim] $level $message';
    }
    return '[kim] $level $message: $error';
  }
}
