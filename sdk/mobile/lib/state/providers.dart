library;

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_riverpod/misc.dart';

import '../core/media.dart';
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

final mediaPortProvider = Provider<KimMediaPort>((ref) {
  throw StateError('mediaPortProvider must be overridden in main / tests');
});

List<Override> kimProviderOverrides({
  required KimRuntime runtime,
  required KimAuthPort auth,
  required KimClientPort client,
  required ConversationStore store,
  KimMediaPort? media,
}) {
  return [
    runtimeProvider.overrideWithValue(runtime),
    authPortProvider.overrideWithValue(auth),
    clientPortProvider.overrideWithValue(client),
    conversationStoreProvider.overrideWithValue(store),
    mediaPortProvider.overrideWithValue(media ?? KimMediaClient()),
  ];
}

/// Radio from [KimConnectivity]. Independent of the WGateway socket.
final radioOnlineProvider = NotifierProvider<RadioOnlineNotifier, bool>(
  RadioOnlineNotifier.new,
);

class RadioOnlineNotifier extends Notifier<bool> {
  @override
  bool build() {
    final listenable = ref.watch(runtimeProvider).connectivity.online;
    void tick() {
      state = listenable.value;
    }

    listenable.addListener(tick);
    ref.onDispose(() => listenable.removeListener(tick));
    return listenable.value;
  }
}
