import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/state/receipts.dart';
import 'package:kim_mobile/state/typing.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

void main() {
  test('typing is conversation-scoped by typer', () {
    final container = ProviderContainer();
    addTearDown(container.dispose);
    final n = container.read(typingProvider.notifier);
    n.applyPush(typer: 'alice', dest: 'bob', active: true);
    expect(container.read(peerTypingProvider('alice')), isTrue);
    expect(container.read(peerTypingProvider('carol')), isFalse);
    n.applyPush(typer: 'alice', dest: 'bob', active: false);
    expect(container.read(peerTypingProvider('alice')), isFalse);
  });

  test('receipts track peer watermark for DM only', () {
    final container = ProviderContainer();
    addTearDown(container.dispose);
    final n = container.read(receiptsProvider.notifier);
    n.applyPush(reader: 'bob', dest: 'alice', kind: 0, messageId: 10);
    expect(container.read(peerReadUpToProvider('bob')), 10);
    n.applyPush(reader: 'bob', dest: 'alice', kind: 0, messageId: 8);
    expect(container.read(peerReadUpToProvider('bob')), 10);
    n.applyPush(reader: 'bob', dest: 'g1', kind: 1, messageId: 99);
    expect(container.read(peerReadUpToProvider('bob')), 10);
  });
}
