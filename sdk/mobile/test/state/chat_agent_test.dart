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

  @override
  Future<void> ensure() async {}

  @override
  Future<AgentSession> open({
    required String sqlitePath,
    required String projectRoot,
    required SessionOpenOpts opts,
  }) async {
    opens += 1;
    lastOpts = opts;
    throw StateError('scripted: no live host');
  }
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
}
