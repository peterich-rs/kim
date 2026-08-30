library;

import 'package:flutter/material.dart';
import 'package:intl/intl.dart';

import '../copy.dart';
import '../core/errors.dart';
import '../core/haptics.dart';
import '../core/runtime.dart';
import '../core/settings.dart';
import '../core/user_agent.dart';
import '../kim_bridge.dart';
import '../theme/motion.dart';
import '../widgets/kim_busy_barrier.dart';
import '../widgets/kim_log_view.dart';
import '../widgets/kim_text_field.dart';
import 'password_page.dart';

class ShellPage extends StatefulWidget {
  const ShellPage({
    super.key,
    required this.runtime,
    this.bridge,
    this.auth,
    this.onSignedOut,
  });

  final KimRuntime runtime;
  final KimBridge? bridge;
  final KimAuthPort? auth;
  final VoidCallback? onSignedOut;

  @override
  State<ShellPage> createState() => _ShellPageState();
}

class _ShellPageState extends State<ShellPage> {
  late final TextEditingController _url;
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
    _dest = TextEditingController(text: _settings.dest);
    _body = TextEditingController(text: 'hello');
  }

  KimAuthPort get _auth => widget.auth ?? _bridge;

  String get _userAgent => kimUserAgent(widget.runtime);

  @override
  void dispose() {
    _url.dispose();
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
  }

  Future<void> _logout() async {
    if (_busy) {
      return;
    }
    setState(() => _busy = true);
    _append('${Copy.loggingOut}…');
    try {
      try {
        await _bridge.disconnect();
      } catch (_) {
        // Already down.
      }
      try {
        await _auth.logout(
          origin: _settings.httpOrigin,
          userAgent: _userAgent,
          token: _settings.token,
        );
      } catch (err) {
        _append(mapUserError(err));
      }
      await _settings.clearSession();
      await KimHaptics.success();
      widget.onSignedOut?.call();
    } finally {
      if (mounted) {
        setState(() => _busy = false);
      }
    }
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
        actions: [
          IconButton(
            tooltip: Copy.changePassword,
            onPressed: _busy
                ? null
                : () async {
                    final ok = await Navigator.of(context).push<bool>(
                      MaterialPageRoute(
                        builder: (_) => PasswordPage(
                          runtime: widget.runtime,
                          auth: _auth,
                        ),
                      ),
                    );
                    if (ok == true && mounted) {
                      _append(Copy.passwordChanged);
                    }
                  },
            icon: const Icon(Icons.lock_reset_outlined),
          ),
          IconButton(
            key: const Key('logout'),
            tooltip: Copy.logout,
            onPressed: _busy ? null : _logout,
            icon: const Icon(Icons.logout),
          ),
        ],
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
                    Padding(
                      padding: const EdgeInsets.only(top: 8, bottom: 4),
                      child: Text(
                        '${Copy.signedInAs} ${_settings.account.isEmpty ? '—' : _settings.account}',
                        style: theme.textTheme.bodyMedium,
                      ),
                    ),
                    Wrap(
                      spacing: 8,
                      children: [
                        TextButton(
                          onPressed: () async {
                            KimHaptics.selection();
                            await _settings.useLocal();
                            _url.text = _settings.url;
                            setState(() {});
                          },
                          child: const Text('local :8001'),
                        ),
                        TextButton(
                          onPressed: () async {
                            KimHaptics.selection();
                            await _settings.useProd();
                            _url.text = _settings.url;
                            setState(() {});
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
                                    _settings.token,
                                    userAgent: _userAgent,
                                  );
                                }),
                          child: const Text('connect'),
                        ),
                        FilledButton(
                          onPressed: _busy
                              ? null
                              : () => _run('signin', _bridge.loginWs),
                          child: const Text('signin'),
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
