import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter_displaymode/flutter_displaymode.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:shared_preferences/shared_preferences.dart';

import 'app.dart';
import 'copy.dart';
import 'core/logger.dart';
import 'core/runtime.dart';
import 'data/conversation_store.dart';
import 'kim_bridge.dart';
import 'state/providers.dart';
import 'state/retry.dart';
import 'theme/kim_theme.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  FlutterError.onError = (details) {
    KimLogger.error('flutter', details.exception, details.stack);
    FlutterError.presentError(details);
  };
  PlatformDispatcher.instance.onError = (error, stack) {
    KimLogger.error('platform', error, stack);
    return true;
  };
  await _enableMaxRefreshRate();
  runZonedGuarded(
    () => runApp(const KimBoot()),
    (error, stack) => KimLogger.error('zone', error, stack),
  );
}

Future<void> _enableMaxRefreshRate() async {
  if (kIsWeb || defaultTargetPlatform != TargetPlatform.android) {
    return;
  }
  try {
    await FlutterDisplayMode.setHighRefreshRate();
  } catch (e, st) {
    KimLogger.warn('display mode', e, st);
  }
}

class KimBoot extends StatefulWidget {
  const KimBoot({super.key});

  @override
  State<KimBoot> createState() => _KimBootState();
}

class _KimBootState extends State<KimBoot> {
  Widget? _app;

  @override
  void initState() {
    super.initState();
    unawaited(_start());
  }

  Future<void> _start() async {
    final runtime = await KimRuntime.bootstrap(requestNotifications: false);
    final bridge = KimBridge();
    final prefs = await SharedPreferences.getInstance();
    final store = await ConversationStore.openForRuntime(
      support: runtime.paths.support,
      rustStore: runtime.rustStore,
      attachStore: bridge.attachStore,
      prefs: prefs,
    );
    final account = runtime.settings.account;
    if (account.isNotEmpty) {
      await store.warmThreads(account);
    }
    if (!mounted) {
      return;
    }
    setState(() {
      _app = ProviderScope(
        retry: kimRetry,
        overrides: kimProviderOverrides(
          runtime: runtime,
          auth: bridge,
          client: bridge,
          store: store,
        ),
        child: const KimApp(),
      );
    });
  }

  @override
  Widget build(BuildContext context) {
    return _app ?? const KimSplash();
  }
}

class KimSplash extends StatelessWidget {
  const KimSplash({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: Copy.brand,
      debugShowCheckedModeBanner: false,
      theme: KimTheme.light(),
      darkTheme: KimTheme.dark(),
      themeMode: ThemeMode.system,
      home: const Scaffold(body: Center(child: CircularProgressIndicator())),
    );
  }
}
