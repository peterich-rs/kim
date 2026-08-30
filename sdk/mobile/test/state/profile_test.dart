import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/models/models.dart';
import 'package:kim_mobile/state/gateway.dart';
import 'package:kim_mobile/state/profile.dart';

import '../support/harness.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test('applyAvatar updates the cached profile', () async {
    final env = await kimHarness(token: 'tok.jwt', account: 'alice');
    env.fake.friends = const [KimPerson(account: 'bob', nickname: 'Bobby')];
    await env.container.read(gatewayProvider.future);
    await env.container
        .read(profileProvider.notifier)
        .applyAvatar('https://media.kim.ainexc.com/alice/a.jpg');
    expect(env.fake.lastAvatar, 'https://media.kim.ainexc.com/alice/a.jpg');
    expect(
      env.container.read(profileProvider).avatar,
      'https://media.kim.ainexc.com/alice/a.jpg',
    );
    expect(
      env.runtime.settings.avatar,
      'https://media.kim.ainexc.com/alice/a.jpg',
    );
  });
}
