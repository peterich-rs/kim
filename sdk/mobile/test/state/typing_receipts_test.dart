import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/features/session/receipts.dart';
import 'package:kim_mobile/features/session/typing.dart';
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

  test('own typing on another device is not shown as peer typing', () {
    final container = ProviderContainer();
    addTearDown(container.dispose);
    final n = container.read(typingProvider.notifier);
    n.applyPush(typer: 'alice', dest: 'b_bot', active: true, me: 'alice');
    expect(container.read(peerTypingProvider('b_bot')), isFalse);
    expect(container.read(peerTypingProvider('alice')), isFalse);
  });

  test('clear drops leftover agent typing', () {
    final container = ProviderContainer();
    addTearDown(container.dispose);
    final n = container.read(typingProvider.notifier);
    n.applyAgentTurn(dest: 'b_bot', busy: true);
    expect(container.read(peerTypingProvider('b_bot')), isTrue);
    n.clear();
    expect(container.read(peerTypingProvider('b_bot')), isFalse);
  });

  test('agent busy keys the bot dest not the owner', () {
    final container = ProviderContainer();
    addTearDown(container.dispose);
    final n = container.read(typingProvider.notifier);
    n.applyPush(typer: 'b_bot', dest: 'alice', active: true, me: 'alice');
    expect(container.read(peerTypingProvider('b_bot')), isTrue);
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
