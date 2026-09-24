library;

import 'package:kim_mobile/bridge/kim_bridge.dart';
import 'package:kim_mobile/src/rust/api/types.dart';

/// UI-facing catalog. Rows live in the Rust store; this does not assemble
/// `profile_json` for a turn.
class AgentCatalog {
  AgentCatalog(this.client);

  final KimClientPort client;

  Future<List<AgentProfile>> profiles() => client.listAgentProfiles();

  Future<void> upsert(AgentProfile row) => client.upsertAgentProfile(row);

  Future<void> delete(String id) => client.deleteAgentProfile(id);

  Future<List<ProviderAccount>> accounts() => client.listProviderAccounts();
}
