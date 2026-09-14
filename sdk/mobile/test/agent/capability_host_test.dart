import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/agent/capability_host.dart';

import '../support/harness.dart';
import '../support/jwt.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test('execute returns unavailable without reading store', () async {
    final env = await kimHarness(
      token: testJwt(acc: 'alice', exp: 4_000_000_000),
      account: 'alice',
    );
    final host = env.container.read(Provider(KimCapabilityHost.new));
    final raw = await host.execute(
      name: 'search_messages',
      argumentsJson: '{"query":"x"}',
      sessionDest: 'bob',
      profileId: 'p1',
    );
    expect(raw, '{"error":"unavailable"}');
  });
}
