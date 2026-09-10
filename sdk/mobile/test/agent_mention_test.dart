import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/agent/mention.dart';

void main() {
  test('mentionsGooseAgent matches @助手 and @goose', () {
    expect(mentionsGooseAgent('hey @助手 总结一下'), isTrue);
    expect(mentionsGooseAgent('@goose please'), isTrue);
    expect(mentionsGooseAgent('hello goose'), isFalse);
    expect(mentionsGooseAgent('email goose@x.com'), isFalse);
  });

  test('goose dest is a synthetic local contact', () {
    expect(isGooseAgentDest(kGooseAgentId), isTrue);
    expect(isGooseAgentDest('alice'), isFalse);
    expect(withGooseAgent(const []).single.account, kGooseAgentId);
    expect(withGooseAgent(const []).single.isBot, isTrue);
    expect(withGooseAgent(const [kGooseAgentPerson]).length, 1);
  });
}
