import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/features/agent/agent_presence.dart';
import 'package:kim_mobile/features/agent/pet_pack.dart';
import 'package:kim_mobile/features/session/typing.dart';

void main() {
  test('typing true → running', () {
    final container = ProviderContainer();
    addTearDown(container.dispose);
    container
        .read(typingProvider.notifier)
        .applyPush(typer: 'goose', dest: 'goose', active: true);
    expect(
      container.read(agentPresenceProvider('goose')).phase,
      PetPhase.running,
    );
  });

  test('begin then finish(failed: false) → review then idle after 1200 ms', () {
    final container = ProviderContainer();
    addTearDown(container.dispose);
    final n = container.read(agentRunStatusProvider.notifier);
    n.begin('goose');
    expect(
      container.read(agentPresenceProvider('goose')).phase,
      PetPhase.running,
    );
    n.finish('goose', failed: false);
    expect(
      container.read(agentPresenceProvider('goose')).phase,
      PetPhase.review,
    );
    final status = container.read(agentRunStatusProvider);
    expect(
      presenceFor(
        dest: 'goose',
        status: status,
        typing: false,
        now: DateTime.now().add(kPetPresenceWindow),
      ).phase,
      PetPhase.idle,
    );
  });

  test('finish(failed: true) wins over typing', () {
    final container = ProviderContainer();
    addTearDown(container.dispose);
    container
        .read(typingProvider.notifier)
        .applyPush(typer: 'goose', dest: 'goose', active: true);
    container
        .read(agentRunStatusProvider.notifier)
        .finish('goose', failed: true);
    expect(
      container.read(agentPresenceProvider('goose')).phase,
      PetPhase.failed,
    );
  });

  test('markOpened → wave; second call same dest does not re-trigger', () {
    final container = ProviderContainer();
    addTearDown(container.dispose);
    final n = container.read(agentRunStatusProvider.notifier);
    n.markOpened('goose');
    expect(container.read(agentPresenceProvider('goose')).phase, PetPhase.wave);
    final opened = container.read(agentRunStatusProvider).openedAt['goose'];
    n.markOpened('goose');
    expect(container.read(agentRunStatusProvider).openedAt['goose'], opened);
    expect(
      presenceFor(
        dest: 'goose',
        status: container.read(agentRunStatusProvider),
        typing: false,
        now: DateTime.now().add(kPetPresenceWindow),
      ).phase,
      PetPhase.idle,
    );
  });

  test('human dest with no typing → idle', () {
    final container = ProviderContainer();
    addTearDown(container.dispose);
    expect(container.read(agentPresenceProvider('alice')).phase, PetPhase.idle);
  });

  test('consumeOneShot drops wave so a remount is idle', () {
    final container = ProviderContainer();
    addTearDown(container.dispose);
    final n = container.read(agentRunStatusProvider.notifier);
    n.markOpened('goose');
    expect(container.read(agentPresenceProvider('goose')).phase, PetPhase.wave);
    n.consumeOneShot('goose');
    expect(container.read(agentPresenceProvider('goose')).phase, PetPhase.idle);
  });
}
