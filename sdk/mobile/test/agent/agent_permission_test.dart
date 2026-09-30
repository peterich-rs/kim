import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/features/agent/providers/agent_permission.dart';
import 'package:kim_mobile/features/agent/providers/agent_profiles.dart';
import 'package:kim_mobile/features/session/providers.dart';

import '../support/fake_kim.dart';

class _RecordingClient extends FakeKim {
  String? callId;
  bool? allow;

  @override
  Future<void> respondAgentPermission({
    required String callId,
    required bool allow,
  }) async {
    this.callId = callId;
    this.allow = allow;
  }
}

void main() {
  test('ask-before override is explicit; default is auto-allow', () {
    expect(permissionAsksBefore(const {}, kAskBeforeBash), isFalse);
    final asked = setPermissionAskBefore(const {}, kAskBeforeBash, ask: true);
    expect(permissionAsksBefore(asked, kAskBeforeBash), isTrue);
    expect(asked[kAskBeforeBash], kPermissionAskBefore);
    final cleared = setPermissionAskBefore(asked, kAskBeforeBash, ask: false);
    expect(cleared.containsKey(kAskBeforeBash), isFalse);
  });

  test('hub clears the prompt and answers through the client port', () async {
    final client = _RecordingClient();
    final container = ProviderContainer(
      overrides: [clientPortProvider.overrideWithValue(client)],
    );
    addTearDown(container.dispose);
    final hub = container.read(agentPermissionHubProvider.notifier)
      ..bindClient(client);
    hub.prompt(
      'b_bot',
      const AgentPermissionPrompt(
        callId: 'c1',
        name: 'bash',
        preview: 'Allow bash?',
      ),
    );
    expect(
      container.read(agentPermissionHubProvider).of('b_bot'),
      hasLength(1),
    );
    await hub.respond(dest: 'b_bot', callId: 'c1', permission: 'allow_once');
    expect(client.callId, 'c1');
    expect(client.allow, isTrue);
    expect(container.read(agentPermissionHubProvider).of('b_bot'), isEmpty);
  });

  test('profile JSON keeps ask-before overrides', () {
    final profile = AgentProfile(
      id: 'p-1',
      displayName: 'Work',
      providerKind: 'openai',
      baseUrl: '',
      model: 'gpt-4o',
      keyRef: '',
      systemPrompt: '',
      permissionOverrides: const {kAskBeforeBash: kPermissionAskBefore},
    );
    final again = AgentProfile.fromJson(profile.toJson());
    expect(again.permissionOverrides[kAskBeforeBash], kPermissionAskBefore);
  });
}
