import 'dart:async';
import 'dart:convert';

import 'package:flutter/foundation.dart';
import 'package:flutter_secure_storage/test/test_flutter_secure_storage_platform.dart';
// ignore: depend_on_referenced_packages
import 'package:flutter_secure_storage_platform_interface/flutter_secure_storage_platform_interface.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/agent/mention.dart';
import 'package:kim_mobile/agent_bridge.dart';
import 'package:kim_mobile/models/models.dart';
import 'package:kim_mobile/state/agent_profiles.dart';
import 'package:kim_mobile/state/chat_agent.dart';
import 'package:kim_mobile/state/provider_accounts.dart';
import 'package:kim_mobile/state/chat_session.dart';
import 'package:kim_mobile/state/contacts.dart';
import 'package:kim_mobile/state/messages.dart';
import 'package:kim_mobile/state/link.dart';
import 'package:kim_mobile/state/outbox.dart';
import 'package:kim_mobile/state/providers.dart';
import 'package:kim_mobile/state/retry.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:shared_preferences/shared_preferences.dart';

import '../support/harness.dart';

class _RecordingAgentBridge extends AgentBridge {
  SessionOpenOpts? lastOpts;
  var opens = 0;
  final openedIds = <String>[];
  AgentSessionPort? session;
  final sessions = <String, AgentSessionPort>{};

  @override
  Future<void> ensure() async {}

  @override
  Future<AgentSessionPort> open({
    required String sqlitePath,
    required String projectRoot,
    required SessionOpenOpts opts,
  }) async {
    opens += 1;
    lastOpts = opts;
    openedIds.add(opts.sessionId);
    final next = sessions[opts.profileId] ?? session;
    if (next == null) {
      throw StateError('scripted: no live host');
    }
    return next;
  }
}

class _OneShotSession implements AgentSessionPort {
  _OneShotSession(this.reply);

  final String reply;
  final _ctrl = StreamController<AgentUiEvent>.broadcast();

  void emit(AgentUiEvent ev) => _ctrl.add(ev);

  @override
  Stream<AgentUiEvent> listen() => _ctrl.stream;

  @override
  Future<String> promptWithContext({
    required String text,
    required String contextJson,
  }) => prompt(text: text);

  @override
  Future<String> completeTool({
    required String callId,
    required String outputJson,
  }) async => 'op';

  @override
  Future<String> respondPermission({
    required String callId,
    required String permission,
  }) async => 'op';

  @override
  Future<String> prompt({required String text}) async {
    _ctrl.add(
      AgentUiEvent(
        kind: 'assistant_finished',
        operationId: 'op1',
        callId: '',
        name: '',
        delta: '',
        argumentsJson: '',
        outputPreview: '',
        ok: true,
        stopReason: 'completed',
        message: reply,
        inputTokens: BigInt.zero,
        outputTokens: BigInt.zero,
        resumedOps: const [],
      ),
    );
    return 'op1';
  }

  @override
  Future<void> close() async {}

  @override
  Future<void> abort() async {}

  @override
  Future<void> reconfigure({required SessionOpenOpts opts}) async {}

  @override
  Future<ResumeReportDto> resume() async =>
      const ResumeReportDto(resumedOps: [], statuses: []);

  @override
  SessionSnapshotDto snapshot() => const SessionSnapshotDto(
    busy: false,
    lastOperationId: '',
    phase: 'idle',
    pendingCallIds: [],
  );
}

