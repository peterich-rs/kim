/// Local Goose personas. Dest `goose` is a synthetic IM contact, not a server user.
library;

import '../models/models.dart';
import '../state/agent_profiles.dart';

const kGooseAgentId = 'goose';
const kGooseAgentName = '助手';

const kGooseAgentPerson = KimPerson(
  account: kGooseAgentId,
  nickname: kGooseAgentName,
  kind: ProfileKind.bot,
);

bool isAgentDest(String dest) =>
    dest == kGooseAgentId || dest.startsWith('agent:');

bool isGooseAgentDest(String dest) =>
    dest == kGooseAgentId || dest == 'agent:goose';

String canonicalAgentDest(String dest) {
  if (dest == 'agent:goose') {
    return kGooseAgentId;
  }
  return dest;
}

KimPerson personForProfile(AgentProfile profile) {
  final account = profile.serverAccount.isNotEmpty
      ? profile.serverAccount
      : canonicalAgentDest(profile.dest);
  return KimPerson(
    account: account,
    nickname: profile.displayName,
    kind: ProfileKind.bot,
  );
}

/// Royal `create_bot` accounts are `b_` + base36 id. Not a local Goose dest.
bool isServerBotAccount(String account) => account.startsWith('b_');

/// Server bot accounts are IM dests, not local `goose` / `agent:` dests.
bool isOwnedRegisteredBot(String dest, List<AgentProfile> profiles) {
  if (dest.isEmpty || isAgentDest(dest)) {
    return false;
  }
  for (final profile in profiles) {
    if (profile.serverAccount.isNotEmpty && profile.serverAccount == dest) {
      return true;
    }
  }
  return false;
}

bool isOwnedAgentAccount(String account, List<AgentProfile> profiles) {
  if (isAgentDest(account)) {
    return true;
  }
  return isOwnedRegisteredBot(account, profiles);
}

List<KimPerson> withLocalAgents(
  List<KimPerson> people,
  List<AgentProfile> enabled,
) {
  final existing = {for (final p in people) p.account};
  final extras = <KimPerson>[];
  for (final profile in enabled) {
    final person = personForProfile(profile);
    if (existing.contains(person.account)) {
      continue;
    }
    extras.add(person);
    existing.add(person.account);
  }
  if (extras.isEmpty) {
    return people;
  }
  return [...extras, ...people];
}

List<KimPerson> withGooseAgent(List<KimPerson> people) {
  return withLocalAgents(people, const [
    AgentProfile(
      id: kGooseAgentId,
      displayName: kGooseAgentName,
      providerKind: 'openai',
      baseUrl: '',
      model: 'gpt-4o',
      keyRef: 'agent.api_key.goose',
      systemPrompt: '',
    ),
  ]);
}

final _mentionToken = RegExp(r'@([^\s,，.。!！?？]+)');

AgentProfile? mentionedProfile(String text, List<AgentProfile> enabled) {
  final tokens = [
    for (final m in _mentionToken.allMatches(text)) m.group(1) ?? '',
  ].where((t) => t.isNotEmpty).toList();
  if (tokens.isEmpty) {
    return null;
  }
  AgentProfile? byId;
  AgentProfile? byName;
  for (final token in tokens) {
    for (final profile in enabled) {
      if (token == profile.id ||
          token.toLowerCase() == profile.id.toLowerCase()) {
        byId ??= profile;
      }
      if (token == profile.displayName ||
          profile.aliases.contains(token) ||
          profile.aliases.any((a) => a.toLowerCase() == token.toLowerCase())) {
        byName ??= profile;
      }
    }
  }
  return byId ?? byName;
}

bool mentionsAgent(String text, List<AgentProfile> enabled) =>
    mentionedProfile(text, enabled) != null;

bool mentionsGooseAgent(String text) {
  return mentionsAgent(text, const [
    AgentProfile(
      id: kGooseAgentId,
      displayName: kGooseAgentName,
      aliases: ['助手'],
      providerKind: 'openai',
      baseUrl: '',
      model: 'gpt-4o',
      keyRef: 'agent.api_key.goose',
      systemPrompt: '',
    ),
  ]);
}
