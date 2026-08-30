import 'dart:io';

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

  test('sendImages uploads then talks type=2', () async {
    final env = await kimHarness(token: 'tok.jwt', account: 'alice');
    env.fake.friends = const [KimPerson(account: 'bob', nickname: 'Bobby')];
    await env.container.read(gatewayProvider.future);
    await env.container.read(contactsProvider.notifier).refresh();
    final file = File(
      '${Directory.systemTemp.path}/kim-img-${DateTime.now().microsecondsSinceEpoch}.jpg',
    );
    await file.writeAsBytes(const [0xFF, 0xD8, 0xFF, 0xD9]);
    addTearDown(() {
      if (file.existsSync()) {
        file.deleteSync();
      }
    });
    final rows = await env.container.read(inboxProvider.notifier).sendImages(
      'bob',
      [
        KimMediaAsset(
          id: 'a',
          path: file.path,
          width: 100,
          height: 80,
          size: 4,
          mimeType: 'image/jpeg',
        ),
      ],
    );
    expect(rows, hasLength(1));
    expect(rows.single.isImage, isTrue);
    expect(env.fake.talks, 0);
    expect(env.fake.imageTalks, 1);
    expect(env.fake.lastImageUrl, 'https://media.kim.ainexc.com/alice/a.jpg');
    expect(env.fake.lastImageExtra, '{"w":100,"h":80}');
    expect(
      env.container.read(inboxProvider).messages['bob']!.single.body,
      'https://media.kim.ainexc.com/alice/a.jpg',
    );
    expect(
      env.container.read(inboxProvider).threads.single.lastBody,
      Copy.imageMessage,
    );
  });

  test('incoming media URL is stored as an image row', () async {
    final env = await kimHarness(token: 'tok.jwt', account: 'alice');
    env.container
        .read(inboxProvider.notifier)
        .receive(
          KimEvent(
            kind: KimEventKind.talk,
            dest: 'bob',
            sender: 'bob',
            body: 'https://media.kim.ainexc.com/bob/a.png',
            extra: '{"w":10,"h":8}',
            messageId: 9,
            sendTime: 1_700_000_000_000,
          ),
        );
    final row = env.container.read(inboxProvider).messages['bob']!.single;
    expect(row.isImage, isTrue);
    expect(row.width, 10);
    expect(row.height, 8);
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
