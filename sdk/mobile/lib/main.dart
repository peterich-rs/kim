import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter_displaymode/flutter_displaymode.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:kim_mobile/app.dart';
import 'package:kim_mobile/copy.dart';
import 'package:kim_mobile/core/logger.dart';
import 'package:kim_mobile/core/runtime.dart';
import 'package:kim_mobile/features/agent/agent_permission.dart';
import 'package:kim_mobile/features/agent/agent_presence.dart';
import 'package:kim_mobile/features/agent/host_support.dart';
import 'package:kim_mobile/bridge/goose_bridge.dart';
import 'package:kim_mobile/bridge/agent_bridge.dart';
import 'package:kim_mobile/bridge/kim_bridge.dart';
import 'package:kim_mobile/src/rust/api/types.dart' as rust_types;
import 'package:kim_mobile/features/session/providers.dart';
import 'package:kim_mobile/features/session/retry.dart';
import 'package:kim_mobile/design/kim_theme.dart';

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
  runApp(const KimBoot());
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
  ProviderContainer? _container;
  DetachableAgentRunSink? _sink;
  AgentRunLoop? _loop;

  @override
  void initState() {
    super.initState();
    unawaited(_start());
  }

  @override
  void dispose() {
    _sink?.detach();
    unawaited(_loop?.stop());
    _container?.dispose();
    super.dispose();
  }

  Future<void> _start() async {
    final runtime = await KimRuntime.bootstrap(requestNotifications: false);
    final bridge = KimBridge();
    await bridge.attachStore('${runtime.paths.support.path}/kim-cache.db');
    final imported = await bridge.importDeviceSettings(
      wsUrl: runtime.settings.url,
      httpOrigin: runtime.settings.httpOrigin,
    );
    runtime.settings.applyRemote(
      wsUrl: imported.wsUrl,
      httpOrigin: imported.httpOrigin,
      account: imported.account,
      env: imported.env,
    );
    await runtime.settings.dropImportedPrefs();
    bridge.watchTokenPersist().listen((event) {
      switch (event) {
        case rust_types.TokenPersistDto_Write(:final token):
          unawaited(runtime.settings.saveToken(token));
        case rust_types.TokenPersistDto_Clear():
          unawaited(runtime.settings.saveToken(''));
      }
    });
    if (!mounted) {
      return;
    }
    final container = ProviderContainer(
      retry: kimRetry,
      overrides: kimProviderOverrides(
        runtime: runtime,
        auth: bridge,
        client: bridge,
        media: bridge,
      ),
    );
    _container = container;
    if (agentHostSupported) {
      final sink = DetachableAgentRunSink(
        container.read(agentRunStatusProvider.notifier),
      );
      _sink = sink;
      final loop = AgentRunLoop(
        bridge,
        AgentBridge(),
        sink: sink,
        permissions: container.read(agentPermissionHubProvider.notifier),
      );
      _loop = loop;
      unawaited(loop.start());
    }
    setState(() {
      _app = UncontrolledProviderScope(
        container: container,
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
