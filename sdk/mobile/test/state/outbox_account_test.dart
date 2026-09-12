import 'dart:async';
import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:kim_media_picker/kim_media_picker.dart';
import 'package:kim_mobile/models/models.dart';
import 'package:kim_mobile/state/auth.dart';
import 'package:kim_mobile/state/contacts.dart';
import 'package:kim_mobile/state/link.dart';
import 'package:kim_mobile/state/outbox.dart';
import 'package:kim_mobile/state/session.dart';

import '../support/harness.dart';

Future<void> _tick() => Future<void>.delayed(Duration.zero);

Future<void> _waitUntil(bool Function() ok, {String? reason}) async {
  for (var i = 0; i < 80; i++) {
    if (ok()) {
      return;
    }
    await Future<void>.delayed(const Duration(milliseconds: 10));
  }
  fail(reason ?? 'condition not met');
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test('account switch during upload does not persist to the new account', () async {
    final env = await kimHarness(token: 'tok.jwt', account: 'alice');
    env.media.uploadHold = Completer<void>();
    env.fake.friends = const [KimPerson(account: 'bob', nickname: 'Bobby')];
    env.container.read(linkProvider);
    await _tick();
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
    final sw = Stopwatch()..start();
    final queued = env.container.read(outboxProvider.notifier).sendImages(
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
    await _waitUntil(() => env.media.uploads == 1);
    debugPrint('enqueue persist ${sw.elapsedMilliseconds}ms');
    expect(env.store.loadPending('alice'), isNotEmpty);
    expect(env.store.loadPending('carol'), isEmpty);
    await env.runtime.settings.saveSession(token: 'tok.jwt', account: 'carol');
    env.container.invalidate(authProvider);
    await _tick();
    expect(env.container.read(sessionProvider).account, 'carol');
    env.media.uploadHold!.complete();
    await queued;
    await _waitUntil(() => env.fake.imageTalks == 1);
    await _tick();
    expect(env.store.loadMessages('carol', 'bob'), isEmpty);
    final aliceRows = env.store.loadMessages('alice', 'bob');
    expect(aliceRows, isNotEmpty);
    expect(aliceRows.single.sender, 'alice');
    expect(aliceRows.single.status, KimSendStatus.sent);
  });
}
