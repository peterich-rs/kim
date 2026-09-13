import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/agent/mention.dart';
import 'package:kim_mobile/state/agent_profiles.dart';

AgentProfile _profile({
  required String id,
  required String displayName,
  List<String> aliases = const [],
}) {
  return AgentProfile(
    id: id,
    displayName: displayName,
    aliases: aliases,
    providerKind: 'openai',
    baseUrl: '',
    model: 'gpt-4o',
    keyRef: 'agent.api_key.$id',
    systemPrompt: '',
  );
}

void main() {
  test('mentionsGooseAgent matches @助手 and @goose', () {
    expect(mentionsGooseAgent('hey @助手 总结一下'), isTrue);
    expect(mentionsGooseAgent('@goose please'), isTrue);
    expect(mentionsGooseAgent('hello goose'), isFalse);
    expect(mentionsGooseAgent('email goose@x.com'), isFalse);
  });

  test('goose dest is a local dest, not a synthesized contact', () {
    expect(isGooseAgentDest(kGooseAgentId), isTrue);
    expect(isGooseAgentDest('alice'), isFalse);
    expect(isOwnedAgentAccount('goose', const []), isFalse);
    expect(profileForChatDest('goose', const []), isNull);
  });

  test('profileForChatDest matches dest, goose, and serverAccount', () {
    final goose = _profile(id: 'goose', displayName: '助手');
    final work = _profile(
      id: 'p-1',
      displayName: 'Work',
    ).copyWith(serverAccount: 'b_ABC');
    final all = [goose, work];
    expect(profileForChatDest('goose', all)?.id, 'goose');
    expect(profileForChatDest('agent:goose', all)?.id, 'goose');
    expect(profileForChatDest('agent:p-1', all)?.id, 'p-1');
    expect(profileForChatDest('b_ABC', all)?.id, 'p-1');
    expect(profileForChatDest('goose', [work]), isNull);
    expect(isOwnedAgentAccount('goose', all), isTrue);
    expect(isOwnedAgentAccount('b_ABC', all), isTrue);
    expect(isOwnedAgentAccount('goose', const []), isFalse);
  });

  test('agent:goose canonicalizes to goose', () {
    expect(isAgentDest('agent:goose'), isTrue);
    expect(isGooseAgentDest('agent:goose'), isTrue);
    expect(canonicalAgentDest('agent:goose'), kGooseAgentId);
    expect(canonicalAgentDest('agent:translator'), 'agent:translator');
  });

  test('serverAccount is IM dest and not a local agent dest', () {
    final profile = _profile(
      id: 'goose',
      displayName: '助手',
    ).copyWith(serverAccount: 'b_ABC');
    expect(personForProfile(profile).account, 'b_ABC');
    expect(isAgentDest('b_ABC'), isFalse);
    expect(isServerBotAccount('b_ABC'), isTrue);
    expect(isOwnedRegisteredBot('b_ABC', [profile]), isTrue);
    expect(isOwnedRegisteredBot('goose', [profile]), isFalse);
  });

  test('mention prefers exact id over display name', () {
    final enabled = [
      _profile(id: 'goose', displayName: '助手', aliases: const ['助手']),
      _profile(
        id: 'translator',
        displayName: '译者',
        aliases: const ['translator'],
      ),
    ];
    expect(mentionedProfile('@translator hello', enabled)?.id, 'translator');
    expect(mentionedProfile('hey @译者', enabled)?.id, 'translator');
    expect(mentionedProfile('@goose', enabled)?.id, 'goose');
    expect(mentionsAgent('no mention', enabled), isFalse);
  });
}
