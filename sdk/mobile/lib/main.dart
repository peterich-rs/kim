import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'app.dart';
import 'core/runtime.dart';
import 'data/conversation_store.dart';
import 'kim_bridge.dart';
import 'state/providers.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  final runtime = await KimRuntime.bootstrap();
  final bridge = KimBridge();
  final store = await ConversationStore.load();
  runApp(
    ProviderScope(
      overrides: [
        runtimeProvider.overrideWithValue(runtime),
        authPortProvider.overrideWithValue(bridge),
        clientPortProvider.overrideWithValue(bridge),
        conversationStoreProvider.overrideWithValue(store),
      ],
      child: const KimApp(),
    ),
  );
}
