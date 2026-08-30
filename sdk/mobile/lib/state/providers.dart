library;

import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../core/runtime.dart';
import '../data/conversation_store.dart';
import '../kim_bridge.dart';

final runtimeProvider = Provider<KimRuntime>((ref) {
  throw StateError('runtimeProvider must be overridden in main / tests');
});

final authPortProvider = Provider<KimAuthPort>((ref) {
  throw StateError('authPortProvider must be overridden in main / tests');
});

final clientPortProvider = Provider<KimClientPort>((ref) {
  throw StateError('clientPortProvider must be overridden in main / tests');
});

final conversationStoreProvider = Provider<ConversationStore>((ref) {
  throw StateError(
    'conversationStoreProvider must be overridden in main / tests',
  );
});
