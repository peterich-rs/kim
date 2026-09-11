/// Dart shell around `kim_agent_ffi` (hard isolation from [KimBridge] / IM).
library;

import 'package:flutter/foundation.dart' show kIsWeb;

import 'src/rust_agent/api/session.dart';
import 'src/rust_agent/frb_generated.dart';

export 'src/rust_agent/api/session.dart'
    show
        AgentSession,
        AgentUiEvent,
        ResumeReportDto,
        SessionOpenOpts,
        SessionSnapshotDto;

/// Production FFI session or a test double. ChatAgent is the only caller.
abstract class AgentSessionPort {
  Stream<AgentUiEvent> listen();
  Future<String> prompt({required String text});
  Future<void> close();
  Future<void> abort();
  Future<void> reconfigure({required SessionOpenOpts opts});
  Future<ResumeReportDto> resume();
  SessionSnapshotDto snapshot();
}

class NativeAgentSession implements AgentSessionPort {
  NativeAgentSession(this._inner);

  final AgentSession _inner;

  @override
  Stream<AgentUiEvent> listen() => _inner.listen();

  @override
  Future<String> prompt({required String text}) => _inner.prompt(text: text);

  @override
  Future<void> close() => _inner.close();

  @override
  Future<void> abort() => _inner.abort();

  @override
  Future<void> reconfigure({required SessionOpenOpts opts}) =>
      _inner.reconfigure(opts: opts);

  @override
  Future<ResumeReportDto> resume() => _inner.resume();

  @override
  SessionSnapshotDto snapshot() => _inner.snapshot();
}

class AgentBridge {
  static bool _inited = false;

  Future<void> ensure() async {
    if (_inited || kIsWeb) {
      return;
    }
    await AgentRustLib.init();
    _inited = true;
  }

  bool get isReady => _inited;

  Future<AgentSessionPort> open({
    required String sqlitePath,
    required String projectRoot,
    required SessionOpenOpts opts,
  }) async {
    await ensure();
    final session = await sessionOpen(
      sqlitePath: sqlitePath,
      projectRoot: projectRoot,
      opts: opts,
    );
    return NativeAgentSession(session);
  }
}
