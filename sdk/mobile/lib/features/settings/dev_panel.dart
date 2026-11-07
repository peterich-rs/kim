library;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:kim_mobile/copy.dart';
import 'package:kim_mobile/core/env.dart';
import 'package:kim_mobile/core/haptics.dart';
import 'package:kim_mobile/core/logger.dart';
import 'package:kim_mobile/design/kim_header.dart';
import 'package:kim_mobile/features/session/link.dart';
import 'package:kim_mobile/features/session/panic.dart';
import 'package:kim_mobile/features/session/providers.dart';
import 'package:kim_mobile/features/settings/dev_panel_state.dart';
import 'package:kim_mobile/src/rust/api/types.dart' as rust_types;

class DevPanelPage extends ConsumerStatefulWidget {
  const DevPanelPage({super.key});

  @override
  ConsumerState<DevPanelPage> createState() => _DevPanelPageState();
}

class _DevPanelPageState extends ConsumerState<DevPanelPage> {
  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) => _load());
  }

  Future<void> _load() async {
    final client = ref.read(clientPortProvider);
    try {
      final m = await client.metricsSnapshot();
      if (!mounted) {
        return;
      }
      final prefs = ref.read(runtimeProvider).settings.prefs;
      final wipeSeen = prefs.getInt('kim.wipe_banner_seen') ?? 0;
      ref
          .read(devPanelProvider.notifier)
          .showMetrics(metrics: m, wipeSeen: wipeSeen);
    } catch (e, st) {
      KimLogger.warn('metricsSnapshot', e, st);
    }
  }

  Future<void> _ackWipe(int total) async {
    await ref
        .read(runtimeProvider)
        .settings
        .prefs
        .setInt('kim.wipe_banner_seen', total);
    ref.read(devPanelProvider.notifier).ackWipe(total);
  }

  Future<void> _switchEnv(bool local) async {
    try {
      await ref
          .read(clientPortProvider)
          .settingsPreset(
            local
                ? rust_types.SettingsPreset.local
                : rust_types.SettingsPreset.prod,
          );
    } catch (e, st) {
      KimLogger.warn('settingsPreset env', e, st);
    }
    ref.read(linkProvider.notifier).retry();
    ref.read(devPanelProvider.notifier).bump();
  }

  Future<void> _export() async {
    final m = ref.read(devPanelProvider).metrics;
    final settings = await ref.read(clientPortProvider).settingsGet();
    final panic = ref.read(rustPanicProvider);
    final text = [
      'KIM_ENV=${kimEnv.name}',
      'ws=${settings.wsUrl}',
      'http=${settings.httpOrigin}',
      'env=${settings.env}',
      'enqueue=${m?.enqueueTotal ?? 0}',
      'persist=${m?.persistTalkTotal ?? 0}',
      'epoch_drop=${m?.epochDropTotal ?? 0}',
      'store_wipe=${m?.storeWipeTotal.toInt() ?? 0}',
      if (panic != null) 'panic=$panic',
    ].join('\n');
    await Clipboard.setData(ClipboardData(text: text));
    await KimHaptics.success();
  }

  @override
  Widget build(BuildContext context) {
    final panel = ref.watch(devPanelProvider);
    final m = panel.metrics;
    final wipe = m?.storeWipeTotal.toInt() ?? 0;
    final showWipe = wipe > panel.wipeSeen;
    final panic = ref.watch(rustPanicProvider);
    final envSnapshot = ref.watch(envSettingsProvider);
    final local = envSnapshot.value?.httpOrigin.contains('127.0.0.1') ?? false;
    return Scaffold(
      appBar: AppBar(
        automaticallyImplyLeading: false,
        leading: kimHeaderLeading(context),
        title: const Text('DevPanel'),
      ),
      body: ListView(
        children: [
          if (showWipe)
            MaterialBanner(
              content: Text(Copy.devPanelWipeBanner(wipe)),
              actions: [
                TextButton(
                  onPressed: () => _ackWipe(wipe),
                  child: Text(Copy.devPanelGotIt),
                ),
              ],
            ),
          ListTile(title: const Text('KIM_ENV'), subtitle: Text(kimEnv.name)),
          Padding(
            padding: const EdgeInsets.fromLTRB(16, 0, 16, 12),
            child: SegmentedButton<bool>(
              segments: [
                ButtonSegment(value: false, label: Text(Copy.prodServer)),
                ButtonSegment(value: true, label: Text(Copy.localServer)),
              ],
              selected: {local},
              onSelectionChanged: (next) => _switchEnv(next.first),
            ),
          ),
          ListTile(
            title: const Text('metrics'),
            subtitle: Text(
              'enqueue ${m?.enqueueTotal ?? 0}  persist ${m?.persistTalkTotal ?? 0}  '
              'epoch_drop ${m?.epochDropTotal ?? 0}  wipe $wipe',
            ),
          ),
          if (panic != null)
            ListTile(title: const Text('RustPanic'), subtitle: Text(panic)),
          ListTile(title: const Text('export logs'), onTap: _export),
        ],
      ),
    );
  }
}
