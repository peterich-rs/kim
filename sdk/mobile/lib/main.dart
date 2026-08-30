import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:shared_preferences/shared_preferences.dart';

import 'app.dart';
import 'core/runtime.dart';
import 'data/conversation_store.dart';
import 'kim_bridge.dart';
import 'state/providers.dart';
import 'state/retry.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  final runtime = await KimRuntime.bootstrap();
  final bridge = KimBridge();
  final store = await ConversationStore.open(
    support: runtime.paths.support,
    prefs: await SharedPreferences.getInstance(),
  );
  runApp(
    ProviderScope(
      retry: kimRetry,
      overrides: kimProviderOverrides(
        runtime: runtime,
        auth: bridge,
        client: bridge,
        store: store,
      ),
      child: const KimApp(),
    ),
  );
}
