import 'package:connectivity_plus/connectivity_plus.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/core/connectivity.dart';
import 'package:kim_mobile/core/format.dart';
import 'package:kim_mobile/data/conversation_store.dart';
import 'package:kim_mobile/models/models.dart';
import 'package:shared_preferences/shared_preferences.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  setUp(() {
    SharedPreferences.setMockInitialValues({});
  });

  test('networkLinkUp treats none as offline and wifi as online', () {
    expect(networkLinkUp(const [ConnectivityResult.none]), isFalse);
    expect(networkLinkUp(const []), isFalse);
    expect(networkLinkUp(const [ConnectivityResult.wifi]), isTrue);
    expect(networkLinkUp(const [ConnectivityResult.mobile]), isTrue);
    expect(networkLinkUp(const [ConnectivityResult.bluetooth]), isFalse);
  });

  test('threads persist per account', () async {
    final store = ConversationStore(await SharedPreferences.getInstance());
    await store.saveThreads('alice', [
      const KimThread(
        id: 'bob',
        kind: ThreadKind.user,
        title: 'bob',
        lastBody: 'hi',
        lastAt: 9,
      ),
    ]);
    expect(store.loadThreads('alice').single.id, 'bob');
    expect(store.loadThreads('carol'), isEmpty);
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
    final store = ConversationStore(await SharedPreferences.getInstance());
    await store.saveMessages('alice', 'bob', [
      const KimChatMsg(
        key: '1',
        dest: 'bob',
        sender: 'alice',
        body: 'hello',
        at: 1,
      ),
    ]);
    expect(store.loadMessages('alice', 'bob').single.body, 'hello');
  });

  test('deleteThread drops the row and messages', () async {
    final store = ConversationStore(await SharedPreferences.getInstance());
    await store.saveThreads('alice', [
      const KimThread(id: 'bob', kind: ThreadKind.user, title: 'bob'),
    ]);
    await store.saveMessages('alice', 'bob', [
      const KimChatMsg(
        key: '1',
        dest: 'bob',
        sender: 'alice',
        body: 'hello',
        at: 1,
      ),
    ]);
    await store.deleteThread('alice', 'bob');
    expect(store.loadThreads('alice'), isEmpty);
    expect(store.loadMessages('alice', 'bob'), isEmpty);
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
