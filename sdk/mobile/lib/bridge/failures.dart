/// Policy over the FRB-generated [ApiFailure] sealed class.
library;

import 'package:kim_mobile/src/rust/api/failure.dart';

ApiFailure? apiFailureOf(Object err) {
  return switch (err) {
    ApiFailure failure => failure,
    _ => null,
  };
}

/// Text carried only for logs. UI copy does not read this.
String apiFailureDetail(Object err) {
  return switch (err) {
    ApiFailure_Disk(:final message) ||
    ApiFailure_InvalidArgument(:final message) ||
    ApiFailure_Internal(:final message) => message,
    ApiFailure_NotFound(:final what) => what,
    _ => '',
  };
}

extension ApiFailurePolicy on ApiFailure {
  /// Transport / congestion. Matches `SdkError::retryable`.
  bool get retryable => switch (this) {
    ApiFailure_NotConnected() ||
    ApiFailure_Busy() ||
    ApiFailure_SqliteBusy() ||
    ApiFailure_RateLimited() => true,
    _ => false,
  };

  /// Send retry. Matches `SdkError::retryable_send`: status 3 or 3xx.
  bool get retryableSend {
    if (retryable) {
      return true;
    }
    return switch (this) {
      ApiFailure_Protocol(:final status)
          when status == 3 || (status >= 300 && status < 400) =>
        true,
      _ => false,
    };
  }
}
