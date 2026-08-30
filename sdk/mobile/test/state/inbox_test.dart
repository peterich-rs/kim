import 'package:flutter_test/flutter_test.dart';
import 'package:kim_media_picker/kim_media_picker.dart';
import 'package:kim_mobile/copy.dart';
import 'package:kim_mobile/models/models.dart';
import 'package:kim_mobile/state/contacts.dart';
import 'package:kim_mobile/state/gateway.dart';
import 'package:kim_mobile/state/inbox.dart';

import '../support/harness.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test('send talks over the client after login', () async {
    final env = await kimHarness(token: 'tok.jwt', account: 'alice');
    env.fake.friends = const [KimPerson(account: 'bob', nickname: 'Bobby')];
    await env.container.read(gatewayProvider.future);
    await env.container.read(contactsProvider.notifier).refresh();
    final msg = await env.container
        .read(inboxProvider.notifier)
        .send('bob', 'hello');
    expect(msg.body, 'hello');
    expect(env.fake.talks, 1);
    expect(env.fake.lastTalkDest, 'bob');
    expect(
      env.container.read(inboxProvider).threads.map((t) => t.id),
      contains('bob'),
    );
  });

  test('send while offline throws not connected', () async {
    final env = await kimHarness(
      token: 'tok.jwt',
      account: 'alice',
      online: false,
    );
    await expectLater(
      env.container.read(inboxProvider.notifier).send('bob', 'hello'),
      throwsA(
        isA<StateError>().having(
          (e) => e.message,
          'message',
          Copy.notConnected,
        ),
      ),
    );
    expect(env.fake.talks, 0);
  });

  test('sendImages stores image rows and does not talk text', () async {
    final env = await kimHarness(token: 'tok.jwt', account: 'alice');
    env.fake.friends = const [KimPerson(account: 'bob', nickname: 'Bobby')];
    await env.container.read(gatewayProvider.future);
    await env.container.read(contactsProvider.notifier).refresh();
    final rows = await env.container.read(inboxProvider.notifier).sendImages(
      'bob',
      const [
        KimMediaAsset(
          id: 'a',
          path: '/tmp/a.jpg',
          width: 100,
          height: 80,
          size: 12,
          mimeType: 'image/jpeg',
        ),
      ],
    );
    expect(rows, hasLength(1));
    expect(rows.single.isImage, isTrue);
    expect(rows.single.body, '/tmp/a.jpg');
    expect(env.fake.talks, 0);
    expect(
      env.container.read(inboxProvider).threads.single.lastBody,
      Copy.imageMessage,
    );
  });

  test('sendImages stores a video row as [视频]', () async {
    final env = await kimHarness(token: 'tok.jwt', account: 'alice');
    env.fake.friends = const [KimPerson(account: 'bob', nickname: 'Bobby')];
    await env.container.read(gatewayProvider.future);
    await env.container.read(contactsProvider.notifier).refresh();
    final rows = await env.container.read(inboxProvider.notifier).sendImages(
      'bob',
      const [
        KimMediaAsset(
          id: 'v',
          path: '/tmp/a.mp4',
          width: 1280,
          height: 720,
          size: 99,
          mimeType: 'video/mp4',
          durationMs: 1200,
        ),
      ],
    );
    expect(rows.single.isVideo, isTrue);
    expect(
      env.container.read(inboxProvider).threads.single.lastBody,
      Copy.videoMessage,
    );
  });
}
