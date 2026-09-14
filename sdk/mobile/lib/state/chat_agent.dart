library;

import 'package:flutter_riverpod/flutter_riverpod.dart';

class ChatAgent {
  ChatAgent(this._ref);
  // ignore: unused_field
  final Ref _ref;
  Future<void> enqueueTurn(String dest, String text, int inReplyTo) async {}
  Future<void> catchUpPending() async {}
  Future<void> sendDirect({required String dest, required String text}) async {}
  Future<void> respondPermission({
    required String dest,
    required String callId,
    required String permission,
    String toolName = '',
  }) async {}
}

final chatAgentProvider = Provider<ChatAgent>(ChatAgent.new);
