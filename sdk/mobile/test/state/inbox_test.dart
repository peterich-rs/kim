import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:kim_media_picker/kim_media_picker.dart';
import 'package:kim_mobile/copy.dart';
import 'package:kim_mobile/models/models.dart';
import 'package:kim_mobile/state/contacts.dart';
import 'package:kim_mobile/state/inbox.dart';
import 'package:kim_mobile/state/link.dart';
import 'package:kim_mobile/state/messages.dart';
import 'package:kim_mobile/state/outbox.dart';

import '../support/harness.dart';

Future<void> _online(dynamic env) async {
  env.container.read(linkProvider);
  await Future<void>.delayed(Duration.zero);
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test('send talks over the client after login', () async {
    final env = await kimHarness(token: 'tok.jwt', account: 'alice');
    env.fake.friends = const [KimPerson(account: 'bob', nickname: 'Bobby')];
    await _online(env);
    await env.container.read(contactsProvider.notifier).refresh();
    final msg = await env.container
        .read(outboxProvider.notifier)
        .sendText('bob', 'hello');
    await Future<void>.delayed(Duration.zero);
    expect(msg.body, 'hello');
    expect(env.fake.talks, 1);
    expect(env.fake.lastTalkDest, 'bob');
    expect(env.fake.lastClientId, msg.key);
    expect(
      env.container.read(threadsProvider).threads.map((t) => t.id),
      contains('bob'),
    );
  });

  test('send while offline still enqueues', () async {
    final env = await kimHarness(
      token: 'tok.jwt',
      account: 'alice',
      online: false,
    );
    final msg = await env.container
        .read(outboxProvider.notifier)
        .sendText('bob', 'hello');
    expect(msg.body, 'hello');
    expect(msg.status, KimSendStatus.sending);
    expect(env.fake.talks, 0);
  });

  test('sendImages uploads then talks type=2', () async {
    final env = await kimHarness(token: 'tok.jwt', account: 'alice');
    env.fake.friends = const [KimPerson(account: 'bob', nickname: 'Bobby')];
    await _online(env);
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
    final rows = await env.container.read(outboxProvider.notifier).sendImages(
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
    await Future<void>.delayed(const Duration(milliseconds: 20));
    expect(rows, hasLength(1));
    expect(rows.single.isImage, isTrue);
    expect(env.fake.talks, 0);
    expect(env.fake.imageTalks, 1);
    expect(env.fake.lastImageUrl, 'https://media.kim.ainexc.com/alice/a.jpg');
    expect(env.fake.lastImageExtra, '{"w":100,"h":80}');
    expect(
      env.container.read(threadMessagesProvider('bob')).items.single.body,
      'https://media.kim.ainexc.com/alice/a.jpg',
    );
    expect(
      env.container.read(threadsProvider).threads.single.lastBody,
      Copy.imageMessage,
    );
  });

  test('incoming media URL is stored as an image row', () async {
    final env = await kimHarness(token: 'tok.jwt', account: 'alice');
    await _online(env);
    env.fake.emitTalk(
      dest: 'bob',
      sender: 'bob',
      body: 'https://media.kim.ainexc.com/bob/a.png',
      extra: '{"w":10,"h":8}',
      sendTime: 1_700_000_000_000,
      messageId: 9,
    );
    await Future<void>.delayed(Duration.zero);
    final row = env.container.read(threadMessagesProvider('bob')).items.single;
    expect(row.isImage, isTrue);
    expect(row.width, 10);
    expect(row.height, 8);
    expect(
      env.container.read(threadsProvider).threads.single.lastBody,
      Copy.imageMessage,
    );
  });

  test('sendImages stores a video row as [视频]', () async {
    final env = await kimHarness(token: 'tok.jwt', account: 'alice');
    env.fake.friends = const [KimPerson(account: 'bob', nickname: 'Bobby')];
    await _online(env);
    await env.container.read(contactsProvider.notifier).refresh();
    final rows = await env.container.read(outboxProvider.notifier).sendImages(
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
    await Future<void>.delayed(const Duration(milliseconds: 20));
    expect(rows.single.isVideo, isTrue);
    expect(
      env.container.read(threadsProvider).threads.single.lastBody,
      Copy.videoMessage,
    );
  });

  test('outbox keeps client_id stable across retry', () async {
    final env = await kimHarness(token: 'tok.jwt', account: 'alice');
    env.fake.friends = const [KimPerson(account: 'bob', nickname: 'Bobby')];
    await _online(env);
    await env.container.read(contactsProvider.notifier).refresh();
    env.fake.talkError = Exception('offline');
    final msg = await env.container
        .read(outboxProvider.notifier)
        .sendText('bob', 'hello');
    await Future<void>.delayed(const Duration(milliseconds: 20));
    expect(env.fake.lastClientId, msg.key);
    expect(
      env.container.read(threadMessagesProvider('bob')).items.single.isFailed,
      isTrue,
    );
    env.fake.talkError = null;
    await env.container.read(outboxProvider.notifier).retry('bob', msg.key);
    await Future<void>.delayed(const Duration(milliseconds: 20));
    expect(env.fake.clientIds, [msg.key, msg.key]);
    expect(
      env.container.read(threadMessagesProvider('bob')).items.single.status,
      KimSendStatus.sent,
    );
  });
}
