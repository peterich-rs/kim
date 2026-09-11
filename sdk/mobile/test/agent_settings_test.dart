import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_secure_storage/test/test_flutter_secure_storage_platform.dart';
// ignore: depend_on_referenced_packages
import 'package:flutter_secure_storage_platform_interface/flutter_secure_storage_platform_interface.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/state/agent_settings.dart';
import 'package:shared_preferences/shared_preferences.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  late FlutterSecureStoragePlatform previousPlatform;

  setUp(() {
    previousPlatform = FlutterSecureStoragePlatform.instance;
    SharedPreferences.setMockInitialValues({});
  });

  tearDown(() {
    FlutterSecureStoragePlatform.instance = previousPlatform;
  });

  test('toOpts preserves session-safe fields', () {
    const s = AgentSettings(
      llmBackend: 'openai',
      baseUrl: 'https://example.com/v1',
      model: 'gpt-test',
      apiKey: 'sk-test',
      enableFsTools: false,
      bashEnabled: false,
    );
    final opts = s.toOpts(resumeOnOpen: false);
    expect(opts.llmBackend, 'openai');
    expect(opts.baseUrl, 'https://example.com/v1');
    expect(opts.apiKey, 'sk-test');
    expect(opts.resumeOnOpen, isFalse);
    expect(s.isLive, isTrue);
  });

  test('thinking effort is written to prefs', () async {
    SharedPreferences.setMockInitialValues({});
    FlutterSecureStoragePlatform.instance = TestFlutterSecureStoragePlatform(
      {},
    );
    final container = ProviderContainer.test();
    addTearDown(container.dispose);
    await container
        .read(agentSettingsProvider.notifier)
        .save(
          const AgentSettings(
            llmBackend: 'openai',
            baseUrl: 'https://api.openai.com/v1',
            model: 'gpt-4o',
            apiKey: 'sk-test',
            enableFsTools: false,
            bashEnabled: false,
            thinkingEffort: 'high',
          ),
        );
    final prefs = await SharedPreferences.getInstance();
    expect(prefs.getString('agent.thinking_effort'), 'high');
    await container.read(agentSettingsProvider.notifier).reload();
    expect(container.read(agentSettingsProvider).thinkingEffort, 'high');
  });

  test(
    'first read is empty defaults; ensureLoaded hydrates keychain',
    () async {
      SharedPreferences.setMockInitialValues({
        'agent.llm_backend': 'anthropic',
        'agent.base_url': 'https://api.anthropic.com',
        'agent.model': 'claude-sonnet-4-5',
      });
      FlutterSecureStoragePlatform.instance = TestFlutterSecureStoragePlatform({
        'agent.api_key': 'sk-live',
      });
      final container = ProviderContainer.test();
      addTearDown(container.dispose);

      expect(container.read(agentSettingsProvider).apiKey, isEmpty);
      expect(container.read(agentSettingsProvider).llmBackend, 'openai');

      await container.read(agentSettingsProvider.notifier).ensureLoaded();
      final loaded = container.read(agentSettingsProvider);
      expect(loaded.apiKey, 'sk-live');
      expect(loaded.llmBackend, 'anthropic');
      expect(loaded.baseUrl, 'https://api.anthropic.com');
      expect(loaded.model, 'claude-sonnet-4-5');
    },
  );

  test(
    'ensureLoaded copies agent.api_key into agent.api_key.goose once',
    () async {
      SharedPreferences.setMockInitialValues({});
      final data = <String, String>{'agent.api_key': 'sk-live'};
      FlutterSecureStoragePlatform.instance = TestFlutterSecureStoragePlatform(
        data,
      );
      final container = ProviderContainer.test();
      addTearDown(container.dispose);
      await container.read(agentSettingsProvider.notifier).ensureLoaded();
      expect(container.read(agentSettingsProvider).apiKey, 'sk-live');
      expect(data['agent.api_key'], 'sk-live');
      expect(data['agent.api_key.goose'], 'sk-live');
    },
  );
}
