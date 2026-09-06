import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/models/models.dart';
import 'package:kim_mobile/state/presence.dart';

void main() {
  test('presenceProvider applies snapshot and push', () {
    final container = ProviderContainer();
    addTearDown(container.dispose);

    expect(
      container.read(peerPresenceProvider('bob')),
      PeerPresenceStatus.unknown,
    );

    container.read(presenceProvider.notifier).applySnapshot([
      {'account': 'bob', 'status': 2, 'lastSeen': 0},
    ]);
    expect(
      container.read(peerPresenceProvider('bob')),
      PeerPresenceStatus.online,
    );

    container
        .read(presenceProvider.notifier)
        .applyPush(account: 'bob', status: 1, lastSeen: 123);
    expect(
      container.read(peerPresenceProvider('bob')),
      PeerPresenceStatus.offline,
    );
    expect(container.read(presenceProvider).lastSeenMs['bob'], 123);
  });
}
