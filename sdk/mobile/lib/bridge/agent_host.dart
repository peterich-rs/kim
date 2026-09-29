/// Desktop host subscriber. Turns run inside `HostAgentRuntime`; this only
/// forwards permission / presence events. Secrets resolve Rust-side through
/// the Keychain executor channel — no Dart seed loop.
library;

import 'dart:async';

import 'package:kim_mobile/bridge/kim_bridge.dart';
import 'package:kim_mobile/core/logger.dart';
import 'package:kim_mobile/features/agent/agent_permission.dart';
import 'package:kim_mobile/features/agent/agent_presence.dart';
import 'package:kim_mobile/features/agent/host_support.dart';

class AgentHostController {
  AgentHostController(this.client, {this.permissions, this.runs});

  final KimClientPort client;
  final AgentPermissionHub? permissions;
  final AgentRunSink? runs;

  StreamSubscription<dynamic>? _permSub;
  StreamSubscription<dynamic>? _uiSub;
  var _stopped = false;

  Future<void> start() async {
    if (!agentHostSupported) {
      return;
    }
    while (!_stopped) {
      permissions?.bindClient(client);
      final done = Completer<void>();
      await _permSub?.cancel();
      await _uiSub?.cancel();
      _permSub = client.watchAgentPermission().listen(
        (event) {
          permissions?.prompt(
            event.dest,
            AgentPermissionPrompt(
              callId: event.callId,
              name: event.name,
              preview: event.preview,
            ),
          );
        },
        onError: (Object error, StackTrace st) {
          KimLogger.warn('agent permission watch', error, st);
          if (!done.isCompleted) {
            done.complete();
          }
        },
        onDone: () {
          if (!done.isCompleted) {
            done.complete();
          }
        },
      );
      _uiSub = client.watchAgentUi().listen((event) {
        switch (event.phase) {
          case 'running':
            runs?.begin(event.dest);
          case 'failed':
            runs?.finish(event.dest, failed: true);
          case 'done':
            runs?.finish(event.dest, failed: false);
          default:
            break;
        }
      });
      await done.future;
      if (_stopped) {
        break;
      }
      KimLogger.warn('agent permission watch ended; resubscribing');
      await Future<void>.delayed(const Duration(milliseconds: 200));
    }
  }

  Future<void> stop() async {
    _stopped = true;
    await _permSub?.cancel();
    _permSub = null;
    await _uiSub?.cancel();
    _uiSub = null;
  }
}
