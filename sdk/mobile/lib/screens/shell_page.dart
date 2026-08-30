library;

import 'package:flutter/material.dart';
import 'package:intl/intl.dart';

import '../core/haptics.dart';
import '../core/runtime.dart';
import '../core/settings.dart';
import '../kim_bridge.dart';
import '../theme/motion.dart';
import '../widgets/kim_busy_barrier.dart';
import '../widgets/kim_log_view.dart';
import '../widgets/kim_text_field.dart';

class ShellPage extends StatefulWidget {
  const ShellPage({super.key, required this.runtime, this.bridge});

  final KimRuntime runtime;
  final KimBridge? bridge;

  @override
  State<ShellPage> createState() => _ShellPageState();
}

class _ShellPageState extends State<ShellPage> {
  late final TextEditingController _url;
  late final TextEditingController _token;
  late final TextEditingController _dest;
  late final TextEditingController _body;
  late final KimBridge _bridge;
  final List<KimLogLine> _lines = [];
  bool _busy = false;
  final _time = DateFormat.Hms();

  SettingsStore get _settings => widget.runtime.settings;

  @override
  void initState() {
    super.initState();
    _bridge = widget.bridge ?? KimBridge();
    _url = TextEditingController(text: _settings.url);
    _token = TextEditingController(text: _settings.token);
    _dest = TextEditingController(text: _settings.dest);
    _body = TextEditingController(text: 'hello');
  }

  @override
  void dispose() {
    _url.dispose();
    _token.dispose();
    _dest.dispose();
    _body.dispose();
    super.dispose();
  }

  void _append(String line) {
    setState(() {
      _lines.add(KimLogLine('${_time.format(DateTime.now())}  $line'));
    });
  }

  Future<void> _persistFields() async {
    await _settings.saveUrl(_url.text);
    await _settings.saveDest(_dest.text);
    await _settings.saveToken(_token.text);
  }

  Future<void> _run(String label, Future<String> Function() fn) async {
    if (_busy) {
      return;
    }
    setState(() => _busy = true);
    _append('$label…');
    try {
      final out = await fn();
      _append(out);
      await KimHaptics.success();
    } catch (e) {
      _append('ERR $e');
      await KimHaptics.error();
    } finally {
      if (mounted) {
        setState(() => _busy = false);
      }
    }
  }

  Future<void> _refreshLog() async {
    _append('refreshed (online=${widget.runtime.connectivity.isOnline})');
  }

  @override
  Widget build(BuildContext context) {
    final runtime = widget.runtime;
    final theme = Theme.of(context);
    return Scaffold(
      appBar: AppBar(
        title: const Text('KIM (shell)'),
        bottom: PreferredSize(
          preferredSize: Size.fromHeight(_busy ? 4 : 0),
          child: AnimatedOpacity(
            duration: KimMotion.short,
            opacity: _busy ? 1 : 0,
            child: AnimatedSize(
              duration: KimMotion.short,
              child: _busy
                  ? const LinearProgressIndicator(minHeight: 3)
                  : const SizedBox(width: double.infinity, height: 0),
            ),
          ),
        ),
      ),
      body: KimBusyBarrier(
        busy: _busy,
        child: RefreshIndicator(
          onRefresh: _refreshLog,
          child: ListView(
            physics: const AlwaysScrollableScrollPhysics(),
            padding: const EdgeInsets.fromLTRB(12, 8, 12, 24),
            children: [
              Text(
                '${_bridge.ffiStatus}\n'
                'Flutter ${KimBridge.flutterPin} · app ${runtime.versionLabel}\n'
                'data ${runtime.paths.dataDirShort}',
                style: theme.textTheme.bodySmall,
              ),
              AnimatedOpacity(
                duration: KimMotion.short,
                opacity: _busy ? 0.55 : 1,
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
                    KimTextField(
                      controller: _url,
                      label: 'WGateway URL (ws:// or wss://)',
                      keyboardType: TextInputType.url,
                      onEditingComplete: () => _settings.saveUrl(_url.text),
                    ),
                    KimTextField(
                      controller: _token,
                      label: 'JWT (from Royal /login — not stored in repo)',
                      obscureable: true,
                      onEditingComplete: () => _settings.saveToken(_token.text),
                    ),
                    Wrap(
                      spacing: 8,
                      children: [
                        TextButton(
                          onPressed: () {
                            KimHaptics.selection();
                            _url.text = SettingsStore.localUrl;
                            _settings.saveUrl(_url.text);
                          },
                          child: const Text('local :8001'),
                        ),
                        TextButton(
                          onPressed: () {
                            KimHaptics.selection();
                            _url.text = SettingsStore.defaultUrl;
                            _settings.saveUrl(_url.text);
                          },
                          child: const Text('prod wss'),
                        ),
                      ],
                    ),
                    const SizedBox(height: 8),
                    Wrap(
                      spacing: 8,
                      runSpacing: 8,
                      children: [
                        FilledButton(
                          onPressed: _busy
                              ? null
                              : () => _run('connect', () async {
                                  await _persistFields();
                                  return _bridge.connect(
                                    _url.text,
                                    _token.text,
                                  );
                                }),
                          child: const Text('connect'),
                        ),
                        FilledButton(
                          onPressed: _busy
                              ? null
                              : () => _run('login', _bridge.login),
                          child: const Text('login'),
                        ),
                        OutlinedButton(
                          onPressed: _busy
                              ? null
                              : () => _run('ping', _bridge.ping),
                          child: const Text('ping'),
                        ),
                      ],
                    ),
                    KimTextField(
                      controller: _dest,
                      label: 'talk dest (account)',
                      onEditingComplete: () => _settings.saveDest(_dest.text),
                    ),
                    KimTextField(
                      controller: _body,
                      label: 'body',
                      textInputAction: TextInputAction.done,
                    ),
                    const SizedBox(height: 8),
                    FilledButton(
                      onPressed: _busy
                          ? null
                          : () => _run('talk', () async {
                              await _settings.saveDest(_dest.text);
                              return _bridge.talk(_dest.text, _body.text);
                            }),
                      child: const Text('talk_to_user'),
                    ),
                    const SizedBox(height: 8),
                    OutlinedButton(
                      onPressed: _busy
                          ? null
                          : () => _run('disconnect', _bridge.disconnect),
                      child: const Text('disconnect'),
                    ),
                  ],
                ),
              ),
              const SizedBox(height: 16),
              Text('log', style: theme.textTheme.titleSmall),
              const SizedBox(height: 6),
              KimLogView(lines: _lines),
            ],
          ),
        ),
      ),
    );
  }
}
