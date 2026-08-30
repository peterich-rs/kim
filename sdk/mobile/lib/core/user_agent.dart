library;

import 'package:flutter/foundation.dart';

import 'runtime.dart';

/// HTTP `User-Agent` passed into Rust. Browsers set this automatically; we
/// cannot rely on reqwest's default `reqwest/0.12`.
String kimUserAgent(KimRuntime runtime) {
  final os = switch (defaultTargetPlatform) {
    TargetPlatform.iOS => 'iOS',
    TargetPlatform.android => 'Android',
    TargetPlatform.macOS => 'macOS',
    TargetPlatform.windows => 'Windows',
    TargetPlatform.linux => 'Linux',
    TargetPlatform.fuchsia => 'Fuchsia',
  };
  return 'KIM/${runtime.version} ($os; build ${runtime.buildNumber})';
}
