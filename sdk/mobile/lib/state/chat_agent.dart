/// Runs the local Goose host when an IM message @mentions 助手.
library;

import 'dart:async';
import 'dart:convert';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:uuid/uuid.dart';

import '../agent/capability_host.dart';
import '../agent/host_support.dart';
import '../agent/mention.dart';
import '../agent_bridge.dart';
import '../core/paths.dart';
import '../models/models.dart';
import 'agent_profiles.dart';
import 'agent_settings.dart';
import 'auth.dart';
import 'inbox.dart';
import 'messages.dart';
import 'providers.dart';

class _Live {
  _Live({
    required this.session,
    required this.threadDest,
    required this.profile,
  });

  final AgentSessionPort session;
  final String threadDest;
  final AgentProfile profile;
  StreamSubscription<AgentUiEvent>? sub;
}

class ChatAgent {
  ChatAgent(this._ref);

  final Ref _ref;
  final _uuid = const Uuid();
  final _lives = <String, _Live>{};
  final _lru = <String>[];
  final _seenToolCalls = <String>{};
  final _toolTimeouts = <String, Timer>{};
  final _pendingToolCalls = <String, Set<String>>{};

  static const _lruLimit = 4;

  String _key(String dest, String profileId) => '$dest::$profileId';

  /// Direct DM with a local agent contact — every line is a prompt.
  Future<void> sendDirect({required String dest, required String text}) async {
    if (!agentHostSupported) {
      return;
    }
    final body = text.trim();
    if (body.isEmpty) {
      return;
    }
    final profile = await _profileForDest(dest);
    await _appendLocal(dest, body, fromAgent: false, profile: profile);
    await _prompt(dest, body, profile: profile);
  }

  Future<void> onOutgoingText({
    required String dest,
    required String text,
  }) async {
    if (!agentHostSupported) {
      return;
    }
    if (isAgentDest(dest)) {
      await sendDirect(dest: dest, text: text);
      return;
    }
    await _ref.read(agentProfilesProvider.notifier).ensureLoaded();
    final enabled = _ref.read(agentProfilesProvider.notifier).visibleAgents;
    final profile = mentionedProfile(text, enabled);
    if (profile == null) {
      return;
    }
    await _prompt(dest, text, profile: profile);
  }

  Future<void> _prompt(
    String dest,
    String text, {
    required AgentProfile profile,
  }) async {
    await _ref.read(agentSettingsProvider.notifier).ensureLoaded();
    final apiKey = await _ref
        .read(agentProfilesProvider.notifier)
        .readApiKey(profile);
    if (apiKey.trim().isEmpty) {
      await _appendLocal(
        dest,
        '未配置 API Key。打开「我 → Agent 设置」填入 OpenAI 或 Anthropic 密钥。',
        profile: profile,
      );
      return;
    }
    try {
      final live = await _ensureSession(dest, profile, apiKey: apiKey);
      if (!isAgentDest(dest)) {
        final ctx = _contextJson(dest);
        if (ctx.isNotEmpty) {
          await live.session.promptWithContext(text: text, contextJson: ctx);
          return;
        }
      }
      await live.session.prompt(text: text);
    } catch (e) {
      await _appendLocal(dest, 'Goose 调用失败：$e', profile: profile);
    }
  }

  Future<AgentProfile> _profileForDest(String dest) async {
    await _ref.read(agentProfilesProvider.notifier).ensureLoaded();
    final store = _ref.read(agentProfilesProvider.notifier);
    final canon = canonicalAgentDest(dest);
    if (canon == kGooseAgentId) {
      return store.goose ??
          AgentProfile.gooseFromSettings(_ref.read(agentSettingsProvider));
    }
    if (canon.startsWith('agent:')) {
      final id = canon.substring('agent:'.length);
      for (final p in _ref.read(agentProfilesProvider)) {
        if (p.id == id) {
          return p;
        }
      }
    }
    return store.goose ??
        AgentProfile.gooseFromSettings(_ref.read(agentSettingsProvider));
  }

