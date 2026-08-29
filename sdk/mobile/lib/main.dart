import 'package:flutter/material.dart';

import 'kim_bridge.dart';

void main() {
  runApp(const KimApp());
}

class KimApp extends StatelessWidget {
  const KimApp({super.key});

  @override
  Widget build(BuildContext context) {
    return const MaterialApp(
      title: 'KIM mobile',
      home: ShellPage(),
    );
  }
}

class ShellPage extends StatefulWidget {
  const ShellPage({super.key});

  @override
  State<ShellPage> createState() => _ShellPageState();
}

class _ShellPageState extends State<ShellPage> {
  final _url = TextEditingController(text: 'wss://kim.ainexc.com/');
  final _token = TextEditingController();
  final _dest = TextEditingController(text: 'bob');
  final _body = TextEditingController(text: 'hello');
  final _log = StringBuffer();
  final _bridge = KimBridge();
  bool _busy = false;

  void _append(String line) {
    setState(() {
      _log.writeln(line);
    });
  }

  Future<void> _run(String label, Future<String> Function() fn) async {
    if (_busy) {
      return;
    }
    setState(() => _busy = true);
    _append('$label...');
    try {
      _append(await fn());
    } catch (e) {
      _append('ERR $e');
    } finally {
      setState(() => _busy = false);
    }
  }

  @override
  void dispose() {
    _url.dispose();
    _token.dispose();
    _dest.dispose();
    _body.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text('KIM (shell)')),
      body: ListView(
        padding: const EdgeInsets.all(12),
        children: [
          Text('Flutter ${KimBridge.flutterPin} · ${_bridge.ffiStatus}'),
          TextField(
            controller: _url,
            decoration: const InputDecoration(
              labelText: 'WGateway URL (ws:// or wss://)',
            ),
          ),
          TextField(
            controller: _token,
            decoration: const InputDecoration(
              labelText: 'JWT (from Royal /login — not stored in repo)',
            ),
          ),
          Wrap(
            spacing: 8,
            children: [
              TextButton(
                onPressed: () => _url.text = 'ws://127.0.0.1:8001/',
                child: const Text('local :8001'),
              ),
              TextButton(
                onPressed: () => _url.text = 'wss://kim.ainexc.com/',
                child: const Text('prod wss'),
              ),
            ],
          ),
          Row(
            children: [
              ElevatedButton(
                onPressed: _busy
                    ? null
                    : () => _run(
                          'connect',
                          () => _bridge.connect(_url.text, _token.text),
                        ),
                child: const Text('connect'),
              ),
              const SizedBox(width: 8),
              ElevatedButton(
                onPressed: _busy ? null : () => _run('login', _bridge.login),
                child: const Text('login'),
              ),
              const SizedBox(width: 8),
              ElevatedButton(
                onPressed: _busy ? null : () => _run('ping', _bridge.ping),
                child: const Text('ping'),
              ),
            ],
          ),
          TextField(
            controller: _dest,
            decoration: const InputDecoration(labelText: 'talk dest (account)'),
          ),
          TextField(
            controller: _body,
            decoration: const InputDecoration(labelText: 'body'),
          ),
          ElevatedButton(
            onPressed: _busy
                ? null
                : () => _run('talk', () => _bridge.talk(_dest.text, _body.text)),
            child: const Text('talk_to_user'),
          ),
          ElevatedButton(
            onPressed: _busy
                ? null
                : () => _run('disconnect', _bridge.disconnect),
            child: const Text('disconnect'),
          ),
          const SizedBox(height: 12),
          const Text('log'),
          SelectableText(_log.toString()),
        ],
      ),
    );
  }
}