Future<void> _until(bool Function() ok, {int ticks = 80}) async {
  for (var i = 0; i < ticks; i++) {
    if (ok()) {
      return;
    }
    await Future<void>.delayed(Duration.zero);
  }
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  late FlutterSecureStoragePlatform previousPlatform;

  setUp(() {
    debugDefaultTargetPlatformOverride = TargetPlatform.macOS;
    previousPlatform = FlutterSecureStoragePlatform.instance;
  });

  tearDown(() {
    debugDefaultTargetPlatformOverride = null;
    FlutterSecureStoragePlatform.instance = previousPlatform;
  });

  test('first bot prompt after restart loads the saved API key', () async {
    final bridge = _RecordingAgentBridge();
    final env = await kimHarness(
      token: 'tok.jwt',
      account: 'alice',
      overrides: [agentBridgeProvider.overrideWithValue(bridge)],
    );
    final prefs = await SharedPreferences.getInstance();
    await prefs.setString('agent.llm_backend', 'openai');
    await prefs.setString('agent.base_url', 'https://api.openai.com/v1');
    await prefs.setString('agent.model', 'gpt-4o');
    FlutterSecureStoragePlatform.instance = TestFlutterSecureStoragePlatform({
      'agent.api_key': 'sk-live',
    });

    await env.container
        .read(chatAgentProvider)
        .sendDirect(dest: kGooseAgentId, text: 'hello');

    expect(bridge.opens, 1);
    expect(bridge.lastOpts?.apiKey, 'sk-live');
    final bodies = env.container
        .read(threadMessagesProvider(kGooseAgentId))
        .items
        .map((m) => m.body)
        .toList();
    expect(bodies, isNot(contains(contains('未配置 API Key'))));
    expect(bodies.last, contains('Goose 调用失败'));
  });

  test('one assistant_finished yields exactly one assistant bubble', () async {
    final bridge = _RecordingAgentBridge()
      ..session = _OneShotSession('hello from goose');
    final env = await kimHarness(
      token: 'tok.jwt',
      account: 'alice',
      overrides: [agentBridgeProvider.overrideWithValue(bridge)],
    );
    FlutterSecureStoragePlatform.instance = TestFlutterSecureStoragePlatform({
      'agent.api_key': 'sk-live',
    });

    await env.container
        .read(chatAgentProvider)
        .sendDirect(dest: kGooseAgentId, text: 'hi');
    await Future<void>.delayed(Duration.zero);

    final items = env.container
        .read(threadMessagesProvider(kGooseAgentId))
        .items;
    final assistant = items
        .where((m) => m.sender == kGooseAgentName)
        .map((m) => m.body)
        .toList();
    expect(assistant, ['hello from goose']);
  });

  test('action_required upserts a confirmation card by call_id', () async {
    final session = _OneShotSession('done');
    final bridge = _RecordingAgentBridge()..session = session;
    final env = await kimHarness(
      token: 'tok.jwt',
      account: 'alice',
      overrides: [agentBridgeProvider.overrideWithValue(bridge)],
    );
    FlutterSecureStoragePlatform.instance = TestFlutterSecureStoragePlatform({
      'agent.api_key': 'sk-live',
    });
    await env.container
        .read(chatAgentProvider)
        .sendDirect(dest: kGooseAgentId, text: 'hi');
    session.emit(
      AgentUiEvent(
        kind: 'action_required',
        operationId: 'op1',
        callId: 'c1',
        name: 'send_message',
        delta: '',
        argumentsJson: '{"dest":"bob","text":"hi"}',
        outputPreview: '',
        ok: false,
        stopReason: '',
        message: 'Allow send_message?',
        inputTokens: BigInt.zero,
        outputTokens: BigInt.zero,
        resumedOps: const [],
      ),
    );
    await Future<void>.delayed(Duration.zero);
    final cards = env.container
        .read(threadMessagesProvider(kGooseAgentId))
        .items
        .where((m) => m.key == 'agent-card-c1')
        .toList();
    expect(cards, hasLength(1));
    expect(cards.first.body, contains('action_required'));
    expect(cards.first.body, contains('pending'));
  });

  test('two profiles open two sessions on the same thread dest', () async {
    final session = _OneShotSession('ok');
    final bridge = _RecordingAgentBridge()..session = session;
    final env = await kimHarness(
      token: 'tok.jwt',
      account: 'alice',
      overrides: [agentBridgeProvider.overrideWithValue(bridge)],
    );
    FlutterSecureStoragePlatform.instance = TestFlutterSecureStoragePlatform({
      'agent.api_key': 'sk-live',
    });
    final prefs = await SharedPreferences.getInstance();
    await prefs.setBool('agent.multi_profile', true);
    final store = env.container.read(agentProfilesProvider.notifier);
    await store.ensureLoaded();
    final goose = store.goose!;
    await store.duplicate(goose);
    final copy = env.container
        .read(agentProfilesProvider)
        .firstWhere((p) => p.id != kGooseAgentId);
    await env.container
        .read(chatAgentProvider)
        .sendDirect(dest: kGooseAgentId, text: 'hi');
    await env.container
        .read(chatAgentProvider)
        .sendDirect(dest: copy.dest, text: 'hi');
    expect(bridge.opens, 2);
    expect(bridge.openedIds.toSet().length, 2);
    await Future<void>.delayed(Duration.zero);
    final senders = env.container
        .read(threadMessagesProvider(copy.dest))
        .items
        .map((m) => m.sender)
        .toSet();
    expect(senders, contains(copy.displayName));
    expect(senders, isNot(equals({kGooseAgentName})));
  });

  test(
    'action_required card uses profile displayName and profile_id',
    () async {
      final session = _OneShotSession('done');
      final bridge = _RecordingAgentBridge()..session = session;
      final env = await kimHarness(
        token: 'tok.jwt',
        account: 'alice',
        overrides: [agentBridgeProvider.overrideWithValue(bridge)],
      );
      FlutterSecureStoragePlatform.instance = TestFlutterSecureStoragePlatform({
        'agent.api_key': 'sk-live',
      });
      final prefs = await SharedPreferences.getInstance();
      await prefs.setBool('agent.multi_profile', true);
      final store = env.container.read(agentProfilesProvider.notifier);
      await store.ensureLoaded();
      await store.duplicate(store.goose!);
      final copy = env.container
          .read(agentProfilesProvider)
          .firstWhere((p) => p.id != kGooseAgentId);
      await env.container
          .read(chatAgentProvider)
          .sendDirect(dest: copy.dest, text: 'hi');
      session.emit(
        AgentUiEvent(
          kind: 'action_required',
          operationId: 'op1',
          callId: 'c-copy',
          name: 'send_message',
          delta: '',
          argumentsJson: '{"dest":"bob","text":"hi"}',
          outputPreview: '',
          ok: false,
          stopReason: '',
          message: 'Allow send_message?',
          inputTokens: BigInt.zero,
          outputTokens: BigInt.zero,
          resumedOps: const [],
        ),
      );
      await Future<void>.delayed(Duration.zero);
      final cards = env.container
          .read(threadMessagesProvider(copy.dest))
          .items
          .where((m) => m.key == 'agent-card-c-copy')
          .toList();
      expect(cards, hasLength(1));
      expect(cards.first.sender, copy.displayName);
      expect(cards.first.body, contains('"profile_id":"${copy.id}"'));
    },
  );

  test('respondPermission routes by profile_id on the card', () async {
    final gooseSession = _RecordingSession();
    final copySession = _RecordingSession();
    final bridge = _RecordingAgentBridge()
      ..sessions['goose'] = gooseSession
      ..session = gooseSession;
    final env = await kimHarness(
      token: 'tok.jwt',
      account: 'alice',
      overrides: [agentBridgeProvider.overrideWithValue(bridge)],
    );
    FlutterSecureStoragePlatform.instance = TestFlutterSecureStoragePlatform({
      'agent.api_key': 'sk-live',
    });
    final prefs = await SharedPreferences.getInstance();
    await prefs.setBool('agent.multi_profile', true);
    final store = env.container.read(agentProfilesProvider.notifier);
    await store.ensureLoaded();
    await store.duplicate(store.goose!);
    final copy = env.container
        .read(agentProfilesProvider)
        .firstWhere((p) => p.id != kGooseAgentId);
    bridge.sessions[copy.id] = copySession;
    await env.container
        .read(chatAgentProvider)
        .onOutgoingText(dest: 'bob', text: '@助手 ping');
    await env.container
        .read(chatAgentProvider)
        .onOutgoingText(dest: 'bob', text: '@${copy.id} ping');
    await Future<void>.delayed(Duration.zero);
    gooseSession.emit(_actionRequired('c-goose'));
    copySession.emit(_actionRequired('c-copy'));
    await Future<void>.delayed(Duration.zero);
    await env.container
        .read(chatAgentProvider)
        .respondPermission(
          dest: 'bob',
          callId: 'c-copy',
          permission: 'allow_once',
          toolName: 'send_message',
        );
    expect(copySession.permissions, ['c-copy:allow_once']);
    expect(gooseSession.permissions, isEmpty);
  });

  test('respondPermission no-ops when keyed live is gone', () async {
    final gooseSession = _RecordingSession();
    final copySession = _RecordingSession();
    final bridge = _RecordingAgentBridge()
      ..sessions['goose'] = gooseSession
      ..session = gooseSession;
    final env = await kimHarness(
      token: 'tok.jwt',
      account: 'alice',
      overrides: [agentBridgeProvider.overrideWithValue(bridge)],
    );
    FlutterSecureStoragePlatform.instance = TestFlutterSecureStoragePlatform({
      'agent.api_key': 'sk-live',
    });
    final prefs = await SharedPreferences.getInstance();
    await prefs.setBool('agent.multi_profile', true);
    final store = env.container.read(agentProfilesProvider.notifier);
    await store.ensureLoaded();
    await store.duplicate(store.goose!);
    final copy = env.container
        .read(agentProfilesProvider)
        .firstWhere((p) => p.id != kGooseAgentId);
    bridge.sessions[copy.id] = copySession;
    await env.container
        .read(chatAgentProvider)
        .onOutgoingText(dest: 'bob', text: '@助手 ping');
    await env.container
        .read(chatAgentProvider)
        .onOutgoingText(dest: 'bob', text: '@${copy.id} ping');
    await Future<void>.delayed(Duration.zero);
    gooseSession.emit(_actionRequired('c-goose'));
    await Future<void>.delayed(Duration.zero);
    await env.container
        .read(chatAgentProvider)
        .sendDirect(dest: 'd1', text: 'hi');
    await env.container
        .read(chatAgentProvider)
        .sendDirect(dest: 'd2', text: 'hi');
    await env.container
        .read(chatAgentProvider)
        .sendDirect(dest: 'd3', text: 'hi');
    await env.container
        .read(chatAgentProvider)
        .respondPermission(
          dest: 'bob',
          callId: 'c-goose',
          permission: 'allow_once',
          toolName: 'send_message',
        );
    expect(copySession.permissions, isEmpty);
    expect(gooseSession.permissions, isEmpty);
  });

  test('AlwaysAllow persists onto the acting profile', () async {
    final session = _RecordingSession();
    final bridge = _RecordingAgentBridge()..session = session;
    final env = await kimHarness(
      token: 'tok.jwt',
      account: 'alice',
      overrides: [agentBridgeProvider.overrideWithValue(bridge)],
    );
    FlutterSecureStoragePlatform.instance = TestFlutterSecureStoragePlatform({
      'agent.api_key': 'sk-live',
    });
    final prefs = await SharedPreferences.getInstance();
    await prefs.setBool('agent.multi_profile', true);
    final store = env.container.read(agentProfilesProvider.notifier);
    await store.ensureLoaded();
    await store.duplicate(store.goose!);
    final copy = env.container
        .read(agentProfilesProvider)
        .firstWhere((p) => p.id != kGooseAgentId);
    bridge.sessions[copy.id] = session;
    await env.container
        .read(chatAgentProvider)
        .sendDirect(dest: copy.dest, text: 'hi');
    session.emit(_actionRequired('c1'));
    await Future<void>.delayed(Duration.zero);
    await env.container
        .read(chatAgentProvider)
        .respondPermission(
          dest: copy.dest,
          callId: 'c1',
          permission: 'always_allow',
          toolName: 'send_message',
        );
    final updated = env.container
        .read(agentProfilesProvider)
        .firstWhere((p) => p.id == copy.id);
    expect(updated.permissionOverrides['send_message'], 'always_allow');
    expect(
      env.container
          .read(agentProfilesProvider.notifier)
          .goose
          ?.permissionOverrides['send_message'],
      isNot('always_allow'),
    );
  });

  test('hidden context blob is chronological', () async {
    final session = _RecordingSession();
    final bridge = _RecordingAgentBridge()..session = session;
    final env = await kimHarness(
      token: 'tok.jwt',
      account: 'alice',
      overrides: [agentBridgeProvider.overrideWithValue(bridge)],
    );
    FlutterSecureStoragePlatform.instance = TestFlutterSecureStoragePlatform({
      'agent.api_key': 'sk-live',
    });
    final bob = env.container.read(threadMessagesProvider('bob').notifier);
    bob.receive(
      const KimChatMsg(
        key: 'm1',
        dest: 'bob',
        sender: 'bob',
        body: 'first',
        at: 1,
      ),
    );
    bob.receive(
      const KimChatMsg(
        key: 'm2',
        dest: 'bob',
        sender: 'alice',
        body: 'second',
        at: 2,
      ),
    );
    await env.container
        .read(chatAgentProvider)
        .onOutgoingText(dest: 'bob', text: '@助手 总结');
    expect(session.lastContext, isNotNull);
    expect(
      session.lastContext!.indexOf('first'),
      lessThan(session.lastContext!.indexOf('second')),
    );
  });

  test(
    'opens with shared account key rather than only agent.api_key',
    () async {
      final session = _OneShotSession('ok');
      final bridge = _RecordingAgentBridge()..session = session;
      final env = await kimHarness(
        token: 'tok.jwt',
        account: 'alice',
        overrides: [agentBridgeProvider.overrideWithValue(bridge)],
      );
      FlutterSecureStoragePlatform.instance = TestFlutterSecureStoragePlatform({
        'agent.api_key': 'sk-legacy',
        'agent.api_key.goose': 'sk-goose',
      });
      final prefs = await SharedPreferences.getInstance();
      await prefs.setBool('agent.multi_profile', true);
      final store = env.container.read(agentProfilesProvider.notifier);
      await store.ensureLoaded();
      await store.duplicate(store.goose!);
      final copy = env.container
          .read(agentProfilesProvider)
          .firstWhere((p) => p.id != kGooseAgentId);
      await env.container
          .read(chatAgentProvider)
          .sendDirect(dest: copy.dest, text: 'hi');
      expect(bridge.lastOpts?.apiKey, 'sk-goose');
    },
  );

  test('profile save with tool change reopens the live session', () async {
    final session = _OneShotSession('ok');
    final bridge = _RecordingAgentBridge()..session = session;
    final env = await kimHarness(
      token: 'tok.jwt',
      account: 'alice',
      overrides: [agentBridgeProvider.overrideWithValue(bridge)],
    );
    FlutterSecureStoragePlatform.instance = TestFlutterSecureStoragePlatform({
      'agent.api_key': 'sk-live',
    });
    await env.container
        .read(chatAgentProvider)
        .sendDirect(dest: kGooseAgentId, text: 'hi');
    expect(bridge.opens, 1);
    final store = env.container.read(agentProfilesProvider.notifier);
    await store.ensureLoaded();
    final goose = store.goose!;
    await store.saveGoose(goose.copyWith(model: 'gpt-4.1'), apiKey: 'sk-live');
    await Future<void>.delayed(Duration.zero);
    await env.container
        .read(chatAgentProvider)
        .sendDirect(dest: kGooseAgentId, text: 'again');
    expect(bridge.opens, 2);
  });

  test('duplicate shares account_id and does not copy the key', () async {
    final env = await kimHarness(token: 'tok.jwt', account: 'alice');
    FlutterSecureStoragePlatform.instance = TestFlutterSecureStoragePlatform({
      'agent.api_key': 'sk-live',
      'agent.api_key.goose': 'sk-live',
    });
    final store = env.container.read(agentProfilesProvider.notifier);
    await store.ensureLoaded();
    await store.duplicate(store.goose!);
    final copy = env.container
        .read(agentProfilesProvider)
        .firstWhere((p) => p.id != kGooseAgentId);
    expect(copy.accountId, store.goose!.accountId);
    expect(copy.accountId, isNotEmpty);
    expect(await store.readApiKey(copy), 'sk-live');
  });

  test('server identity flag persists', () async {
    final env = await kimHarness(token: 'tok.jwt', account: 'alice');
    final store = env.container.read(agentProfilesProvider.notifier);
    await store.ensureLoaded();
    expect(store.serverIdentity, isFalse);
    await store.setServerIdentity(true);
    expect(store.serverIdentity, isTrue);
    final prefs = await SharedPreferences.getInstance();
    expect(prefs.getBool('agent.server_identity'), isTrue);
  });

  test('new profile save registers on the server when flag is on', () async {
    final env = await kimHarness(token: 'tok.jwt', account: 'alice');
    final store = env.container.read(agentProfilesProvider.notifier);
    await store.ensureLoaded();
    await store.setServerIdentity(true);
    env.fake.botCreates = 0;
    final created = AgentProfile(
      id: 'translator',
      displayName: '译者',
      providerKind: 'openai',
      baseUrl: '',
      model: 'gpt-4o',
      keyRef: 'agent.api_key.translator',
      systemPrompt: '',
    );
    await store.saveProfile(created);
    expect(env.fake.botCreates, 1);
    expect(env.fake.lastBotCreateId, 'translator');
    expect(
      env.container
          .read(agentProfilesProvider)
          .firstWhere((p) => p.id == 'translator')
          .serverAccount,
      'b_translator',
    );
    await store.saveProfile(
      created.copyWith(model: 'gpt-4.1', serverAccount: 'b_translator'),
    );
    expect(env.fake.botCreates, 1);
  });

  test('login does not register; turning identity on does', () async {
    final env = await kimHarness(token: 'tok.jwt', account: 'alice');
    final store = env.container.read(agentProfilesProvider.notifier);
    await store.ensureLoaded();
    expect(env.fake.botCreates, 0);
    await store.setServerIdentity(true);
    expect(env.fake.botCreates, 1);
    expect(env.fake.lastBotCreateId, kGooseAgentId);
    expect(store.goose!.serverAccount, 'b_goose');
  });

  test('botCreate failure is visible and not swallowed', () async {
    final env = await kimHarness(token: 'tok.jwt', account: 'alice');
    env.fake.botCreateError = StateError('status 2');
    final store = env.container.read(agentProfilesProvider.notifier);
    await store.ensureLoaded();
    await store.setServerIdentity(true);
    expect(store.identityError, isNotEmpty);
    expect(store.goose!.serverAccount, isEmpty);
  });

  test('turning on identity after goose 1:1 is open still registers', () async {
    final env = await kimHarness(token: 'tok.jwt', account: 'alice');
    final store = env.container.read(agentProfilesProvider.notifier);
    await store.ensureLoaded();
    env.container.read(chatSessionProvider(kGooseAgentId));
    await Future<void>.delayed(Duration.zero);
    expect(env.fake.botCreates, 0);
    await store.setServerIdentity(true);
    await _until(() => env.fake.botCreates == 1);
    expect(env.fake.lastBotCreateId, kGooseAgentId);
    expect(
      env.container.read(chatSessionProvider(kGooseAgentId)).redirectDest,
      'b_goose',
    );
  });

  test('sendText on open goose dest after flag on goes to server', () async {
    final env = await kimHarness(token: 'tok.jwt', account: 'alice');
    env.container.read(linkProvider);
    await Future<void>.delayed(Duration.zero);
    final store = env.container.read(agentProfilesProvider.notifier);
    await store.ensureLoaded();
    final session = env.container.read(
      chatSessionProvider(kGooseAgentId).notifier,
    );
    await Future<void>.delayed(Duration.zero);
    await store.setServerIdentity(true);
    await _until(() => env.fake.botCreates == 1);
    final ok = await session.sendText('hello from pc');
    expect(ok, isTrue);
    await _until(() => env.fake.talks == 1);
    expect(env.fake.lastTalkDest, 'b_goose');
    expect(env.fake.lastTalkBody, 'hello from pc');
  });

  test('profileForDest matches serverAccount', () async {
    final env = await kimHarness(token: 'tok.jwt', account: 'alice');
    final store = env.container.read(agentProfilesProvider.notifier);
    await store.ensureLoaded();
    await store.saveProfile(store.goose!.copyWith(serverAccount: 'b_XXX'));
    final hit = await env.container
        .read(chatAgentProvider)
        .profileForDest('b_XXX');
    expect(hit.id, kGooseAgentId);
    expect(hit.serverAccount, 'b_XXX');
  });

  test('flag on registered: enqueue does not prompt until TalkResp', () async {
    final bridge = _RecordingAgentBridge()..session = _OneShotSession('ok');
    final env = await kimHarness(
      token: 'tok.jwt',
      account: 'alice',
      overrides: [agentBridgeProvider.overrideWithValue(bridge)],
    );
    FlutterSecureStoragePlatform.instance = TestFlutterSecureStoragePlatform({
      'agent.api_key': 'sk-live',
      'agent.api_key.goose': 'sk-live',
    });
    env.container.read(linkProvider);
    await Future<void>.delayed(Duration.zero);
    final store = env.container.read(agentProfilesProvider.notifier);
    await store.ensureLoaded();
    await store.setServerIdentity(true);
    await store.saveProfile(store.goose!.copyWith(serverAccount: 'b_bot'));
    env.fake.friends = const [
      KimPerson(account: 'b_bot', nickname: '助手', kind: ProfileKind.bot),
    ];
    await env.container.read(contactsProvider.notifier).refresh();
    env.fake.sendHold = Completer<void>();
    final sent = env.container
        .read(outboxProvider.notifier)
        .sendText('b_bot', 'hello');
    await Future<void>.delayed(Duration.zero);
    expect(bridge.opens, 0);
    env.fake.sendHold!.complete();
    await sent;
    await _until(() => bridge.opens == 1);
    expect(bridge.opens, 1);
    expect(env.fake.botReplies, hasLength(1));
    expect(env.fake.botReplies.single.inReplyTo, 1);
    final localAgent = env.container
        .read(threadMessagesProvider('b_bot'))
        .items
        .where((m) => m.key.startsWith('agent-'));
    expect(localAgent, isEmpty);
  });

  test(
    'FIFO: two TalkResp plus overlapping pending yield three serial bot_reply',
    () async {
      final session = _HoldSession();
      final bridge = _RecordingAgentBridge()..session = session;
      final env = await kimHarness(
        token: 'tok.jwt',
        account: 'alice',
        overrides: [agentBridgeProvider.overrideWithValue(bridge)],
      );
      FlutterSecureStoragePlatform.instance = TestFlutterSecureStoragePlatform({
        'agent.api_key': 'sk-live',
        'agent.api_key.goose': 'sk-live',
      });
      env.container.read(linkProvider);
      await Future<void>.delayed(Duration.zero);
      final store = env.container.read(agentProfilesProvider.notifier);
      await store.ensureLoaded();
      await store.setServerIdentity(true);
      await store.saveProfile(store.goose!.copyWith(serverAccount: 'b_bot'));
      expect(store.serverIdentity, isTrue);
      expect(store.goose!.serverAccount, 'b_bot');
      final agent = env.container.read(chatAgentProvider);
      expect(agent.enqueueTurn('b_bot', 'one', 1), isTrue);
      expect(agent.enqueueTurn('b_bot', 'two', 2), isTrue);
      env.fake.pendingItems = const [
        KimBotPendingItem(messageId: 1, body: 'one', sendTime: 1),
        KimBotPendingItem(messageId: 2, body: 'two', sendTime: 2),
        KimBotPendingItem(messageId: 3, body: 'three', sendTime: 3),
      ];
      await agent.catchUpPending();
      await _until(() => session.prompts.length == 1);
      expect(session.prompts, ['one']);
      expect(agent.promptMaxInFlight, 1);
      session.release();
      await _until(() => session.prompts.length == 2);
      expect(session.prompts, ['one', 'two']);
      expect(agent.promptMaxInFlight, 1);
      session.release();
      await _until(() => session.prompts.length == 3);
      expect(session.prompts, ['one', 'two', 'three']);
      expect(agent.promptMaxInFlight, 1);
      session.release();
      await _until(() => env.fake.botReplies.length == 3);
      expect(env.fake.botReplies.map((r) => r.inReplyTo), [1, 2, 3]);
      expect(env.fake.botReplyMaxInFlight, 1);
    },
  );

  test(
    'empty finished does not complete; late finished does not steal next turn',
    () async {
      final session = _HoldSession();
      final bridge = _RecordingAgentBridge()..session = session;
      final env = await kimHarness(
        token: 'tok.jwt',
        account: 'alice',
        overrides: [agentBridgeProvider.overrideWithValue(bridge)],
      );
      FlutterSecureStoragePlatform.instance = TestFlutterSecureStoragePlatform({
        'agent.api_key': 'sk-live',
        'agent.api_key.goose': 'sk-live',
      });
      env.container.read(linkProvider);
      await Future<void>.delayed(Duration.zero);
      final store = env.container.read(agentProfilesProvider.notifier);
      await store.ensureLoaded();
      await store.setServerIdentity(true);
      await store.saveProfile(store.goose!.copyWith(serverAccount: 'b_bot'));
      final agent = env.container.read(chatAgentProvider);
      expect(agent.enqueueTurn('b_bot', 'one', 1), isTrue);
      expect(agent.enqueueTurn('b_bot', 'two', 2), isTrue);
      await _until(() => session.prompts.length == 1);
      session.emit(_finished(''));
      await Future<void>.delayed(Duration.zero);
      expect(env.fake.botReplies, isEmpty);
      expect(session.prompts, ['one']);
      session.emit(_failed('boom'));
      await _until(() => session.prompts.length == 2);
      expect(env.fake.botReplies, isEmpty);
      session.emit(_finished('late'));
      await Future<void>.delayed(Duration.zero);
      expect(env.fake.botReplies, isEmpty);
      session.release();
      await _until(() => env.fake.botReplies.length == 1);
      expect(env.fake.botReplies.single.inReplyTo, 2);
    },
  );

  test('disk JSON without provider opens DeepSeek with catalog kind', () async {
    final session = _OneShotSession('ok');
    final bridge = _RecordingAgentBridge()..session = session;
    final env = await kimHarness(
      token: 'tok.jwt',
      account: 'alice',
      overrides: [agentBridgeProvider.overrideWithValue(bridge)],
    );
    FlutterSecureStoragePlatform.instance = TestFlutterSecureStoragePlatform({
      'agent.api_key.acct.acct-ds': 'sk-ds',
    });
    final accounts = env.container.read(providerAccountsProvider.notifier);
    await accounts.ensureLoaded();
    await accounts.upsert(
      const ProviderAccount(
        id: 'acct-ds',
        vendorId: 'deepseek',
        baseUrl: 'https://api.deepseek.com',
        keyRef: 'agent.api_key.acct.acct-ds',
        displayName: 'DeepSeek',
      ),
    );
    final store = env.container.read(agentProfilesProvider.notifier);
    await store.ensureLoaded();
    await store.saveGoose(
      store.goose!.copyWith(
        accountId: 'acct-ds',
        providerKind: 'deepseek',
        baseUrl: 'https://api.deepseek.com',
        model: 'deepseek-flash',
        reasoning: const ReasoningChoice(kind: 'effort_enum', value: 'high'),
      ),
      apiKey: 'sk-ds',
    );
    final disk = jsonDecode(
      (await SharedPreferences.getInstance()).getString('agent.profiles')!,
    );
    expect(disk.first['provider'], isNull);
    await env.container
        .read(chatAgentProvider)
        .sendDirect(dest: kGooseAgentId, text: 'hi');
    expect(bridge.lastOpts?.llmBackend, 'deepseek');
    expect(bridge.lastOpts?.apiKey, 'sk-ds');
    final host = jsonDecode(bridge.lastOpts!.profileJson) as Map;
    expect(host['provider']['kind'], 'deepseek');
    expect(host['reasoning']['value'], 'high');
    expect(host['model']['name'], 'deepseek-flash');
  });

  test('save ProviderAccount does not call botCreate', () async {
    final env = await kimHarness(token: 'tok.jwt', account: 'alice');
    final store = env.container.read(agentProfilesProvider.notifier);
    await store.ensureLoaded();
    await store.setServerIdentity(true);
    env.fake.botCreates = 0;
    final accounts = env.container.read(providerAccountsProvider.notifier);
    await accounts.upsert(
      const ProviderAccount(
        id: 'acct-new',
        vendorId: 'groq',
        baseUrl: 'https://api.groq.com/openai/v1',
        keyRef: 'agent.api_key.acct.acct-new',
        displayName: 'Groq',
      ),
    );
    expect(env.fake.botCreates, 0);
  });

  test('missing account refuses open without stale kind fallback', () async {
    final env = await kimHarness(token: 'tok.jwt', account: 'alice');
    final store = env.container.read(agentProfilesProvider.notifier);
    await store.ensureLoaded();
    final id = store.goose!.accountId;
    expect(id, isNotEmpty);
    await env.container.read(providerAccountsProvider.notifier).delete(id);
    expect(
      () => store.readApiKey(store.goose!),
      throwsA(isA<MissingProviderAccount>()),
    );
  });

  test('reload does not resurrect a missing account as OpenAI', () async {
    final env = await kimHarness(token: 'tok.jwt', account: 'alice');
    FlutterSecureStoragePlatform.instance = TestFlutterSecureStoragePlatform({
      'agent.api_key.acct.acct-ds': 'sk-ds',
    });
    final accounts = env.container.read(providerAccountsProvider.notifier);
    await accounts.ensureLoaded();
    await accounts.upsert(
      const ProviderAccount(
        id: 'acct-ds',
        vendorId: 'deepseek',
        baseUrl: 'https://api.deepseek.com',
        keyRef: 'agent.api_key.acct.acct-ds',
        displayName: 'DeepSeek',
      ),
    );
    final store = env.container.read(agentProfilesProvider.notifier);
    await store.ensureLoaded();
    await store.saveGoose(
      store.goose!.copyWith(
        accountId: 'acct-ds',
        providerKind: 'deepseek',
        baseUrl: 'https://api.deepseek.com',
        model: 'deepseek-flash',
      ),
      apiKey: 'sk-ds',
    );
    final prefs = await SharedPreferences.getInstance();
    final disk = jsonDecode(prefs.getString('agent.profiles')!) as List;
    expect(disk.first['account_id'], 'acct-ds');
    expect(disk.first['provider'], isNull);
    await prefs.setString(kProviderAccountsPref, '[]');

    final container2 = ProviderContainer.test(
      retry: kimRetry,
      overrides: kimProviderOverrides(
        runtime: env.runtime,
        auth: env.fake,
        client: env.fake,
        store: env.store,
        media: env.media,
      ),
    );
    addTearDown(container2.dispose);
    final store2 = container2.read(agentProfilesProvider.notifier);
    await store2.ensureLoaded();
    expect(store2.goose!.accountId, 'acct-ds');
    expect(container2.read(providerAccountsProvider), isEmpty);
    expect(
      () => store2.readApiKey(store2.goose!),
      throwsA(isA<MissingProviderAccount>()),
    );
  });

  Future<void> addProfile(AgentProfileStore store, String id) {
    return store.saveProfile(
      AgentProfile(
        id: id,
        displayName: id,
        providerKind: 'openai',
        baseUrl: '',
        model: 'gpt-4o',
        keyRef: 'agent.api_key.$id',
        systemPrompt: '',
      ),
    );
  }

  test('new profile immediately botCreate; goose is not deletable', () async {
    final env = await kimHarness(token: 'tok.jwt', account: 'alice');
    final store = env.container.read(agentProfilesProvider.notifier);
    await store.ensureLoaded();
    await store.setServerIdentity(true);
    env.fake.botCreates = 0;
    await addProfile(store, 'coder');
    expect(env.fake.botCreates, 1);
    expect(env.fake.lastBotCreateId, 'coder');
    await store.delete(kGooseAgentId);
    expect(store.goose, isNotNull);
    expect(env.fake.botDeletes, 0);
  });

  test(
    'delete registered calls botDelete and 108 still clears local',
    () async {
      final env = await kimHarness(token: 'tok.jwt', account: 'alice');
      final store = env.container.read(agentProfilesProvider.notifier);
      await store.ensureLoaded();
      await store.setServerIdentity(true);
      await addProfile(store, 'translator');
      expect(
        env.container
            .read(agentProfilesProvider)
            .any((p) => p.id == 'translator'),
        isTrue,
      );
      env.fake.botDeletes = 0;
      await store.delete('translator');
      expect(env.fake.botDeletes, 1);
      expect(env.fake.lastBotDeleteDest, 'b_translator');
      expect(
        env.container
            .read(agentProfilesProvider)
            .any((p) => p.id == 'translator'),
        isFalse,
      );

      await addProfile(store, 'gone');
      env.fake.botDeleteError = StateError('status 108');
      await store.delete('gone');
      expect(
        env.container.read(agentProfilesProvider).any((p) => p.id == 'gone'),
        isFalse,
      );

      env.fake.botDeleteError = null;
      await addProfile(store, 'keep');
      env.fake.botDeleteError = StateError('status 2');
      await expectLater(store.delete('keep'), throwsA(isA<Object>()));
      expect(store.identityError, isNotEmpty);
      expect(
        env.container.read(agentProfilesProvider).any((p) => p.id == 'keep'),
        isTrue,
      );
    },
  );

  test('disabled profiles count toward the 20-cap', () async {
    final env = await kimHarness(token: 'tok.jwt', account: 'alice');
    final store = env.container.read(agentProfilesProvider.notifier);
    await store.ensureLoaded();
    await store.setServerIdentity(true);
    for (var i = 0; i < 18; i++) {
      await addProfile(store, 'p$i');
    }
    await addProfile(store, 'disabled');
    await store.setEnabled('disabled', false);
    expect(
      cloudIdentitySlots(
        env.container.read(agentProfilesProvider),
        serverIdentity: true,
      ),
      kMaxBotsPerOwner,
    );
    expect(
      addProfile(store, 'overflow'),
      throwsA(isA<AgentProfileCapExceeded>()),
    );
    expect(
      env.container.read(agentProfilesProvider).any((p) => p.id == 'overflow'),
      isFalse,
    );
  });

  test('translator and coder templates do not use host fixtures', () async {
    final env = await kimHarness(token: 'tok.jwt', account: 'alice');
    final store = env.container.read(agentProfilesProvider.notifier);
    await store.ensureLoaded();
    await store.setServerIdentity(true);
    env.fake.botCreates = 0;
    final translator = await store.createFromTemplate(
      AgentProfileStore.templateTranslator,
    );
    expect(translator.model, 'deepseek-flash');
    expect(translator.providerKind, 'deepseek');
    expect(translator.reasoning?.value, 'none');
    expect(translator.tools.fs, isFalse);
    expect(translator.tools.sendMessage, isFalse);
    expect(env.fake.botCreates, 1);
    final accounts = env.container.read(providerAccountsProvider);
    expect(accounts.any((a) => a.vendorId == 'deepseek'), isTrue);
    env.fake.botCreates = 0;
    await env.container
        .read(providerAccountsProvider.notifier)
        .upsert(
          const ProviderAccount(
            id: 'acct-custom',
            vendorId: 'openai_compatible',
            baseUrl: 'https://127.0.0.1:8000/v1',
            keyRef: 'agent.api_key.acct.acct-custom',
          ),
        );
    expect(env.fake.botCreates, 0);
    final coder = await store.createFromTemplate(
      AgentProfileStore.templateCoder,
    );
    expect(coder.model, 'claude-sonnet-4-5');
    expect(coder.providerKind, 'anthropic');
    expect(coder.reasoning?.value, 'high');
    expect(coder.tools.fs, isTrue);
  });

  test('login/online does not batch-ensure bot identities', () async {
    final env = await kimHarness(token: 'tok.jwt', account: 'alice');
    final store = env.container.read(agentProfilesProvider.notifier);
    await store.ensureLoaded();
    store.serverIdentity = true;
    env.fake.botCreates = 0;
    env.container.read(linkProvider);
    await _until(
      () => env.container.read(linkProvider).status == ConnStatus.online,
    );
    expect(env.fake.botCreates, 0);
    expect(store.goose!.serverAccount, isEmpty);
  });
}