  Future<_Live> _ensureSession(
    String dest,
    AgentProfile profile, {
    required String apiKey,
  }) async {
    final key = _key(dest, profile.id);
    final existing = _lives[key];
    if (existing != null) {
      _touch(key);
      return existing;
    }
    while (_lru.length >= _lruLimit) {
      // WHY: _closeLive also cancels the evicted key's tool-timeout timer.
      await _closeLive(_lru.first);
    }
    final paths = KimPaths.instance;
    await paths.ensureAgentDirs();
    final bridge = _ref.read(agentBridgeProvider);
    await bridge.ensure();
    final opts = _openOpts(dest, profile, apiKey);
    final fileDest = dest.replaceAll('/', '_').replaceAll('\\', '_');
    final sessionFile =
        '${paths.agentSessions.path}/${fileDest}__${profile.id}.json';
    final session = await bridge.open(
      sqlitePath: sessionFile,
      projectRoot: paths.agentWorkspace.path,
      opts: opts,
    );
    final live = _Live(session: session, threadDest: dest, profile: profile);
    live.sub = session.listen().listen((ev) {
      switch (ev.kind) {
        case 'assistant_finished':
          if (ev.message.trim().isNotEmpty) {
            unawaited(_appendLocal(dest, ev.message.trim(), profile: profile));
          }
        case 'failed':
          if (ev.message.trim().isNotEmpty) {
            unawaited(_appendLocal(dest, ev.message.trim(), profile: profile));
          }
        case 'tool_started':
        case 'tool_finished':
          unawaited(_upsertToolCard(dest, ev, profile: profile));
        case 'tool_request':
          unawaited(_onToolRequest(dest, ev, profile: profile));
        case 'action_required':
          unawaited(_appendActionCard(dest, ev, profile: profile));
        default:
          break;
      }
    });
    _lives[key] = live;
    _touch(key);
    return live;
  }

  void _touch(String key) {
    _lru.remove(key);
    _lru.add(key);
  }

  SessionOpenOpts _openOpts(String dest, AgentProfile profile, String apiKey) {
    return SessionOpenOpts(
      model: profile.model,
      llmBackend: profile.providerKind,
      resumeOnOpen: true,
      baseUrl: profile.baseUrl,
      apiKey: apiKey,
      enableFsTools: profile.tools.fs,
      bashEnabled: profile.tools.bash,
      profileId: profile.id,
      profileJson: jsonEncode(profile.toJson()),
      thinkingEffort: profile.thinkingEffort,
      gooseMode: profile.mode,
      enableKimTools: true,
      enableApprovals: true,
      sessionId: '$dest::${profile.id}',
    );
  }

  String _contextJson(String dest) {
    final account = _ref.read(authProvider).account;
    if (account.isEmpty) {
      return '';
    }
    final items = _ref.read(threadMessagesProvider(dest)).items;
    const cap = 8 * 1024;
    final lines = <String>[];
    var used = 0;
    for (final m in items.reversed) {
      if (m.kind != KimMsgKind.text || m.sys) {
        continue;
      }
      final line = '${m.sender}: ${m.body}\n';
      if (used + line.length > cap) {
        break;
      }
      used += line.length;
      lines.add(line);
    }
    return lines.reversed.join();
  }

  Future<void> _onToolRequest(
    String dest,
    AgentUiEvent ev, {
    required AgentProfile profile,
  }) async {
    final callId = ev.callId.trim();
    if (callId.isEmpty || !_seenToolCalls.add(callId)) {
      return;
    }
    _armToolTimeout(dest, profile.id, callId);
    final host = KimCapabilityHost(_ref);
    final out = await host.execute(
      name: ev.name,
      argumentsJson: ev.argumentsJson.isEmpty ? '{}' : ev.argumentsJson,
      sessionDest: dest,
      profileId: profile.id,
      callId: callId,
    );
    // WHY: keyed miss must no-op; another persona on this dest is the wrong live.
    final live = _lives[_key(dest, profile.id)];
    if (live == null) {
      _clearToolCall(dest, profile.id, callId);
      return;
    }
    try {
      await live.session.completeTool(callId: callId, outputJson: out);
      _clearToolCall(dest, profile.id, callId);
    } catch (_) {
      // WHY: a failed complete must not leave this round's timeout armed.
      _clearToolCall(dest, profile.id, callId);
    }
  }

  void _armToolTimeout(String dest, String profileId, String callId) {
    final key = _key(dest, profileId);
    _pendingToolCalls.putIfAbsent(key, () => <String>{}).add(callId);
    _toolTimeouts[key]?.cancel();
    _toolTimeouts[key] = Timer(const Duration(minutes: 10), () {
      final ids = _pendingToolCalls.remove(key) ?? {};
      _toolTimeouts.remove(key);
      final live = _lives[key];
      if (live == null) {
        return;
      }
      for (final id in ids) {
        unawaited(
          live.session.completeTool(
            callId: id,
            outputJson: '{"ok":false,"error":"timeout"}',
          ),
        );
      }
    });
  }

