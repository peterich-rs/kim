import 'dart:async';

import 'package:flutter_test/flutter_test.dart';

import '../support/legacy_agent_drive.dart';

import 'package:kim_mobile/bridge/goose_bridge.dart';
import 'package:kim_mobile/src/rust/api/types.dart' hide SessionSnapshot;

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
  Future<void> park() async {}

  @override
  Future<void> steer({required String text}) async {}

  @override
  Future<void> reconfigure() async {}

  @override
  Future<ResumeReport> resume() async =>
      const ResumeReport(resumedOps: [], statuses: []);

  @override
  Future<SessionSnapshot> snapshot() async => const SessionSnapshot(
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
      _ev(kind: 'failed', stopReason: 'hard_timeout', recentlyActive: true),
      _ev(kind: 'failed', stopReason: 'poisoned', message: 'host poisoned'),
      _ev(kind: 'assistant_finished', stopReason: 'side_effect', message: ''),
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
        await loop.driveSession(_StopSession(ev), dest: 'b_bot', text: 'hi');
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
    // Broadcast drops events with no listeners — wait for watch to attach.
    // The loop resubscribes until stop(), so closing the controller does not
    // finish start().
    await Future<void>.delayed(const Duration(milliseconds: 50));
    fake.agentRunCtrl.add(
      AgentRunRequest(
        dest: 'b_bot',
        profileId: 'p-1',
        text: 'hi',
        inReplyTo: 1,
        epoch: BigInt.one,
      ),
    );
    for (var i = 0; i < 50 && fake.submittedAgentRuns.isEmpty; i++) {
      await Future<void>.delayed(const Duration(milliseconds: 20));
    }
    await loop.stop();
    await fake.agentRunCtrl.close();
    await done;
    expect(fake.submittedAgentRuns, isNotEmpty);
    expect(fake.submittedAgentRuns.last.stopReason, 'provider');
    expect(fake.submittedAgentRuns.last.error, 'rate limited');
  });

  test('driveSession returns completed when cancel waits for close', () async {
    final session = _HangUntilCloseSession(
      _ev(
        kind: 'assistant_finished',
        stopReason: 'completed',
        message: 'hi',
        ok: true,
      ),
    );
    final loop = AgentRunLoop(FakeKim(), AgentBridge());
    final result = await loop
        .driveSession(session, dest: 'b_bot', text: 'hi')
        .timeout(const Duration(milliseconds: 200));
    expect(result.stopReason, 'completed');
    expect(result.text, 'hi');
    expect(session.closeCount, 1);
  });

  test('driveSession persist parks instead of closing', () async {
    final session = _HangUntilCloseSession(
      _ev(
        kind: 'assistant_finished',
        stopReason: 'completed',
        message: 'hi',
        ok: true,
      ),
    );
    final loop = AgentRunLoop(FakeKim(), AgentBridge());
    final result = await loop.driveSession(
      session,
      dest: 'b_bot',
      text: 'hi',
      persist: true,
    );
    expect(result.stopReason, 'completed');
    expect(session.parkCount, 1);
    expect(session.closeCount, 0);
  });
}

class _HangUntilCloseSession implements AgentSessionPort {
  _HangUntilCloseSession(this.event);

  final AgentUiEvent event;
  final _src = StreamController<AgentUiEvent>.broadcast();
  final _cancel = Completer<void>();
  var closeCount = 0;
  var parkCount = 0;

  @override
  Stream<AgentUiEvent> listen() {
    late final StreamController<AgentUiEvent> out;
    out = StreamController<AgentUiEvent>(
      onListen: () {
        _src.stream.listen(out.add, onError: out.addError, onDone: out.close);
      },
      onCancel: () => _cancel.future,
    );
    return out.stream;
  }

  @override
  Future<String> prompt({required String text}) async {
    _src.add(event);
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
  Future<void> close() async {
    closeCount += 1;
    if (!_cancel.isCompleted) {
      _cancel.complete();
    }
  }

  @override
  Future<void> abort() async {}

  @override
  Future<void> park() async {
    parkCount += 1;
    if (!_cancel.isCompleted) {
      _cancel.complete();
    }
  }

  @override
  Future<void> steer({required String text}) async {}

  @override
  Future<void> reconfigure() async {}

  @override
  Future<ResumeReport> resume() async =>
      const ResumeReport(resumedOps: [], statuses: []);

  @override
  Future<SessionSnapshot> snapshot() async => const SessionSnapshot(
    busy: false,
    lastOperationId: '',
    phase: '',
    pendingCallIds: [],
  );
}
