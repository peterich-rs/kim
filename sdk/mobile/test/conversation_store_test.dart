import 'dart:convert';
import 'dart:io';

import 'package:connectivity_plus/connectivity_plus.dart';
import 'package:flutter_test/flutter_test.dart';
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
    expect(networkLinkUp(const [ConnectivityResult.bluetooth]), isFalse);
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
    final db = await ConversationStore.open(support: tmp, prefs: prefs);
    addTearDown(db.close);
    expect(db.loadThreads('alice').single.id, 'bob');
    expect(db.loadMessages('alice', 'bob').single.body, 'hi');
    await prefs.setString(
      'kim.threads.alice',
      jsonEncode([
        {'id': 'carol', 'kind': 'user', 'title': 'carol', 'lastAt': 1},
      ]),
    );
    final again = await ConversationStore.open(support: tmp, prefs: prefs);
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
}
