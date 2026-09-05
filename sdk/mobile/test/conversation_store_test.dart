import 'dart:convert';
import 'dart:io';

import 'package:connectivity_plus/connectivity_plus.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/copy.dart';
import 'package:kim_mobile/core/connectivity.dart';
import 'package:kim_mobile/core/format.dart';
import 'package:kim_mobile/data/conversation_store.dart';
import 'package:kim_mobile/models/models.dart';
import 'package:shared_preferences/shared_preferences.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  ConversationStore store() {
    final db = ConversationStore.memory();
    addTearDown(db.close);
    return db;
  }

  test('networkLinkUp treats none as offline and wifi as online', () {
    expect(networkLinkUp(const [ConnectivityResult.none]), isFalse);
    expect(networkLinkUp(const []), isFalse);
    expect(networkLinkUp(const [ConnectivityResult.wifi]), isTrue);
    expect(networkLinkUp(const [ConnectivityResult.mobile]), isTrue);
    expect(networkLinkUp(const [ConnectivityResult.other]), isTrue);
    expect(networkLinkUp(const [ConnectivityResult.bluetooth]), isFalse);
    expect(loopbackUnreachableOnThisDevice('wss://kim.ainexc.com/'), isFalse);
    expect(loopbackUnreachableOnThisDevice('ws://127.0.0.1:8001/'), isFalse);
  });

  test('threads persist per account', () async {
    final db = store();
    await db.saveThreads('alice', [
      const KimThread(
        id: 'bob',
        kind: ThreadKind.user,
        title: 'bob',
        lastBody: 'hi',
        lastAt: 9,
      ),
    ]);
    expect(db.loadThreads('alice').single.id, 'bob');
    expect(db.loadThreads('carol'), isEmpty);
  });

  test('sendTimeMs treats unix seconds as seconds', () {
    expect(sendTimeMs(1_700_000_000), 1_700_000_000_000);
    expect(sendTimeMs(1_700_000_000_000), 1_700_000_000_000);
  });

  test('sendTimeMs converts nanosecond wire times into DateTime range', () {
    const nano = 1788077118498491646;
    final ms = sendTimeMs(nano, now: 1_700_000_000_000);
    expect(ms, lessThanOrEqualTo(kDateTimeMsMax));
    expect(ms, greaterThanOrEqualTo(kDateTimeMsMin));
    expect(dateTimeFromEpoch(nano), isNotNull);
    expect(() => formatListTime(nano), returnsNormally);
    expect(formatListTime(nano), isNotEmpty);
  });

  test('messages clip and survive reload', () async {
    final db = store();
    await db.saveMessages('alice', 'bob', [
      const KimChatMsg(
        key: '1',
        dest: 'bob',
        sender: 'alice',
        body: 'hello',
        at: 1,
      ),
    ]);
    expect(db.loadMessages('alice', 'bob').single.body, 'hello');
  });

  test('image rows keep kind and size', () async {
    final db = store();
    await db.saveMessages('alice', 'bob', [
      const KimChatMsg(
        key: '1',
        dest: 'bob',
        sender: 'alice',
        body: 'https://media.kim.ainexc.com/a.png',
        at: 1,
        kind: KimMsgKind.image,
        width: 120,
        height: 80,
      ),
    ]);
    final row = db.loadMessages('alice', 'bob').single;
    expect(row.isImage, isTrue);
    expect(row.width, 120);
    expect(row.height, 80);
  });

  test('deleteThread drops the row and messages', () async {
    final db = store();
    await db.saveThreads('alice', [
      const KimThread(id: 'bob', kind: ThreadKind.user, title: 'bob'),
    ]);
    await db.saveMessages('alice', 'bob', [
      const KimChatMsg(
        key: '1',
        dest: 'bob',
        sender: 'alice',
        body: 'hello',
        at: 1,
      ),
    ]);
    await db.deleteThread('alice', 'bob');
    expect(db.loadThreads('alice'), isEmpty);
    expect(db.loadMessages('alice', 'bob'), isEmpty);
  });

  test('imports SharedPreferences JSON once', () async {
    SharedPreferences.setMockInitialValues({
      'kim.threads.alice': jsonEncode([
        {
          'id': 'bob',
          'kind': 'user',
          'title': 'bob',
          'lastBody': 'hi',
          'lastAt': 9,
          'unread': 1,
        },
      ]),
      'kim.msgs.alice.bob': jsonEncode([
        {
          'key': '1',
          'dest': 'bob',
          'sender': 'alice',
          'body': 'hi',
          'at': 9,
          'sys': false,
          'failed': false,
          'kind': 'text',
          'width': 0,
          'height': 0,
        },
      ]),
    });
    final tmp = Directory.systemTemp.createTempSync('kim-cache-');
    addTearDown(() {
      if (tmp.existsSync()) {
        tmp.deleteSync(recursive: true);
      }
    });
    final prefs = await SharedPreferences.getInstance();
    final db = await ConversationStore.open(
      support: tmp,
      prefs: prefs,
      isolate: false,
    );
    addTearDown(db.close);
    expect(db.loadThreads('alice').single.id, 'bob');
    expect(db.loadMessages('alice', 'bob').single.body, 'hi');
    await prefs.setString(
      'kim.threads.alice',
      jsonEncode([
        {'id': 'carol', 'kind': 'user', 'title': 'carol', 'lastAt': 1},
      ]),
    );
    final again = await ConversationStore.open(
      support: tmp,
      prefs: prefs,
      isolate: false,
    );
    addTearDown(again.close);
    expect(again.loadThreads('alice').single.id, 'bob');
  });

  test('formatListTime uses clock for today', () {
    final now = DateTime.now();
    final ts = DateTime(
      now.year,
      now.month,
      now.day,
      9,
      5,
    ).millisecondsSinceEpoch;
    expect(formatListTime(ts), '09:05');
  });

  test('avatar helpers are stable', () {
    expect(initialOf('alice'), 'A');
    expect(avatarColor('alice'), avatarColor('alice'));
    expect(
      truncate('abcdefghijklmnopqrstuvwxyz0123456789zzzz', max: 8),
      'abcdefgh…',
    );
  });

  test('upsertThread updates on conflict', () async {
    final db = store();
    await db.upsertThread(
      'alice',
      const KimThread(
        id: 'bob',
        kind: ThreadKind.user,
        title: 'bob',
        lastBody: 'hi',
        lastAt: 1,
        unread: 1,
      ),
    );
    await db.upsertThread(
      'alice',
      const KimThread(
        id: 'bob',
        kind: ThreadKind.user,
        title: 'Bobby',
        lastBody: 'there',
        lastAt: 2,
        unread: 0,
      ),
    );
    final row = db.loadThreads('alice').single;
    expect(row.title, 'Bobby');
    expect(row.lastBody, 'there');
    expect(row.unread, 0);
  });

  test(
    'upsertMessages patches on key conflict and pages newest first',
    () async {
      final db = store();
      await db.upsertMessages('alice', 'bob', [
        const KimChatMsg(
          key: 'a',
          dest: 'bob',
          sender: 'alice',
          body: 'one',
          at: 1,
          status: KimSendStatus.sending,
        ),
        const KimChatMsg(
          key: 'b',
          dest: 'bob',
          sender: 'bob',
          body: 'two',
          at: 2,
          messageId: 9,
        ),
      ]);
      await db.upsertMessages('alice', 'bob', [
        const KimChatMsg(
          key: 'a',
          dest: 'bob',
          sender: 'alice',
          body: 'one',
          at: 1,
          messageId: 8,
          status: KimSendStatus.sent,
        ),
      ]);
      final all = db.loadMessages('alice', 'bob');
      expect(all, hasLength(2));
      expect(all.first.messageId, 8);
      expect(all.first.status, KimSendStatus.sent);
      final page = db.loadMessagesPage('alice', 'bob', limit: 1);
      expect(page, hasLength(1));
      expect(page.single.key, 'b');
      final older = db.loadMessagesPage('alice', 'bob', beforeAt: 2, limit: 10);
      expect(older.single.key, 'a');
    },
  );

  test('markThreadRead clears unread', () async {
    final db = store();
    await db.upsertThread(
      'alice',
      const KimThread(
        id: 'bob',
        kind: ThreadKind.user,
        title: 'bob',
        unread: 3,
      ),
    );
    await db.markThreadRead('alice', 'bob');
    expect(db.loadThreads('alice').single.unread, 0);
  });

  test('loadPending is sending-only; loadFailed is failed', () async {
    final db = store();
    await db.upsertMessages('alice', 'bob', [
      const KimChatMsg(
        key: 's',
        dest: 'bob',
        sender: 'alice',
        body: 'sending',
        at: 1,
        status: KimSendStatus.sending,
      ),
      const KimChatMsg(
        key: 'f',
        dest: 'bob',
        sender: 'alice',
        body: 'failed',
        at: 2,
        failed: true,
        status: KimSendStatus.failed,
      ),
      const KimChatMsg(
        key: 'ok',
        dest: 'bob',
        sender: 'alice',
        body: 'sent',
        at: 3,
        status: KimSendStatus.sent,
      ),
    ]);
    expect(db.loadPending('alice').single.key, 's');
    expect(db.loadFailed('alice').single.key, 'f');
  });

  test('own send and history with same messageId collapse to one row', () async {
    final dir = Directory.systemTemp.createTempSync('kim-id-');
    addTearDown(() {
      if (dir.existsSync()) {
        dir.deleteSync(recursive: true);
      }
    });
    var db = await ConversationStore.open(support: dir, isolate: false);
    const uuid = 'aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee';
    await db.applyMessages('alice', const [
      KimChatMsg(
        key: uuid,
        dest: 'bob',
        sender: 'alice',
        body: 'hi',
        at: 1,
        messageId: 123,
        status: KimSendStatus.sent,
      ),
    ], policy: UnreadPolicy.keep);
    await db.applyMessages('alice', const [
      KimChatMsg(
        key: 'm123',
        dest: 'bob',
        sender: 'alice',
        body: 'hi',
        at: 1,
        messageId: 123,
      ),
    ], policy: UnreadPolicy.keep);
    expect(db.loadMessages('alice', 'bob'), hasLength(1));
    expect(db.loadMessages('alice', 'bob').single.key, uuid);
    db.close();
    db = await ConversationStore.open(support: dir, isolate: false);
    addTearDown(db.close);
    expect(db.loadMessages('alice', 'bob'), hasLength(1));
    expect(db.loadMessages('alice', 'bob').single.key, uuid);
    expect(db.loadMessages('alice', 'bob').single.messageId, 123);
  });

  test('same timestamp page uses key as a second cursor', () async {
    final db = store();
    await db.applyMessages('alice', [
      for (var i = 0; i < 60; i++)
        KimChatMsg(
          key: 'k${i.toString().padLeft(3, '0')}',
          dest: 'bob',
          sender: 'bob',
          body: '$i',
          at: 5,
          messageId: i + 1,
        ),
    ], policy: UnreadPolicy.keep);
    final first = db.loadMessagesPage('alice', 'bob', limit: 50);
    expect(first, hasLength(50));
    final older = db.loadMessagesPage(
      'alice',
      'bob',
      beforeAt: first.last.at,
      beforeKey: first.last.key,
      limit: 50,
    );
    expect(older, hasLength(10));
    expect({...first.map((m) => m.key), ...older.map((m) => m.key)}, hasLength(60));
  });

  test('replay of same messageId does not bump unread', () async {
    final db = store();
    const msg = KimChatMsg(
      key: 'm9',
      dest: 'bob',
      sender: 'bob',
      body: 'hi',
      at: 1,
      messageId: 9,
    );
    final first = await db.applyMessages('alice', [msg]);
    expect(first.single.inserted, isTrue);
    expect(first.single.unreadDelta, 1);
    expect(first.single.thread.unread, 1);
    final second = await db.applyMessages('alice', [msg]);
    expect(second.single.inserted, isFalse);
    expect(second.single.unreadDelta, 0);
    expect(second.single.thread.unread, 1);
  });

  test('isolate-backed store round-trips apply', () async {
    final dir = Directory.systemTemp.createTempSync('kim-iso-');
    addTearDown(() {
      if (dir.existsSync()) {
        dir.deleteSync(recursive: true);
      }
    });
    final db = await ConversationStore.open(support: dir);
    addTearDown(db.close);
    final results = await db.applyMessages('alice', const [
      KimChatMsg(
        key: 'm1',
        dest: 'bob',
        sender: 'bob',
        body: 'hi',
        at: 1,
        messageId: 1,
      ),
    ]);
    expect(results, hasLength(1));
    expect(results.single.inserted, isTrue);
    await db.warmThreads('alice');
    await db.ensureMessages('alice', 'bob');
    expect(db.loadThreads('alice').single.id, 'bob');
    expect(db.loadMessagesPage('alice', 'bob').single.body, 'hi');
  });

  test('isolate upsertThread from inbox persist does not throw', () async {
    final dir = Directory.systemTemp.createTempSync('kim-iso-thread-');
    addTearDown(() {
      if (dir.existsSync()) {
        dir.deleteSync(recursive: true);
      }
    });
    final db = await ConversationStore.open(support: dir);
    addTearDown(db.close);
    await db.upsertThread(
      'alice',
      const KimThread(
        id: 'bob',
        kind: ThreadKind.user,
        title: 'bob',
        lastBody: 'hi',
        lastAt: 9,
        unread: 1,
      ),
    );
    await db.warmThreads('alice');
    expect(db.loadThreads('alice').single.unread, 1);
    await db.upsertThread(
      'alice',
      const KimThread(
        id: 'bob',
        kind: ThreadKind.user,
        title: 'Bobby',
        lastBody: 'there',
        lastAt: 10,
        unread: 2,
      ),
    );
    expect(db.loadThreads('alice').single.title, 'Bobby');
    expect(db.loadThreads('alice').single.unread, 2);
  });

  test('image lastBody is stored as [图片] across reopen', () async {
    final dir = Directory.systemTemp.createTempSync('kim-preview-');
    addTearDown(() {
      if (dir.existsSync()) {
        dir.deleteSync(recursive: true);
      }
    });
    var db = await ConversationStore.open(support: dir, isolate: false);
    await db.applyMessages('alice', const [
      KimChatMsg(
        key: 'aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee',
        dest: 'bob',
        sender: 'alice',
        body: 'https://media.kim.ainexc.com/alice/a.jpg',
        at: 1,
        kind: KimMsgKind.image,
        messageId: 9,
        status: KimSendStatus.sent,
      ),
    ], policy: UnreadPolicy.keep);
    expect(db.loadThreads('alice').single.lastBody, Copy.imageMessage);
    db.close();
    db = await ConversationStore.open(support: dir, isolate: false);
    addTearDown(db.close);
    expect(db.loadThreads('alice').single.lastBody, Copy.imageMessage);
    await db.upsertThread(
      'alice',
      const KimThread(
        id: 'bob',
        kind: ThreadKind.user,
        title: 'bob',
        lastBody: 'https://media.kim.ainexc.com/alice/a.jpg',
        lastAt: 1,
      ),
    );
    expect(db.loadThreads('alice').single.lastBody, Copy.imageMessage);
  });
}
