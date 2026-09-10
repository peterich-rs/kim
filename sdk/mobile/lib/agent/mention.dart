/// Local Goose persona. Dest `goose` is a synthetic IM contact, not a server user.
library;

import '../models/models.dart';

const kGooseAgentId = 'goose';
const kGooseAgentName = '助手';

const kGooseAgentPerson = KimPerson(
  account: kGooseAgentId,
  nickname: kGooseAgentName,
  kind: ProfileKind.bot,
);

bool isGooseAgentDest(String dest) => dest == kGooseAgentId;

final _goose = RegExp(
  r'(?:^|[\s])@goose(?:$|[\s,，.。!！?？])',
  caseSensitive: false,
);

bool mentionsGooseAgent(String text) {
  if (text.contains('@助手')) {
    return true;
  }
  return _goose.hasMatch(' $text');
}

List<KimPerson> withGooseAgent(List<KimPerson> people) {
  if (people.any((p) => p.account == kGooseAgentId)) {
    return people;
  }
  return [kGooseAgentPerson, ...people];
}
