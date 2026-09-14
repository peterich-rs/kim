library;

import 'package:flutter_riverpod/flutter_riverpod.dart';

class KimCapabilityHost {
  KimCapabilityHost(this.ref);

  final Ref ref;

  Future<String> execute({
    required String name,
    required String argumentsJson,
    required String sessionDest,
    required String profileId,
    String callId = '',
  }) async {
    return '{"error":"unavailable"}';
  }
}