AgentUiEvent _finished(String message) => AgentUiEvent(
  kind: 'assistant_finished',
  operationId: 'op1',
  callId: '',
  name: '',
  delta: '',
  argumentsJson: '',
  outputPreview: '',
  ok: true,
  stopReason: 'completed',
  message: message,
  inputTokens: BigInt.zero,
  outputTokens: BigInt.zero,
  resumedOps: const [],
);

AgentUiEvent _failed(String message) => AgentUiEvent(
  kind: 'failed',
  operationId: 'op1',
  callId: '',
  name: '',
  delta: '',
  argumentsJson: '',
  outputPreview: '',
  ok: false,
  stopReason: 'error',
  message: message,
  inputTokens: BigInt.zero,
  outputTokens: BigInt.zero,
  resumedOps: const [],
);

class _RecordingSession implements AgentSessionPort {
  final permissions = <String>[];
  String? lastContext;
  final _ctrl = StreamController<AgentUiEvent>.broadcast();

  void emit(AgentUiEvent ev) => _ctrl.add(ev);

  @override
  Stream<AgentUiEvent> listen() => _ctrl.stream;

  @override
  Future<String> promptWithContext({
    required String text,
    required String contextJson,
  }) async {
    lastContext = contextJson;
    return prompt(text: text);
  }

