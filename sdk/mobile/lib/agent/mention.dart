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
  return KimPerson(
    account: canonicalAgentDest(profile.dest),
    nickname: profile.displayName,
    kind: ProfileKind.bot,
  );
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