  void _clearToolCall(String dest, String profileId, String callId) {
    final key = _key(dest, profileId);
    final pending = _pendingToolCalls[key];
    pending?.remove(callId);
    if (pending == null || pending.isEmpty) {
      _toolTimeouts.remove(key)?.cancel();
      _pendingToolCalls.remove(key);
    }
  }

  Future<void> _upsertToolCard(
    String dest,
    AgentUiEvent ev, {
    AgentProfile? profile,
  }) async {
    final callId = ev.callId.trim();
    if (callId.isEmpty) {
      return;
    }
    final running = ev.kind == 'tool_started';
    await _upsertCard(
      dest,
      callId: callId,
      name: ev.name,
      type: 'tool',
      state: running ? 'running' : (ev.ok ? 'ok' : 'error'),
      preview: ev.outputPreview,
      ok: !running && ev.ok,
      sender: profile?.displayName ?? kGooseAgentName,
      profileId: profile?.id,
    );
  }

  Future<void> _appendActionCard(
    String dest,
    AgentUiEvent ev, {
    required AgentProfile profile,
  }) async {
    final callId = ev.callId.trim();
    if (callId.isEmpty) {
      return;
    }
    final preview = _previewFor(ev.name, ev.argumentsJson, ev.message);
    await _upsertCard(
      dest,
      callId: callId,
      name: ev.name,
      type: 'action_required',
      state: 'pending',
      preview: preview,
      ok: false,
      sender: profile.displayName,
      profileId: profile.id,
    );
  }

  String _previewFor(String name, String argumentsJson, String prompt) {
    if (prompt.trim().isNotEmpty) {
      return prompt.trim();
    }
    try {
      final raw = jsonDecode(argumentsJson);
      if (raw is Map) {
        if (name == 'send_message') {
          return '${raw['dest'] ?? ''}: ${raw['text'] ?? ''}';
        }
      }
    } catch (_) {}
    return name;
  }

  Future<void> respondPermission({
    required String dest,
    required String callId,
    required String permission,
    required String toolName,
  }) async {
    var profileId = kGooseAgentId;
    final existing = _ref
        .read(threadMessagesProvider(dest))
        .items
        .where((m) => m.key == 'agent-card-$callId')
        .toList();
    if (existing.isNotEmpty) {
      try {
        final prev = jsonDecode(existing.first.body);
        if (prev is Map && prev['profile_id'] != null) {
          profileId = '${prev['profile_id']}';
        }
      } catch (_) {}
    }
    await _upsertCard(
      dest,
      callId: callId,
      name: toolName,
      type: 'action_required',
      state: 'resolved',
      preview: '',
      ok:
          permission != 'deny_once' &&
          permission != 'always_deny' &&
          permission != 'cancel',
      profileId: profileId,
    );
    if (permission == 'always_allow') {
      await _rememberAlwaysAllow(toolName, profileId);
    }
    // WHY: keyed miss must no-op; another persona on this dest is the wrong live.
    final live = _lives[_key(dest, profileId)];
    if (live == null) {
      return;
    }
    try {
      await live.session.respondPermission(
        callId: callId,
        permission: permission,
      );
    } catch (_) {
      await _upsertCard(
        dest,
        callId: callId,
        name: toolName,
        type: 'action_required',
        state: 'pending',
        preview: '',
        ok: false,
        profileId: profileId,
      );
    }
  }

  Future<void> _rememberAlwaysAllow(String toolName, String profileId) async {
    await _ref.read(agentProfilesProvider.notifier).ensureLoaded();
    final store = _ref.read(agentProfilesProvider.notifier);
    if (toolName.trim().isEmpty) {
      return;
    }
    AgentProfile? profile;
    for (final p in _ref.read(agentProfilesProvider)) {
      if (p.id == profileId) {
        profile = p;
        break;
      }
    }
    profile ??= store.goose;
    if (profile == null) {
      return;
    }
    final next = Map<String, String>.from(profile.permissionOverrides);
    next[toolName] = 'always_allow';
    await store.saveProfile(profile.copyWith(permissionOverrides: next));
  }

