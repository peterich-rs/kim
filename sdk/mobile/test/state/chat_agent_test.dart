import 'dart:async';

import 'package:flutter_secure_storage/test/test_flutter_secure_storage_platform.dart';
// ignore: depend_on_referenced_packages
import 'package:flutter_secure_storage_platform_interface/flutter_secure_storage_platform_interface.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/agent/mention.dart';
import 'package:kim_mobile/agent_bridge.dart';
import 'package:kim_mobile/state/chat_agent.dart';
import 'package:kim_mobile/state/messages.dart';
import 'package:shared_preferences/shared_preferences.dart';

import '../support/harness.dart';

class _RecordingAgentBridge extends AgentBridge {
  SessionOpenOpts? lastOpts;
  var opens = 0;
  AgentSessionPort? session;

  @override
  Future<void> ensure() async {}

  @override
  Future<AgentSessionPort> open({
    required String sqlitePath,
    required String projectRoot,
    required SessionOpenOpts opts,
  }) async {
    opens += 1;
    lastOpts = opts;
    final next = session;
    if (next == null) {
      throw StateError('scripted: no live host');
    }
    return next;
  }
}

class _OneShotSession implements AgentSessionPort {
  _OneShotSession(this.reply);

  final String reply;
  final _ctrl = StreamController<AgentUiEvent>.broadcast();

  @override
  Stream<AgentUiEvent> listen() => _ctrl.stream;

  @override
  Future<String> promptWithContext({
    required String text,
    required String contextJson,
  }) => prompt(text: text);

  @override
  Future<String> completeTool({
    required String callId,
    required String outputJson,
  }) async => 'op';

  @override
  Future<String> prompt({required String text}) async {
    _ctrl.add(
      AgentUiEvent(
        kind: 'assistant_finished',
        operationId: 'op1',
        callId: '',
        name: '',
        delta: '',
        argumentsJson: '',
        outputPreview: '',
        ok: true,
        stopReason: 'completed',
        message: reply,
        inputTokens: BigInt.zero,
        outputTokens: BigInt.zero,
        resumedOps: const [],
      ),
    );
    return 'op1';
  }

  @override
  Future<void> close() async {}

  @override
  Future<void> abort() async {}

  @override
  Future<void> reconfigure({required SessionOpenOpts opts}) async {}

  @override
  Future<ResumeReportDto> resume() async =>
      const ResumeReportDto(resumedOps: [], statuses: []);

  @override
  SessionSnapshotDto snapshot() => const SessionSnapshotDto(
    busy: false,
    lastOperationId: '',
    phase: 'idle',
    pendingCallIds: [],
  );
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  late FlutterSecureStoragePlatform previousPlatform;

  setUp(() {
    previousPlatform = FlutterSecureStoragePlatform.instance;
  });

  tearDown(() {
    FlutterSecureStoragePlatform.instance = previousPlatform;
  });

  test('first bot prompt after restart loads the saved API key', () async {
    final bridge = _RecordingAgentBridge();
    final env = await kimHarness(
      token: 'tok.jwt',
      account: 'alice',
      overrides: [agentBridgeProvider.overrideWithValue(bridge)],
    );
    final prefs = await SharedPreferences.getInstance();
    await prefs.setString('agent.llm_backend', 'openai');
    await prefs.setString('agent.base_url', 'https://api.openai.com/v1');
    await prefs.setString('agent.model', 'gpt-4o');
    FlutterSecureStoragePlatform.instance = TestFlutterSecureStoragePlatform({
      'agent.api_key': 'sk-live',
    });

    await env.container
        .read(chatAgentProvider)
        .sendDirect(dest: kGooseAgentId, text: 'hello');

    expect(bridge.opens, 1);
    expect(bridge.lastOpts?.apiKey, 'sk-live');
    final bodies = env.container
        .read(threadMessagesProvider(kGooseAgentId))
        .items
        .map((m) => m.body)
        .toList();
    expect(bodies, isNot(contains(contains('未配置 API Key'))));
    expect(bodies.last, contains('Goose 调用失败'));
  });

  test('one assistant_finished yields exactly one assistant bubble', () async {
    final bridge = _RecordingAgentBridge()
      ..session = _OneShotSession('hello from goose');
    final env = await kimHarness(
      token: 'tok.jwt',
      account: 'alice',
      overrides: [agentBridgeProvider.overrideWithValue(bridge)],
    );
    FlutterSecureStoragePlatform.instance = TestFlutterSecureStoragePlatform({
      'agent.api_key': 'sk-live',
    });

    await env.container
        .read(chatAgentProvider)
        .sendDirect(dest: kGooseAgentId, text: 'hi');
    await Future<void>.delayed(Duration.zero);

    final items = env.container
        .read(threadMessagesProvider(kGooseAgentId))
        .items;
    final assistant = items
        .where((m) => m.sender == kGooseAgentName)
        .map((m) => m.body)
        .toList();
    expect(assistant, ['hello from goose']);
  });
}
