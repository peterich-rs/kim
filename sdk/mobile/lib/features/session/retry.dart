library;

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_riverpod/misc.dart';

import 'package:kim_mobile/core/errors.dart';
import 'package:kim_mobile/core/failures.dart';

/// Retry transient provider failures; never retry auth / validation errors.
///
/// Applied on [ProviderScope.retry]. `build()` errors that are [Error]s or
/// already-wrapped [ProviderException]s follow Riverpod's default skip.
Duration? kimRetry(int retryCount, Object error) {
  final inner = switch (error) {
    ProviderException(:final exception) => exception,
    _ => error,
  };
  final failure = apiFailureOf(inner);
  if (failure != null) {
    return failure.retryable
        ? ProviderContainer.defaultRetry(retryCount, inner)
        : null;
  }
  if (isPermanentClientError(inner)) {
    return null;
  }
  return ProviderContainer.defaultRetry(retryCount, inner);
}
