library;

import 'package:flutter/cupertino.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';

/// Push page with a follow-through back gesture.
///
/// Apple: [CupertinoPage] (edge swipe + parallax). Android: [MaterialPage]
/// plus [PredictiveBackPageTransitionsBuilder] from [KimTheme].
Page<void> kimPushPage({
  required LocalKey key,
  required Widget child,
  String? name,
  Object? arguments,
}) {
  switch (defaultTargetPlatform) {
    case TargetPlatform.iOS:
    case TargetPlatform.macOS:
      return CupertinoPage<void>(
        key: key,
        name: name,
        arguments: arguments,
        child: child,
      );
    default:
      return MaterialPage<void>(
        key: key,
        name: name,
        arguments: arguments,
        child: child,
      );
  }
}