  Future<void> _upsertCard(
    String dest, {
    required String callId,
    required String name,
    required String type,
    required String state,
    required String preview,
    required bool ok,
    String sender = kGooseAgentName,
    String? profileId,
  }) async {
    final account = _ref.read(authProvider).account;
    if (account.isEmpty) {
      return;
    }
    final key = 'agent-card-$callId';
    final now = DateTime.now().millisecondsSinceEpoch;
    final existing = _ref
        .read(threadMessagesProvider(dest))
        .items
        .where((m) => m.key == key)
        .toList();
    Map<String, Object?> body = {
      'v': 1,
      'type': type,
      'call_id': callId,
      'name': name,
      'state': state,
      'preview': preview,
      'ok': ok,
      'profile_id': ?profileId,
    };
    if (existing.isNotEmpty) {
      try {
        final prev = jsonDecode(existing.first.body);
        if (prev is Map) {
          body = {...Map<String, Object?>.from(prev), ...body};
          if (preview.isEmpty && prev['preview'] != null) {
            body['preview'] = prev['preview'];
          }
        }
      } catch (_) {}
    }
    final msg = existing.isEmpty
        ? KimChatMsg(
            key: key,
            dest: dest,
            sender: sender,
            body: jsonEncode(body),
            at: now,
            kind: KimMsgKind.agentCard,
          )
        : existing.first.copyWith(body: jsonEncode(body));
    await _ref.read(messageRepositoryProvider).applyLive(account, [
      msg,
    ], viewingDest: dest);
    if (!_ref.mounted) {
      return;
    }
    _ref.read(threadMessagesProvider(dest).notifier).receive(msg);
  }

  Future<void> _appendLocal(
    String dest,
    String body, {
    bool fromAgent = true,
    AgentProfile? profile,
  }) async {
    if (body.isEmpty) {
      return;
    }
    final account = _ref.read(authProvider).account;
    if (account.isEmpty) {
      return;
    }
    final now = DateTime.now().millisecondsSinceEpoch;
    final sender = fromAgent
        ? (profile?.displayName ?? kGooseAgentName)
        : account;
    final msg = KimChatMsg(
      key: '${fromAgent ? 'agent' : 'me'}-${_uuid.v4()}',
      dest: dest,
      sender: sender,
      body: body,
      at: now,
    );
    await _ref.read(messageRepositoryProvider).applyLive(account, [
      msg,
    ], viewingDest: dest);
    if (!_ref.mounted) {
      return;
    }
    _ref.read(threadMessagesProvider(dest).notifier).receive(msg);
    _ref.read(threadsProvider.notifier).applyTalk(msg, fromSelf: !fromAgent);
  }

  bool _machineChanged(AgentProfile a, AgentProfile b) {
    return a.model != b.model ||
        a.providerKind != b.providerKind ||
        a.baseUrl != b.baseUrl ||
        a.keyRef != b.keyRef ||
        a.systemPrompt != b.systemPrompt ||
        a.mode != b.mode ||
        a.maxTurns != b.maxTurns ||
        a.thinkingEffort != b.thinkingEffort ||
        a.steer != b.steer ||
        jsonEncode(a.tools.toJson()) != jsonEncode(b.tools.toJson()) ||
        jsonEncode([for (final e in a.extensions) e.toJson()]) !=
            jsonEncode([for (final e in b.extensions) e.toJson()]);
  }

  Future<void> _closeLive(String key) async {
    _lru.remove(key);
    final live = _lives.remove(key);
    _toolTimeouts.remove(key)?.cancel();
    _pendingToolCalls.remove(key);
    await live?.sub?.cancel();
    await live?.session.close();
  }

  Future<void> _onProfilesChanged(List<AgentProfile> next) async {
    final byId = {for (final p in next) p.id: p};
    final stale = [
      for (final e in _lives.entries)
        if (byId[e.value.profile.id] == null ||
            _machineChanged(e.value.profile, byId[e.value.profile.id]!))
          e.key,
    ];
    for (final key in stale) {
      await _closeLive(key);
    }
  }

  Future<void> _closeGooseLives() async {
    final stale = [
      for (final e in _lives.entries)
        if (e.value.profile.id == kGooseAgentId) e.key,
    ];
    for (final key in stale) {
      await _closeLive(key);
    }
  }

  Future<void> _closeLives() async {
    final keys = _lives.keys.toList();
    for (final key in keys) {
      await _closeLive(key);
    }
  }

  Future<void> dispose() async {
    await _closeLives();
  }
}

final chatAgentProvider = Provider<ChatAgent>((ref) {
  final agent = ChatAgent(ref);
  ref.listen(agentProfilesProvider, (prev, next) {
    unawaited(agent._onProfilesChanged(next));
  });
  ref.listen(agentSettingsProvider, (prev, next) {
    unawaited(agent._closeGooseLives());
  });
  ref.onDispose(() {
    unawaited(agent.dispose());
  });
  return agent;
});
