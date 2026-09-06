/// Dart shell around `kim_agent_ffi` (hard isolation from [KimBridge] / IM).
library;

import 'package:flutter/foundation.dart' show kIsWeb;

import 'src/rust_agent/api/session.dart' as agent;
import 'src/rust_agent/frb_generated.dart';

export 'src/rust_agent/api/session.dart'
    show
        AgentSession,
        AgentUiEvent,
        ResumeReportDto,
        SessionOpenOpts,
        SessionSnapshotDto;

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

  Future<agent.AgentSession> open({
    required String sqlitePath,
    required String projectRoot,
    required agent.SessionOpenOpts opts,
  }) async {
    await ensure();
    return agent.sessionOpen(
      sqlitePath: sqlitePath,
      projectRoot: projectRoot,
      opts: opts,
    );
  }
}
