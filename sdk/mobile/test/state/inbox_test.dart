import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:kim_media_picker/kim_media_picker.dart';
import 'package:kim_mobile/copy.dart';
import 'package:kim_mobile/models/models.dart';
import 'package:kim_mobile/state/contacts.dart';
import 'package:kim_mobile/core/format.dart';
import 'package:kim_mobile/state/inbox.dart';
import 'package:kim_mobile/state/link.dart';
import 'package:kim_mobile/state/location.dart';
import 'package:kim_mobile/state/messages.dart';
import 'package:kim_mobile/state/outbox.dart';
import 'package:kim_mobile/state/providers.dart';

import '../support/fake_kim.dart';
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

  test('send while socket offline still enqueues', () async {
    final env = await kimHarness(token: 'tok.jwt', account: 'alice');
    env.fake.connectError = Exception('offline');
    await _online(env);
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

  test('inbox lastBody that is a media URL shows as [图片]', () async {
    final env = await kimHarness(token: 'tok.jwt', account: 'alice');
    env.container.read(threadsProvider.notifier).mergeInbox(const [
      KimThread(
        id: 'bob',
        kind: ThreadKind.user,
        title: 'bob',
        lastBody: 'https://media.kim.ainexc.com/alice/a.jpg',
        lastAt: 9,
      ),
    ]);
    expect(
      env.container.read(threadsProvider).threads.single.lastBody,
      Copy.imageMessage,
    );
    await Future<void>.delayed(Duration.zero);
    expect(env.store.loadThreads('alice').single.lastBody, Copy.imageMessage);
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

  test('talk sendTime is stored as milliseconds', () async {
    final env = await kimHarness(token: 'tok.jwt', account: 'alice');
    env.fake.friends = const [KimPerson(account: 'bob', nickname: 'Bobby')];
    env.fake.talkSendTime = 1788077118498491646;
    await _online(env);
    await env.container.read(contactsProvider.notifier).refresh();
    await env.container.read(outboxProvider.notifier).sendText('bob', 'hello');
    await Future<void>.delayed(const Duration(milliseconds: 20));
    final stored = env.container
        .read(threadMessagesProvider('bob'))
        .items
        .single;
    expect(stored.at, sendTimeMs(1788077118498491646));
    expect(stored.at, lessThanOrEqualTo(kDateTimeMsMax));
  });

  test(
    'viewing a thread clears unread and keeps inbox merge from restoring it',
    () async {
      final env = await kimHarness(token: 'tok.jwt', account: 'alice');
      env.container.read(threadsProvider.notifier).mergeInbox(const [
        KimThread(
          id: 'bob',
          kind: ThreadKind.user,
          title: 'bob',
          lastBody: 'hi',
          lastAt: 9,
          unread: 3,
        ),
      ]);
      expect(env.container.read(threadsProvider).threads.single.unread, 3);
      env.container.read(locationProvider.notifier).setPath('/chat/bob');
      env.container.read(threadsProvider.notifier).markRead('bob');
      expect(env.container.read(threadsProvider).threads.single.unread, 0);
      env.container.read(threadsProvider.notifier).mergeInbox(const [
        KimThread(
          id: 'bob',
          kind: ThreadKind.user,
          title: 'bob',
          lastBody: 'hi',
          lastAt: 9,
          unread: 3,
        ),
      ]);
      expect(env.container.read(threadsProvider).threads.single.unread, 0);
    },
  );

  test('incoming talk while viewing stays read on the list', () async {
    final env = await kimHarness(token: 'tok.jwt', account: 'alice');
    await _online(env);
    env.container.read(locationProvider.notifier).setPath('/chat/bob');
    env.fake.emitTalk(
      dest: 'bob',
      sender: 'bob',
      body: 'hello',
      sendTime: 1_700_000_000_000,
      messageId: 11,
    );
    await Future<void>.delayed(Duration.zero);
    expect(env.container.read(threadsProvider).threads.single.unread, 0);
    expect(env.fake.reads, 1);
    expect(env.fake.lastReadDest, 'bob');
    expect(env.fake.lastReadMessageId, 11);
  });

  test('replaying the same messageId does not increase unread', () async {
    final env = await kimHarness(token: 'tok.jwt', account: 'alice');
    await _online(env);
    env.fake.emitTalk(
      dest: 'bob',
      sender: 'bob',
      body: 'hello',
      sendTime: 1_700_000_000_000,
      messageId: 9,
    );
    await Future<void>.delayed(const Duration(milliseconds: 20));
    expect(env.container.read(threadsProvider).threads.single.unread, 1);
    env.fake.emitTalk(
      dest: 'bob',
      sender: 'bob',
      body: 'hello',
      sendTime: 1_700_000_000_000,
      messageId: 9,
    );
    await Future<void>.delayed(const Duration(milliseconds: 20));
    expect(env.container.read(threadsProvider).threads.single.unread, 1);
    expect(
      env.container.read(threadMessagesProvider('bob')).items,
      hasLength(1),
    );
  });

  test('failed image send after upload retries without uploading again', () async {
    final env = await kimHarness(token: 'tok.jwt', account: 'alice');
    env.fake.friends = const [KimPerson(account: 'bob', nickname: 'Bobby')];
    env.fake.talkError = Exception('offline');
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
    final media = env.container.read(mediaPortProvider) as FakeKimMedia;
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
    await Future<void>.delayed(const Duration(milliseconds: 40));
    expect(media.uploads, 1);
    expect(
      env.container.read(threadMessagesProvider('bob')).items.single.body,
      media.url,
    );
    expect(
      env.container.read(threadMessagesProvider('bob')).items.single.isFailed,
      isTrue,
    );
    env.fake.talkError = null;
    await env.container
        .read(outboxProvider.notifier)
        .retry('bob', rows.single.key);
    await Future<void>.delayed(const Duration(milliseconds: 40));
    expect(media.uploads, 1);
    expect(env.fake.imageTalks, 2);
    expect(
      env.container.read(threadMessagesProvider('bob')).items.single.status,
      KimSendStatus.sent,
    );
  });

  test('reconcile of a full remote page keeps hasMore true', () async {
    final env = await kimHarness(token: 'tok.jwt', account: 'alice');
    env.fake.historyRows = [
      for (var i = 50; i >= 1; i--)
        KimHistoryMsg(
          messageId: i,
          msgType: 1,
          body: '$i',
          extra: '',
          sender: 'bob',
          sendTime: 1_700_000_000 + i,
          direction: 0,
        ),
    ];
    await env.container
        .read(threadMessagesProvider('bob').notifier)
        .reconcile();
    expect(env.container.read(threadMessagesProvider('bob')).hasMore, isTrue);
    expect(
      env.container.read(threadMessagesProvider('bob')).items,
      hasLength(50),
    );
  });

  test('sync page persists then confirms the page id', () async {
    final env = await kimHarness(token: 'tok.jwt', account: 'alice');
    await _online(env);
    env.fake.emitSyncPage(
      pageId: 42,
      talks: const [
        KimEvent(
          kind: KimEventKind.talk,
          dest: 'bob',
          sender: 'bob',
          body: 'later',
          messageId: 42,
          sendTime: 1_700_000_000_000,
        ),
      ],
    );
    await Future<void>.delayed(const Duration(milliseconds: 20));
    expect(env.fake.confirms, 1);
    expect(env.fake.lastConfirm, 42);
    expect(
      env.container.read(threadMessagesProvider('bob')).items.single.messageId,
      42,
    );
    expect(env.store.loadMessages('alice', 'bob'), hasLength(1));
  });
}
