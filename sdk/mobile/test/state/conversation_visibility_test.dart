import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/features/chats/providers/conversation_visibility.dart';
import 'package:kim_mobile/features/session/location.dart';
import 'package:kim_mobile/models/models.dart';

import '../support/harness.dart';
import '../support/jwt.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test('chat path reports visible dest and generation', () async {
    final env = await kimHarness(
      token: testJwt(acc: 'alice', exp: 4_000_000_000),
      account: 'alice',
    );
    final sub = env.container.listen(conversationVisibilityProvider, (_, _) {});
    addTearDown(sub.close);
    env.container.read(locationProvider.notifier).setPath('/chat/bob');
    await Future<void>.delayed(Duration.zero);
    expect(env.fake.visibilityDest, 'bob');
    expect(env.fake.visibilityForeground, isTrue);
    expect(env.fake.visibilityKind, ThreadKind.user);
    expect(env.fake.visibilityGeneration, greaterThan(0));

    env.container.read(locationProvider.notifier).setPath('/');
    await Future<void>.delayed(Duration.zero);
    expect(env.fake.visibilityDest, isEmpty);
  });
}
