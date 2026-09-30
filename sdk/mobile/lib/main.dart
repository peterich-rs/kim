import 'dart:async';
import 'dart:io' show Platform;

import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter_displaymode/flutter_displaymode.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:kim_mobile/app.dart';
import 'package:kim_mobile/copy.dart';
import 'package:kim_mobile/core/logger.dart';
import 'package:kim_mobile/core/runtime.dart';
import 'package:kim_mobile/core/secret_store_executor.dart';
import 'package:kim_mobile/features/agent/providers/agent_permission.dart';
import 'package:kim_mobile/features/agent/providers/agent_presence.dart';
import 'package:kim_mobile/features/agent/data/host_support.dart';
import 'package:kim_mobile/bridge/agent_host.dart';
import 'package:kim_mobile/bridge/kim_bridge.dart';
import 'package:kim_mobile/src/rust/api/bootstrap.dart';
import 'package:kim_mobile/src/rust_agent/api/catalog.dart' as agent_catalog;
import 'package:kim_mobile/features/session/providers.dart';
import 'package:kim_mobile/features/session/retry.dart';
import 'package:kim_mobile/design/kim_theme.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  FlutterError.onError = (details) {
    KimLogger.error('flutter', details.exception, details.stack);
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
  AgentHostController? _host;
  SecretStoreExecutor? _secrets;

  @override
  void initState() {
    super.initState();
    unawaited(_start());
  }

  @override
  void dispose() {
    _sink?.detach();
    unawaited(_host?.stop());
    unawaited(_secrets?.detach());
    _container?.dispose();
    super.dispose();
  }

  Future<void> _start() async {
    final runtime = await KimRuntime.bootstrap(requestNotifications: false);
    final bridge = KimBridge();
    if (!kIsWeb) {
      await platformBootstrap(
        documents: runtime.paths.documents.path,
        support: runtime.paths.support.path,
        cache: runtime.paths.cache.path,
        temp: runtime.paths.temp.path,
        appVersion: runtime.version,
        buildNumber: runtime.buildNumber,
        simulator: platformIsSimulator,
      );
      if (agentHostSupported) {
        await agent_catalog.setPlatformSupportRoot(
          support: runtime.paths.support.path,
        );
      }
      final secrets = SecretStoreExecutor();
      _secrets = secrets;
      await secrets.attach();
      await bridge.attachStore();
      KimLogger.info('boot store attached');
      await runtime.settings.dropImportedPrefs();
    }
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
      final host = AgentHostController(
        bridge,
        permissions: container.read(agentPermissionHubProvider.notifier),
        runs: sink,
      );
      _host = host;
      KimLogger.info('boot agent host');
      unawaited(host.start());
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

/// iOS simulator detection for the loopback warning: the env vars exist only
/// in the simulator process. Android emulators reach host loopback via 10.0.2.2,
/// so `127.0.0.1` there is the device itself.
bool get platformIsSimulator {
  if (kIsWeb) {
    return false;
  }
  if (defaultTargetPlatform == TargetPlatform.iOS) {
    return Platform.environment['SIMULATOR_DEVICE_NAME'] != null ||
        Platform.environment['SIMULATOR_UDID'] != null;
  }
  return false;
}
