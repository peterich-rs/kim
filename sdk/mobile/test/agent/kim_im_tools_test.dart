import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/features/agent/kim_im_tools.dart';
import 'package:kim_mobile/models/models.dart';

import '../support/fake_kim.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test('search_contacts returns friend hits', () async {
    final fake = FakeKim()
      ..friends = const [
        KimPerson(account: 'bob', nickname: 'Bob'),
        KimPerson(account: 'cara', nickname: 'Cara'),
      ];
    final json = jsonDecode(
      await KimImTools(fake).execute(
        name: 'search_contacts',
        argumentsJson: '{"query":"bo"}',
        currentDest: 'b_bot',
      ),
    );
    expect(json['ok'], true);
    expect(json['people'], [
      {'account': 'bob', 'nickname': 'Bob', 'kind': 1},
    ]);
  });

  test('send_message refuses agent dests', () async {
    final fake = FakeKim();
    final json = jsonDecode(
      await KimImTools(fake).execute(
        name: 'send_message',
        argumentsJson: '{"dest":"b_bot","text":"hi"}',
        currentDest: 'b_bot',
      ),
    );
    expect(json['ok'], false);
    expect(fake.talks, 0);
  });

  test('send_message enqueues text for a person', () async {
    final fake = FakeKim();
    final json = jsonDecode(
      await KimImTools(fake).execute(
        name: 'send_message',
        argumentsJson: '{"dest":"bob","text":"hi"}',
        currentDest: 'alice',
      ),
    );
    expect(json['ok'], true);
    expect(fake.enqueues, 1);
    expect(fake.lastEnqueueBody, 'hi');
    expect(fake.lastEnqueueDest, 'bob');
  });
}
