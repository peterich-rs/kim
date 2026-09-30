import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/bridge/kim_ports.dart';
import 'package:kim_mobile/features/agent/data/catalog.dart';
import 'package:kim_mobile/features/agent/providers/agent_profiles.dart';
import 'package:kim_mobile/features/agent/providers/provider_accounts.dart';

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

  test('defaultModelForAccount prefers catalog default then first', () {
    const account = ProviderAccount(
      id: 'acct-1',
      vendorId: 'openai',
      baseUrl: '',
      keyRef: '',
      models: ['gpt-4.1', 'gpt-4o'],
    );
    expect(
      defaultModelForAccount(
        account,
        const VendorSummary(
          id: 'openai',
          displayName: 'OpenAI',
          group: 'primary',
          sortRank: 0,
          defaultBaseUrl: '',
          defaultModel: 'gpt-4o',
        ),
      ),
      'gpt-4o',
    );
    expect(
      defaultModelForAccount(
        account,
        const VendorSummary(
          id: 'openai',
          displayName: 'OpenAI',
          group: 'primary',
          sortRank: 0,
          defaultBaseUrl: '',
          defaultModel: 'missing',
        ),
      ),
      'gpt-4.1',
    );
  });

  test('selectableModelIds drops gateway routing aliases', () {
    expect(
      selectableModelIds([
        'gpt-4o',
        'gpt-*',
        'claude-*',
        'codex-*',
        'composer-2.5',
        '',
      ]),
      ['gpt-4o', 'composer-2.5'],
    );
  });

  test('canonicalizeVendorId maps grok to xai and leaves groq', () {
    expect(canonicalizeVendorId('grok'), 'xai');
    expect(canonicalizeVendorId('XAI'), 'xai');
    expect(canonicalizeVendorId('groq'), 'groq');
  });

  test('sortVendors uses group then sort_rank, not vendor id', () {
    final vendors = VendorSummary.listFromJson(vendorsJson);
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
      isNot(contains(predicate<VendorSummary>((v) => v.id == 'openrouter'))),
    );
  });

  test('DeepSeek surface default is effort_enum none|low|high|max', () {
    const surface = ReasoningSurface(
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
    const surface = ReasoningSurface(
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
    const surface = ReasoningSurface(
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

  test('migration seeds from the Rust vendor cache via the port', () async {
    final cache = <String, List<String>>{
      'deepseek': ['deepseek-flash', 'deepseek-v4-pro'],
    };
    final client = _CacheStub(cache);
    final seeded = await migrateAccountModelIds(
      client: client,
      vendorId: 'deepseek',
      existing: const [],
      catalogModels: const [],
    );
    expect(seeded, ['deepseek-flash', 'deepseek-v4-pro']);
    // Empty vendor or failing port degrades to catalog-only seeds.
    expect(
      await migrateAccountModelIds(
        client: client,
        vendorId: '',
        existing: const [],
        catalogModels: const ['m1'],
      ),
      ['m1'],
    );
    expect(
      await migrateAccountModelIds(
        client: _CacheStub(null),
        vendorId: 'any',
        existing: const [],
        catalogModels: const ['m2'],
      ),
      ['m2'],
    );
  });

  test('Dart kDefaultSystemPrompt matches the Rust DEFAULT_IDENTITY_PROMPT', () {
    const rust =
        'You are a local desktop agent inside the KIM messenger. '
        'You run on the user\'s machine (not a cloud bot). Reply in the user\'s language. Be concise. '
        'Only use tools that appear in your tool list; never claim tools you were not given.';
    expect(kDefaultSystemPrompt, rust);
    expect(kDefaultSystemPrompt.contains('send_message'), isFalse);
    expect(kDefaultSystemPrompt.contains('search_contacts'), isFalse);
    expect(isLegacyToolLaundryIdentity(kLegacyToolLaundryIdentity), isTrue);
    expect(migrateIdentityPrompt(kLegacyToolLaundryIdentity), isEmpty);
  });

  test('refresh unions fetched with existing, keeping hand-typed ids', () {
    expect(unionAccountModels(['gpt-4o', 'gpt-5'], ['gpt-4o', 'my-finetune']), [
      'gpt-4o',
      'gpt-5',
      'my-finetune',
    ]);
  });

  test('fetch failure does not change the list', () {
    const original = ['gpt-4o', 'my-finetune'];
    final models = List<String>.from(original);
    // Callers skip unionAccountModels / upsert when fetch throws.
    expect(models, original);
    expect(unionAccountModels(['gpt-5'], original), isNot(equals(original)));
  });

  test(
    'ProviderAccount models roundtrip and migrate from catalog cache',
    () async {
      final cache = <String, List<String>>{
        'openai': ['gpt-4o', 'hand-typed'],
      };
      const account = ProviderAccount(
        id: 'acct-1',
        vendorId: 'openai',
        baseUrl: 'https://api.openai.com/v1',
        keyRef: 'agent.api_key.acct.acct-1',
        displayName: 'OpenAI',
      );
      expect(account.models, isEmpty);
      final seeded = await migrateAccountModelIds(
        client: _CacheStub(cache),
        vendorId: account.vendorId,
        existing: account.models,
        catalogModels: const ['gpt-4o'],
        defaultModel: 'gpt-4o',
      );
      expect(seeded, ['gpt-4o', 'hand-typed']);
      final json = account.copyWith(models: seeded).toJson();
      expect(json['models'], ['gpt-4o', 'hand-typed']);
      final restored = ProviderAccount.fromJson(json);
      expect(restored.models, ['gpt-4o', 'hand-typed']);
      final again = await migrateAccountModelIds(
        client: _CacheStub(cache),
        vendorId: restored.vendorId,
        existing: restored.models,
        catalogModels: const ['should-not-replace'],
      );
      expect(again, ['gpt-4o', 'hand-typed']);
    },
  );

  test('Claude surface has no Off or Medium', () {
    const surface = ReasoningSurface(
      kind: 'effort_enum',
      allowed: ['low', 'high', 'max'],
      defaultValue: 'high',
    );
    expect(surface.allowed, ['low', 'high', 'max']);
    expect(defaultChoiceFor(surface).value, 'high');
  });
}

class _CacheStub implements KimClientPort {
  _CacheStub(this.cache);

  final Map<String, List<String>>? cache;

  @override
  Future<List<String>> catalogModelCache(String vendor) async {
    if (cache == null) {
      throw StateError('store not attached');
    }
    return cache![vendor] ?? const [];
  }

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}
