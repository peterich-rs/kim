/// Compile-time environment. `--dart-define=KIM_ENV=dev|staging|prod`.
library;

import 'package:flutter/foundation.dart';

enum KimEnv { dev, staging, prod }

KimEnv get kimEnv {
  const raw = String.fromEnvironment('KIM_ENV', defaultValue: 'prod');
  return switch (raw) {
    'dev' => KimEnv.dev,
    'staging' => KimEnv.staging,
    _ => KimEnv.prod,
  };
}

/// DevPanel is compiled out of production release builds.
bool get kimDevPanelEnabled => kDebugMode || kimEnv != KimEnv.prod;
