import 'dart:async';

import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/bridge/agent_bridge.dart';
import 'package:kim_mobile/bridge/goose_bridge.dart';
import 'package:kim_mobile/src/rust/api/types.dart' hide SessionSnapshotDto;
import 'package:kim_mobile/src/rust_agent/api/session.dart';

import '../support/fake_kim.dart';

AgentUiEvent _ev({
  required String kind,
  String stopReason = '',
  String message = '',
  bool recentlyActive = false,
  bool ok = false,
}) {
  return AgentUiEvent(
    kind: kind,
    operationId: 'op',
    callId: '',
    name: '',
    delta: '',
    argumentsJson: '',
    outputPreview: '',
    ok: ok,
    stopReason: stopReason,
    message: message,
    inputTokens: BigInt.zero,
    outputTokens: BigInt.zero,
    resumedOps: const [],
    recentlyActive: recentlyActive,
  );
}

class _StopSession implements AgentSessionPort {
  _StopSession(this.event);

  final AgentUiEvent event;
  final _ctrl = StreamController<AgentUiEvent>.broadcast();

  @override
  Stream<AgentUiEvent> listen() => _ctrl.stream;

  @override
  Future<String> prompt({required String text}) async {
    _ctrl.add(event);
    return '';
  }

  @override
  Future<String> promptWithContext({
    required String text,
    required String contextJson,
  }) async => '';

  @override
  Future<String> completeTool({
    required String callId,
    required String outputJson,
  }) async => '';

  @override
  Future<String> respondPermission({
    required String callId,
    required String permission,
  }) async => '';

  @override
  Future<void> close() async {}

  @override
  Future<void> abort() async {}

  @override
  Future<void> steer({required String text}) async {}

  @override
  Future<void> reconfigure({required SessionOpenOpts opts}) async {}

  @override
  Future<ResumeReportDto> resume() async =>
      const ResumeReportDto(resumedOps: [], statuses: []);

  @override
  SessionSnapshotDto snapshot() => const SessionSnapshotDto(
    busy: false,
    lastOperationId: '',
    phase: '',
    pendingCallIds: [],
  );
}

void main() {
  test('driveSession does not throw quiet stop reasons', () async {
    final cases = [
      _ev(kind: 'assistant_finished', stopReason: 'empty', message: ''),
      _ev(kind: 'failed', stopReason: 'idle_timeout'),
      _ev(
        kind: 'failed',
        stopReason: 'hard_timeout',
        recentlyActive: true,
      ),
      _ev(kind: 'failed', stopReason: 'poisoned', message: 'host poisoned'),
      _ev(
        kind: 'assistant_finished',
        stopReason: 'side_effect',
        message: '',
      ),
      _ev(
        kind: 'assistant_finished',
        stopReason: 'completed',
        message: 'hi',
        ok: true,
      ),
    ];
    for (final ev in cases) {
      final loop = AgentRunLoop(FakeKim(), AgentBridge());
      final result = await loop.driveSession(
        _StopSession(ev),
        dest: 'b_bot',
        text: 'hi',
      );
      expect(result.stopReason, ev.stopReason, reason: ev.stopReason);
      expect(result.recentlyActive, ev.recentlyActive);
    }
  });

  test('driveSession throws DriveStop with typed stopReason', () async {
    final cases = [
      _ev(kind: 'aborted', stopReason: 'cancelled'),
      _ev(kind: 'failed', stopReason: 'provider', message: 'rate limited'),
      _ev(kind: 'failed', stopReason: 'failed', message: 'boom'),
    ];
    for (final ev in cases) {
      final loop = AgentRunLoop(FakeKim(), AgentBridge());
      try {
        await loop.driveSession(
          _StopSession(ev),
          dest: 'b_bot',
          text: 'hi',
        );
        fail('expected DriveStop for ${ev.stopReason}');
      } on DriveStop catch (stop) {
        expect(stop.result.stopReason, ev.stopReason);
        expect(stop.result.text, ev.message);
      }
    }
  });

  test('start submits DriveStop stopReason instead of failed', () async {
    final fake = FakeKim();
    final loop = AgentRunLoop(fake, AgentBridge())
      ..promptOverride = (_) async {
        throw const DriveStop(
          DriveResult(
            text: 'rate limited',
            stopReason: 'provider',
            replied: false,
            visible: false,
            recentlyActive: false,
          ),
        );
      };
    final done = loop.start();
    fake.agentRunCtrl.add(
      AgentRunRequestDto(
        dest: 'b_bot',
        profileId: 'p-1',
        text: 'hi',
        inReplyTo: 1,
        epoch: BigInt.one,
      ),
    );
    await Future<void>.delayed(const Duration(milliseconds: 80));
    await fake.agentRunCtrl.close();
    await done;
    expect(fake.submittedAgentRuns, isNotEmpty);
    expect(fake.submittedAgentRuns.last.stopReason, 'provider');
    expect(fake.submittedAgentRuns.last.error, 'rate limited');
  });
}