  @override
  Future<String> completeTool({
    required String callId,
    required String outputJson,
  }) async => 'op';

  @override
  Future<String> respondPermission({
    required String callId,
    required String permission,
  }) async {
    permissions.add('$callId:$permission');
    return 'op';
  }

  @override
  Future<String> prompt({required String text}) async => 'op1';

  @override
  Future<void> close() async {}

  @override
  Future<void> abort() async {}

  @override
  Future<void> reconfigure({required SessionOpenOpts opts}) async {}

  @override
  Future<ResumeReportDto> resume() async =>
      const ResumeReportDto(resumedOps: [], statuses: []);

  @override
  SessionSnapshotDto snapshot() => const SessionSnapshotDto(
    busy: false,
    lastOperationId: '',
    phase: 'idle',
    pendingCallIds: [],
  );
}

class _HoldSession implements AgentSessionPort {
  final prompts = <String>[];
  Completer<void>? _gate;
  final _ctrl = StreamController<AgentUiEvent>.broadcast();

  void emit(AgentUiEvent ev) => _ctrl.add(ev);

  void release() {
    final gate = _gate;
    _gate = Completer<void>();
    gate?.complete();
  }

  @override
  Stream<AgentUiEvent> listen() => _ctrl.stream;

  @override
  Future<String> promptWithContext({
    required String text,
    required String contextJson,
  }) => prompt(text: text);

