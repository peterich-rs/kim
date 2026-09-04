/// Unverified JWT payload peek. The gateway checks the signature.
library;

import 'dart:convert';

abstract final class JwtPeek {
  static const skewSeconds = 30;

  static String? account(String token) {
    final acc = _payload(token)?['acc'];
    if (acc is String && acc.isNotEmpty) {
      return acc;
    }
    return null;
  }

  static int? exp(String token) {
    final exp = _payload(token)?['exp'];
    if (exp is int) {
      return exp;
    }
    if (exp is num) {
      return exp.toInt();
    }
    return null;
  }

  /// True only when `exp` is present and already past (30s skew).
  /// Tokens that are not JWTs (tests, opaque) are left alone.
  static bool isExpired(String token, {DateTime? now}) {
    final exp = JwtPeek.exp(token);
    if (exp == null) {
      return false;
    }
    final unix = (now ?? DateTime.now()).millisecondsSinceEpoch ~/ 1000;
    return exp <= unix + skewSeconds;
  }

  static Map<String, Object?>? _payload(String token) {
    final parts = token.split('.');
    if (parts.length < 2 || parts[1].isEmpty) {
      return null;
    }
    try {
      final json = utf8.decode(base64Url.decode(_pad(parts[1])));
      final body = jsonDecode(json);
      if (body is Map<String, Object?>) {
        return body;
      }
      if (body is Map) {
        return Map<String, Object?>.from(body);
      }
      return null;
    } catch (_) {
      return null;
    }
  }

  static String _pad(String s) {
    final mod = s.length % 4;
    if (mod == 0) {
      return s;
    }
    return s.padRight(s.length + (4 - mod), '=');
  }
}
