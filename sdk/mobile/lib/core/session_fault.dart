/// Dart view of sticky session identity end. Wire kinds match kim-sdk
/// `SessionFault::wire_kind` / `SdkError` kinds. Transport last_error is not this.
library;

enum SessionFault { identityExpired, kicked }

SessionFault? classifySessionFault(String? lastError) {
  return switch (lastError) {
    'auth_expired' ||
    'unauthorized' ||
    'auth-failed' => SessionFault.identityExpired,
    'kickout' => SessionFault.kicked,
    _ => null,
  };
}

bool sessionFaultIsIdentity(String? lastError) =>
    classifySessionFault(lastError) != null;