  @override
  Future<String> completeTool({
    required String callId,
    required String outputJson,
  }) async => 'op';

  @override
  Future<String> respondPermission({
    required String callId,
    required String permission,
  }) async => 'op';

  @override
  Future<String> prompt({required String text}) async {
    prompts.add(text);
    _gate ??= Completer<void>();
    await _gate!.future;
    _ctrl.add(
      AgentUiEvent(
        kind: 'assistant_finished',
        operationId: 'op1',
        callId: '',
        name: '',
        delta: '',
        argumentsJson: '',
        outputPreview: '',
        ok: true,
        stopReason: 'completed',
        message: 'reply:$text',
        inputTokens: BigInt.zero,
        outputTokens: BigInt.zero,
        resumedOps: const [],
      ),
    );
    return 'op1';
  }

  @override
  Future<void> close() async {}

  @override
  Future<void> abort() async {}

  @override
  Future<void> reconfigure({required SessionOpenOpts opts}) async {}

  @override
  Future<ResumeReportDto> resume() async =>
      const ResumeReportDto(resumedOps: [], statuses: []);

  @override
  SessionSnapshotDto snapshot() => const SessionSnapshotDto(
    busy: false,
    lastOperationId: '',
    phase: 'idle',
    pendingCallIds: [],
  );
}

AgentUiEvent _actionRequired(String callId) {
  return AgentUiEvent(
    kind: 'action_required',
    operationId: 'op1',
    callId: callId,
    name: 'send_message',
    delta: '',
    argumentsJson: '{"dest":"bob","text":"hi"}',
    outputPreview: '',
    ok: false,
    stopReason: '',
    message: 'Allow send_message?',
    inputTokens: BigInt.zero,
    outputTokens: BigInt.zero,
    resumedOps: const [],
  );
}
