import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/agent/catalog.dart';
import 'package:kim_mobile/state/agent_profiles.dart';
import 'package:shared_preferences/shared_preferences.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  const vendorsJson = '''
[
  {"id":"openrouter","display_name":"OpenRouter","group":"gateway","sort_rank":100,"default_base_url":"https://openrouter.ai/api/v1","alt_base_urls":[],"dynamic_models":true,"custom_model":true,"default_model":"","models":[]},
  {"id":"deepseek","display_name":"DeepSeek","group":"primary","sort_rank":30,"default_base_url":"https://api.deepseek.com","alt_base_urls":[],"dynamic_models":true,"custom_model":true,"default_model":"deepseek-flash","models":["deepseek-flash","deepseek-v4-pro"]},
  {"id":"openai","display_name":"OpenAI","group":"primary","sort_rank":10,"default_base_url":"https://api.openai.com/v1","alt_base_urls":[],"dynamic_models":true,"custom_model":true,"default_model":"gpt-4o","models":["gpt-4o"]},
  {"id":"groq","display_name":"Groq","group":"other","sort_rank":80,"default_base_url":"https://api.groq.com/openai/v1","alt_base_urls":[],"dynamic_models":true,"custom_model":true,"default_model":"","models":[]}
]
''';

  test('sortVendors uses group then sort_rank, not vendor id', () {
    final vendors = VendorSummaryDto.listFromJson(vendorsJson);
    final sorted = sortVendors(vendors);
    expect(sorted.map((v) => v.id).toList(), [
      'openai',
      'deepseek',
      'openrouter',
      'groq',
    ]);
    expect(vendorsInGroup(vendors, 'primary').map((v) => v.id).toList(), [
      'openai',
      'deepseek',
    ]);
    expect(vendorsInGroup(vendors, 'gateway').single.id, 'openrouter');
    expect(
      vendorsInGroup(vendors, 'primary'),
      isNot(contains(predicate<VendorSummaryDto>((v) => v.id == 'openrouter'))),
    );
  });

  test('DeepSeek surface default is effort_enum none|low|high|max', () {
    const surface = ReasoningSurfaceDto(
      kind: 'effort_enum',
      allowed: ['none', 'low', 'high', 'max'],
      defaultValue: 'high',
    );
    final choice = defaultChoiceFor(surface);
    expect(choice.kind, 'effort_enum');
    expect(choice.value, 'high');
    expect(surface.allowed, ['none', 'low', 'high', 'max']);
    expect(surface.allowed, isNot(contains('medium')));
    expect(surface.allowed, isNot(contains('off')));
  });

  test('alignChoice drops unsupported effort onto surface default', () {
    const surface = ReasoningSurfaceDto(
      kind: 'effort_enum',
      allowed: ['none', 'low', 'high', 'max'],
      defaultValue: 'high',
    );
    final aligned = alignChoice(
      surface,
      const ReasoningChoice(kind: 'effort_enum', value: 'medium'),
    );
    expect(aligned.dropped, isTrue);
    expect(aligned.choice.value, 'high');
  });

  test('alignChoice keeps a value in the allowed list', () {
    const surface = ReasoningSurfaceDto(
      kind: 'effort_enum',
      allowed: ['none', 'low', 'high', 'max'],
      defaultValue: 'high',
    );
    final aligned = alignChoice(
      surface,
      const ReasoningChoice(kind: 'effort_enum', value: 'none'),
    );
    expect(aligned.dropped, isFalse);
    expect(aligned.choice.value, 'none');
  });

  test('custom endpoint URL allows https and loopback http only', () {
    expect(isAllowedAgentBaseUrl('https://vllm.example/v1'), isTrue);
    expect(isAllowedAgentBaseUrl('http://127.0.0.1:8000/v1'), isTrue);
    expect(isAllowedAgentBaseUrl('http://localhost:8080/v1'), isTrue);
    expect(isAllowedAgentBaseUrl('http://evil.example/v1'), isFalse);
    expect(isAllowedAgentBaseUrl('not-a-url'), isFalse);
  });

  test('catalog_validate result drops secret keys from Advanced JSON', () {
    const raw =
        '{"choice":{"v":1,"kind":"advanced","json":{"enable_thinking":true}},"dropped":["api_key"]}';
    final result = CatalogValidateResult.fromJsonString(raw);
    expect(result.dropped, ['api_key']);
    expect(result.choice.kind, 'advanced');
    expect(result.choice.advanced!['enable_thinking'], isTrue);
    expect(result.choice.advanced!.containsKey('api_key'), isFalse);
    expect(raw.contains('sk-'), isFalse);
  });

  test('fetch model cache roundtrips under agent.catalog_cache', () async {
    SharedPreferences.setMockInitialValues({});
    await saveCatalogModelCache('deepseek', const [
      'deepseek-flash',
      'deepseek-v4-pro',
    ]);
    final prefs = await SharedPreferences.getInstance();
    expect(prefs.getString('agent.catalog_cache.deepseek'), isNotEmpty);
    expect(await loadCatalogModelCache('deepseek'), [
      'deepseek-flash',
      'deepseek-v4-pro',
    ]);
  });

  test('Claude surface has no Off or Medium', () {
    const surface = ReasoningSurfaceDto(
      kind: 'effort_enum',
      allowed: ['low', 'high', 'max'],
      defaultValue: 'high',
    );
    expect(surface.allowed, ['low', 'high', 'max']);
    expect(defaultChoiceFor(surface).value, 'high');
  });
}
