import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/core/settings.dart';
import 'package:kim_mobile/data/conversation_store.dart';
import 'package:shared_preferences/shared_preferences.dart';

import 'support/fake_kim.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test('rustStore on skips isolate and calls attachStore', () async {
    SharedPreferences.setMockInitialValues({KimFlags.rustStorePref: true});
    final prefs = await SharedPreferences.getInstance();
    expect(KimFlags.rustStore(prefs), isTrue);
    final dir = Directory.systemTemp.createTempSync('kim-boot-');
    addTearDown(() {
      if (dir.existsSync()) {
        dir.deleteSync(recursive: true);
      }
    });
    final fake = FakeKim();
    final store = await ConversationStore.openForRuntime(
      support: dir,
      rustStore: true,
      attachStore: fake.attachStore,
    );
    addTearDown(store.close);
    expect(store.isolateBacked, isFalse);
    expect(fake.attachStores, 1);
    expect(fake.lastAttachPath, endsWith('kim-cache.db'));
  });

  test('rustStore off opens dart store and does not attach', () async {
    SharedPreferences.setMockInitialValues({KimFlags.rustStorePref: false});
    final prefs = await SharedPreferences.getInstance();
    expect(KimFlags.rustStore(prefs), isFalse);
    final dir = Directory.systemTemp.createTempSync('kim-boot-');
    addTearDown(() {
      if (dir.existsSync()) {
        dir.deleteSync(recursive: true);
      }
    });
    final fake = FakeKim();
    final store = await ConversationStore.openForRuntime(
      support: dir,
      rustStore: false,
      attachStore: fake.attachStore,
      prefs: prefs,
    );
    addTearDown(store.close);
    expect(fake.attachStores, 0);
    expect(store.isolateBacked, isTrue);
  });
}
