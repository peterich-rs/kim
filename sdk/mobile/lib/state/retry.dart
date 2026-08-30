library;

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_riverpod/misc.dart';

import '../core/errors.dart';

/// Retry transient provider failures; never retry auth / validation errors.
///
/// Applied on [ProviderScope.retry]. `build()` errors that are [Error]s or
/// already-wrapped [ProviderException]s follow Riverpod's default skip.
Duration? kimRetry(int retryCount, Object error) {
  final inner = switch (error) {
    ProviderException(:final exception) => exception,
    _ => error,
  };
  if (isPermanentClientError(inner)) {
    return null;
  }
  return ProviderContainer.defaultRetry(retryCount, inner);
}
