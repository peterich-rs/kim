/// HTTPS enforcement for Royal auth origins.
library;

/// Returns null when [origin] is allowed; otherwise a clear error reason.
String? insecureAuthOriginReason(String origin) {
  final base = origin.trim().replaceAll(RegExp(r'/$'), '');
  if (base.isEmpty) {
    return 'auth origin is empty';
  }
  final uri = Uri.tryParse(base);
  if (uri == null || !uri.hasScheme || uri.host.isEmpty) {
    return 'auth origin must be https (or http://127.0.0.1 / localhost for local dev)';
  }
  if (uri.scheme == 'https') {
    return null;
  }
  if (uri.scheme == 'http' && _isLoopbackHost(uri.host)) {
    return null;
  }
  return 'auth origin must be https (or http://127.0.0.1 / localhost for local dev)';
}

bool isSecureAuthOrigin(String origin) => insecureAuthOriginReason(origin) == null;

bool _isLoopbackHost(String host) {
  final h = host.toLowerCase();
  return h == '127.0.0.1' ||
      h == 'localhost' ||
      h == '::1' ||
      h == '0:0:0:0:0:0:0:1';
}
