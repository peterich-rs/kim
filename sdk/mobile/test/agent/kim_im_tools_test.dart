import 'dart:async';
import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/bridge/agent_bridge.dart';
import 'package:kim_mobile/bridge/goose_bridge.dart';
import 'package:kim_mobile/features/agent/kim_im_tools.dart';
import 'package:kim_mobile/models/models.dart';

import '../support/fake_kim.dart';

AgentUiEvent _ev({
  required String kind,
  String callId = '',
  String name = '',
  String argumentsJson = '',
  String message = '',
  String delta = '',
}) {
  return AgentUiEvent(
    kind: kind,
    operationId: 'op',
    callId: callId,
    name: name,
    delta: delta,
    argumentsJson: argumentsJson,
    outputPreview: '',
    ok: kind == 'assistant_finished',
    stopReason: '',
    message: message,
    inputTokens: BigInt.zero,
    outputTokens: BigInt.zero,
    resumedOps: const [],
    recentlyActive: false,
  );
}

class _ToolSession implements AgentSessionPort {
  final _ctrl = StreamController<AgentUiEvent>.broadcast();
  String? completedCallId;
  String? completedOutput;
  var closed = false;

  @override
  Stream<AgentUiEvent> listen() => _ctrl.stream;

  @override
  Future<String> prompt({required String text}) async {
    _ctrl.add(
      _ev(
        kind: 'tool_request',
        callId: 'c1',
        name: 'search_contacts',
        argumentsJson: '{"query":"bob"}',
      ),
    );
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
  }) async {
    completedCallId = callId;
    completedOutput = outputJson;
    _ctrl.add(_ev(kind: 'assistant_finished', message: 'found bob'));
    return '';
  }

  @override
  Future<String> respondPermission({
    required String callId,
    required String permission,
  }) async => '';

  @override
  Future<void> close() async {
    closed = true;
    await _ctrl.close();
  }

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
  TestWidgetsFlutterBinding.ensureInitialized();

  test('search_contacts returns friend hits', () async {
    final fake = FakeKim()
      ..friends = const [
        KimPerson(account: 'bob', nickname: 'Bob'),
        KimPerson(account: 'cara', nickname: 'Cara'),
      ];
    final json = jsonDecode(
      await KimImTools(fake).execute(
        name: 'search_contacts',
        argumentsJson: '{"query":"bo"}',
        currentDest: 'b_bot',
      ),
    );
    expect(json['ok'], true);
    expect(json['people'], [
      {'account': 'bob', 'nickname': 'Bob', 'kind': 1},
    ]);
  });

  test('send_message refuses agent dests', () async {
    final fake = FakeKim();
    final json = jsonDecode(
      await KimImTools(fake).execute(
        name: 'send_message',
        argumentsJson: '{"dest":"b_bot","text":"hi"}',
        currentDest: 'b_bot',
      ),
    );
    expect(json['ok'], false);
    expect(fake.talks, 0);
  });

  test(
    'driveSession completes deferred tool_request instead of hanging',
    () async {
      final fake = FakeKim()
        ..friends = const [KimPerson(account: 'bob', nickname: 'Bob')];
      final session = _ToolSession();
      final loop = AgentRunLoop(fake, AgentBridge());
      final output = await loop
          .driveSession(session, dest: 'b_bot', text: 'find bob')
          .timeout(const Duration(seconds: 2));
      expect(output.text, 'found bob');
      expect(session.completedCallId, 'c1');
      expect(session.completedOutput, contains('bob'));
      expect(jsonDecode(session.completedOutput!)['ok'], true);
      expect(session.closed, isTrue);
    },
  );

  test(
    'driveSession does not treat progress deltas as the final reply',
    () async {
      final session = _DeltaSession(finish: '');
      final loop = AgentRunLoop(FakeKim(), AgentBridge());
      final output = await loop.driveSession(
        session,
        dest: 'b_bot',
        text: 'fix login',
      );
      expect(output.text, isEmpty);
      expect(session.closed, isTrue);
    },
  );

  test(
    'driveSession returns assistant_finished text, not earlier deltas',
    () async {
      final session = _DeltaSession(finish: '按钮已经从登录页拿掉了。');
      final loop = AgentRunLoop(FakeKim(), AgentBridge());
      final output = await loop.driveSession(
        session,
        dest: 'b_bot',
        text: 'fix login',
      );
      expect(output.text, '按钮已经从登录页拿掉了。');
    },
  );
}

class _DeltaSession implements AgentSessionPort {
  _DeltaSession({required this.finish});

  final String finish;
  final _ctrl = StreamController<AgentUiEvent>.broadcast();
  var closed = false;

  @override
  Stream<AgentUiEvent> listen() => _ctrl.stream;

  @override
  Future<String> prompt({required String text}) async {
    _ctrl
      ..add(_ev(kind: 'assistant_text_delta', delta: '接着查登录页历史和可能叠在上面的控件。'))
      ..add(_ev(kind: 'assistant_finished', message: finish));
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
    closed = true;
    await _ctrl.close();
  }

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
