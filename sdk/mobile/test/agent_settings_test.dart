import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/state/agent_settings.dart';

void main() {
  test('toOpts preserves session-safe fields', () {
    const s = AgentSettings(
      llmBackend: 'responses_http',
      baseUrl: 'https://example.com/v1',
      model: 'gpt-test',
      apiKey: 'sk-test',
      enableFsTools: true,
      bashEnabled: false,
    );
    final opts = s.toOpts(resumeOnOpen: false);
    expect(opts.llmBackend, 'responses_http');
    expect(opts.baseUrl, 'https://example.com/v1');
    expect(opts.apiKey, 'sk-test');
    expect(opts.resumeOnOpen, isFalse);
    expect(s.isLive, isTrue);
  });
}
