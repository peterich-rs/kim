import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/bridge/goose_bridge.dart';
import 'package:kim_mobile/features/agent/agent_permission.dart';
import 'package:kim_mobile/features/agent/agent_profiles.dart';

class _FakeSession implements AgentSessionPort {
  String? permission;
  String? callId;

  @override
  Stream<AgentUiEvent> listen() => const Stream.empty();

  @override
  Future<String> prompt({required String text}) async => '';

  @override
  Future<String> promptWithContext({
    required String text,
    required String contextJson,
  }) async => '';

  @override
  Future<String> completeTool({
    required String callId,
    required String outputJson,
  }) async => '';

  @override
  Future<String> respondPermission({
    required String callId,
    required String permission,
  }) async {
    this.callId = callId;
    this.permission = permission;
    return '';
  }

  @override
  Future<void> close() async {}

  @override
  Future<void> abort() async {}

  @override
  Future<void> steer({required String text}) async {}

  @override
  Future<void> reconfigure({required SessionOpenOpts opts}) async {}

  @override
  Future<ResumeReportDto> resume() async =>
      const ResumeReportDto(resumedOps: [], statuses: []);

  @override
  SessionSnapshotDto snapshot() => const SessionSnapshotDto(
    busy: false,
    lastOperationId: '',
    phase: '',
    pendingCallIds: [],
  );
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

  test(
    'hub surfaces a prompt and routes respond to the live session',
    () async {
      final container = ProviderContainer();
      addTearDown(container.dispose);
      final hub = container.read(agentPermissionHubProvider.notifier);
      final session = _FakeSession();
      hub.attach('b_bot', session);
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
      expect(session.callId, 'c1');
      expect(session.permission, 'allow_once');
      expect(container.read(agentPermissionHubProvider).of('b_bot'), isEmpty);
    },
  );

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
