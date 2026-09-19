import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/features/agent/agent_profiles.dart';
import 'package:kim_mobile/features/agent/context_window.dart';
import 'package:kim_mobile/features/agent/context_window_controls.dart';
import 'package:kim_mobile/l10n/app_localizations.dart';

void main() {
  test('defaultContextTokens uses published windows', () {
    expect(defaultContextTokens('gpt-4o'), 128000);
    expect(defaultContextTokens('openai/gpt-4o-mini'), 128000);
    expect(defaultContextTokens('gpt-4.1'), 1047576);
    expect(defaultContextTokens('gpt-5'), 400000);
    expect(defaultContextTokens('gpt-5.6-sol'), 1050000);
    expect(defaultContextTokens('claude-sonnet-4-5'), 200000);
    expect(defaultContextTokens('claude-sonnet-4-6'), 1000000);
    expect(defaultContextTokens('claude-opus-5'), 1000000);
    expect(defaultContextTokens('grok-4.6'), 500000);
    expect(defaultContextTokens('deepseek-flash'), 1000000);
    expect(defaultContextTokens('deepseek-v4-pro'), 1000000);
    expect(defaultContextTokens('kimi-k2.6'), 262144);
    expect(defaultContextTokens('moonshotai/kimi-k3'), 1048576);
    expect(defaultContextTokens('qwen3.8-max'), 1000000);
    expect(defaultContextTokens('glm-5.2'), 1000000);
    expect(defaultContextTokens('MiniMax-M3'), 1000000);
    expect(defaultContextTokens('local-mystery'), kDefaultContextTokens);
    expect(defaultContextTokens(''), kDefaultContextTokens);
  });

  test('parseContextTokens accepts k/m suffixes', () {
    expect(parseContextTokens('128000'), 128000);
    expect(parseContextTokens('128k'), 128000);
    expect(parseContextTokens('1M'), 1000000);
    expect(parseContextTokens('1.05m'), 1050000);
    expect(parseContextTokens('0'), isNull);
    expect(parseContextTokens('abc'), isNull);
  });

  test('profile json round-trips context_tokens', () {
    const profile = AgentProfile(
      id: 'p-1',
      displayName: 'Work',
      providerKind: 'openai',
      baseUrl: '',
      model: 'gpt-4o',
      keyRef: '',
      systemPrompt: '',
      contextTokens: 128000,
    );
    final decoded = AgentProfile.fromJson(profile.toJson());
    expect(decoded.contextTokens, 128000);
    expect(decoded.toJson()['model'], containsPair('context_tokens', 128000));
  });

  test('missing context_tokens stays unset until the editor fills it', () {
    final decoded = AgentProfile.fromJson({
      'id': 'p-1',
      'display_name': 'Work',
      'model': {'name': 'gpt-4o'},
      'system_prompt': '',
    });
    expect(decoded.contextTokens, isNull);
  });

  testWidgets('picker labels the model default', (tester) async {
    await tester.pumpWidget(
      MaterialApp(
        locale: const Locale('zh'),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        home: Scaffold(
          body: ContextWindowControls(
            tokens: 128000,
            model: 'gpt-4o',
            onChanged: (_) {},
          ),
        ),
      ),
    );
    expect(find.byKey(const Key('agent-context')), findsOneWidget);
    expect(find.text('128K（模型默认）'), findsOneWidget);
  });
}
