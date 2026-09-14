import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/models/models.dart';
import 'package:kim_mobile/src/rust/api/types.dart';
import 'package:kim_mobile/state/link.dart';

import '../support/harness.dart';
import '../support/jwt.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test('offline snapshot paints Offline on linkProvider', () async {
    final env = await kimHarness(
      token: testJwt(acc: 'alice', exp: 4_000_000_000),
      account: 'alice',
    );
    env.container.read(linkProvider);
    env.fake.pushSnapshot(
      const SessionSnapshotDto(
        link: LinkStateDto.offline(),
        threads: [],
        unreadTotal: 0,
      ),
    );
    await Future<void>.delayed(Duration.zero);
    expect(env.container.read(linkProvider).status, ConnStatus.offline);
  });

  test('retry only notifyRadioUp when session already started', () async {
    final env = await kimHarness(
      token: testJwt(acc: 'alice', exp: 4_000_000_000),
      account: 'alice',
    );
    env.container.read(linkProvider);
    await env.container.read(linkProvider.notifier).retry();
    expect(env.fake.radioUps, greaterThan(0));
  });

  test('connect failure stays offline via snapshot', () async {
    final env = await kimHarness(
      token: testJwt(acc: 'alice', exp: 4_000_000_000),
      account: 'alice',
    );
    env.fake.connectError = Exception('boom');
    env.container.read(linkProvider);
    await Future<void>.delayed(const Duration(milliseconds: 20));
    env.fake.pushSnapshot(
      const SessionSnapshotDto(
        link: LinkStateDto.offline(),
        lastError: 'boom',
        threads: [],
        unreadTotal: 0,
      ),
    );
    await Future<void>.delayed(Duration.zero);
    expect(env.container.read(linkProvider).status, ConnStatus.offline);
  });
}
