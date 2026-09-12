import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/data/conversation_store.dart';
import 'package:kim_mobile/data/message_repository.dart';
import 'package:kim_mobile/models/models.dart';
import 'package:kim_mobile/state/inbox.dart';
import 'package:kim_mobile/state/link.dart';
import 'package:kim_mobile/state/location.dart';
import 'package:kim_mobile/state/providers.dart';

import '../support/harness.dart';

Future<void> _tick() => Future<void>.delayed(Duration.zero);

Future<void> _waitUntil(bool Function() ok, {String? reason}) async {
  for (var i = 0; i < 50; i++) {
    if (ok()) {
      return;
    }
    await Future<void>.delayed(const Duration(milliseconds: 10));
  }
  fail(reason ?? 'condition not met');
}

class _GatedRepo extends MessageRepository {
  _GatedRepo(super.store, {this.liveHold, this.throwOnSync = false});

  Completer<void>? liveHold;
  bool throwOnSync;
  final order = <String>[];

  @override
  Future<List<ApplyResult>> applyLive(
    String account,
    Iterable<KimChatMsg> msgs, {
    String? viewingDest,
  }) async {
    order.add('live-start');
    final hold = liveHold;
    if (hold != null) {
      await hold.future;
    }
    final sw = Stopwatch()..start();
    final results = await super.applyLive(
      account,
      msgs,
      viewingDest: viewingDest,
    );
    debugPrint('applyMessages live ${sw.elapsedMilliseconds}ms');
    order.add('live-done');
    return results;
  }

  @override
  Future<List<ApplyResult>> applySync(
    String account,
    Iterable<KimChatMsg> msgs, {
    String? viewingDest,
  }) async {
    order.add('sync-start');
    if (throwOnSync) {
      throw StateError('apply failed');
    }
    final sw = Stopwatch()..start();
    final results = await super.applySync(
      account,
      msgs,
      viewingDest: viewingDest,
    );
    debugPrint('applyMessages sync ${sw.elapsedMilliseconds}ms');
    order.add('sync-done');
    return results;
  }
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test('rustStore catch-up keeps unread while ChatPage is open', () async {
    final env = await kimHarness(
      token: 'tok.jwt',
      account: 'alice',
      rustStore: true,
    );
    env.container.read(locationProvider.notifier).setPath('/chat/bob');
    env.container.read(threadsProvider.notifier).mergeInbox([
      KimThread(
        id: 'bob',
        kind: ThreadKind.user,
        title: 'bob',
        lastBody: 'later',
        lastAt: 2,
        unread: 3,
      ),
    ]);
    expect(env.container.read(threadsProvider).thread('bob')?.unread, 3);
  });


  test('talk and syncPage finish in arrival order', () async {
    final hold = Completer<void>();
    _GatedRepo? repo;
    final env = await kimHarness(
      token: 'tok.jwt',
      account: 'alice',
      overrides: [
        messageRepositoryProvider.overrideWith((ref) {
          final created = _GatedRepo(
            ref.watch(conversationStoreProvider),
            liveHold: hold,
          );
          repo = created;
          return created;
        }),
      ],
    );
    env.container.read(linkProvider);
    await _tick();
    env.fake.emitTalk(
      dest: 'bob',
      sender: 'bob',
      body: 'live',
      messageId: 11,
      sendTime: 1_700_000_000_000,
    );
    env.fake.emitSyncPage(
      pageId: 12,
      talks: const [
        KimEvent(
          kind: KimEventKind.talk,
          dest: 'bob',
          sender: 'bob',
          body: 'sync',
          messageId: 12,
          sendTime: 1_700_000_001_000,
        ),
      ],
    );
    await _waitUntil(() => repo?.order.contains('live-start') == true);
    expect(repo!.order, ['live-start']);
    expect(env.fake.acks, 0);
    expect(env.fake.confirms, 0);
    hold.complete();
    await _waitUntil(() => repo?.order.length == 4);
    expect(repo!.order, ['live-start', 'live-done', 'sync-start', 'sync-done']);
    expect(env.fake.acks, 1);
    expect(env.fake.confirms, 1);
  });

  test('applySync throw skips syncConfirm', () async {
    _GatedRepo? repo;
    final env = await kimHarness(
      token: 'tok.jwt',
      account: 'alice',
      overrides: [
        messageRepositoryProvider.overrideWith((ref) {
          final created = _GatedRepo(
            ref.watch(conversationStoreProvider),
            throwOnSync: true,
          );
          repo = created;
          return created;
        }),
      ],
    );
    env.container.read(linkProvider);
    await _tick();
    env.fake.emitSyncPage(
      pageId: 99,
      talks: const [
        KimEvent(
          kind: KimEventKind.talk,
          dest: 'bob',
          sender: 'bob',
          body: 'sync',
          messageId: 99,
          sendTime: 1_700_000_000_000,
        ),
      ],
    );
    await _waitUntil(() => repo?.order.contains('sync-start') == true);
    await _tick();
    expect(env.fake.confirms, 0);
    expect(repo!.order, ['sync-start']);
  });
}
