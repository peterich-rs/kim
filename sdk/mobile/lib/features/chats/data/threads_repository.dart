/// Maps session DTOs + local agent personas into inbox threads.
library;

import 'package:kim_mobile/bridge/kim_ports.dart';
import 'package:kim_mobile/features/agent/agent_profiles.dart';
import 'package:kim_mobile/features/agent/host_support.dart';
import 'package:kim_mobile/features/agent/mention.dart';
import 'package:kim_mobile/features/session/kim_session.dart';
import 'package:kim_mobile/models/models.dart';
import 'package:kim_mobile/src/rust/api/types.dart' show ThreadView;

class ThreadsRepository {
  const ThreadsRepository(this._client);

  final KimClientPort _client;

  /// Remote inbox rows → UI threads, optionally merged with local agents.
  List<KimThread> project({
    required List<ThreadView> remote,
    List<AgentProfile> localAgents = const [],
  }) {
    var threads = [for (final t in remote) kimThreadFrom(t)];
    if (agentHostSupported && localAgents.isNotEmpty) {
      threads = mergeLocalAgents(threads, localAgents);
    }
    return threads;
  }

  /// Local enabled agent personas appear as threads when missing from inbox.
  static List<KimThread> mergeLocalAgents(
    List<KimThread> threads,
    List<AgentProfile> enabled,
  ) {
    final existing = {for (final t in threads) t.id};
    final extras = <KimThread>[];
    for (final profile in enabled) {
      final id = personForProfile(profile).account;
      if (existing.contains(id)) {
        continue;
      }
      extras.add(
        KimThread(id: id, kind: ThreadKind.user, title: profile.displayName),
      );
      existing.add(id);
    }
    if (extras.isEmpty) {
      return threads;
    }
    return [...extras, ...threads];
  }

  Future<void> markConversationRead(String dest, ThreadKind kind) {
    return _client.markConversationRead(dest, kind);
  }

  Future<void> deleteThread(String id) {
    return _client.deleteThread(id);
  }
}

/// Backward-compatible name used by older call sites / tests.
List<KimThread> withLocalThreads(
  List<KimThread> threads,
  List<AgentProfile> enabled,
) => ThreadsRepository.mergeLocalAgents(threads, enabled);
