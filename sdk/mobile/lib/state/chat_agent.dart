/// Runs the local Goose host when an IM message @mentions 助手.
library;

import 'dart:async';
import 'dart:convert';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:uuid/uuid.dart';

import '../agent/capability_host.dart';
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
  Timer? _toolTimeout;

  static const _lruLimit = 4;

  String _key(String dest, String profileId) => '$dest::$profileId';

  _Live? _liveForDest(String dest) {
    for (final key in _lru.reversed) {
      final live = _lives[key];
      if (live?.threadDest == dest) {
        return live;
      }
    }
    return null;
  }

  /// Direct DM with a local agent contact — every line is a prompt.
  Future<void> sendDirect({required String dest, required String text}) async {
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
    final settings = _ref.read(agentSettingsProvider);
    if (settings.apiKey.trim().isEmpty) {
      await _appendLocal(
        dest,
        '未配置 API Key。打开「我 → Agent 设置」填入 OpenAI 或 Anthropic 密钥。',
        profile: profile,
      );
      return;
    }
    try {
      final live = await _ensureSession(dest, settings, profile);
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
      return store.goose ?? AgentProfile.gooseFromSettings(_ref.read(agentSettingsProvider));
    }
    if (canon.startsWith('agent:')) {
      final id = canon.substring('agent:'.length);
      for (final p in _ref.read(agentProfilesProvider)) {
        if (p.id == id) {
          return p;
        }
      }
    }
    return store.goose ?? AgentProfile.gooseFromSettings(_ref.read(agentSettingsProvider));
  }

  Future<_Live> _ensureSession(
    String dest,
    AgentSettings settings,
    AgentProfile profile,
  ) async {
    final key = _key(dest, profile.id);
    final existing = _lives[key];
    if (existing != null) {
      _touch(key);
      return existing;
    }
    while (_lru.length >= _lruLimit) {
      final evict = _lru.removeAt(0);
      final old = _lives.remove(evict);
      await old?.sub?.cancel();
      await old?.session.close();
    }
    final paths = KimPaths.instance;
    await paths.ensureAgentDirs();
    final bridge = _ref.read(agentBridgeProvider);
    await bridge.ensure();
    final opts = _openOpts(settings, dest, profile);
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
            unawaited(
              _appendLocal(dest, ev.message.trim(), profile: profile),
            );
          }
        case 'failed':
          if (ev.message.trim().isNotEmpty) {
            unawaited(
              _appendLocal(dest, ev.message.trim(), profile: profile),
            );
          }
        case 'tool_started':
        case 'tool_finished':
          unawaited(_upsertToolCard(dest, ev, profile: profile));
        case 'tool_request':
          unawaited(_onToolRequest(dest, ev, profile: profile));
        case 'action_required':
          unawaited(_appendActionCard(dest, ev));
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

  SessionOpenOpts _openOpts(
    AgentSettings settings,
    String dest,
    AgentProfile profile,
  ) {
    return SessionOpenOpts(
      model: profile.model,
      llmBackend: profile.providerKind,
      resumeOnOpen: true,
      baseUrl: profile.baseUrl,
      apiKey: settings.apiKey,
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
    final buf = StringBuffer();
    for (final m in items.reversed) {
      if (m.kind != KimMsgKind.text || m.sys) {
        continue;
      }
      final line = '${m.sender}: ${m.body}\n';
      if (buf.length + line.length > cap) {
        break;
      }
      buf.write(line);
    }
    return buf.toString();
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
    _armToolTimeout(dest, callId);
    final host = KimCapabilityHost(_ref);
    final out = await host.execute(
      name: ev.name,
      argumentsJson: ev.argumentsJson.isEmpty ? '{}' : ev.argumentsJson,
      sessionDest: dest,
      profileId: profile.id,
      callId: callId,
    );
    final live = _lives[_key(dest, profile.id)] ?? _liveForDest(dest);
    if (live == null) {
      return;
    }
    try {
      await live.session.completeTool(callId: callId, outputJson: out);
    } catch (_) {}
  }

  void _armToolTimeout(String dest, String callId) {
    _toolTimeout?.cancel();
    _toolTimeout = Timer(const Duration(minutes: 10), () {
      final live = _liveForDest(dest);
      if (live == null) {
        return;
      }
      unawaited(
        live.session.completeTool(
          callId: callId,
          outputJson: '{"ok":false,"error":"timeout"}',
        ),
      );
    });
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
    );
  }

  Future<void> _appendActionCard(String dest, AgentUiEvent ev) async {
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
      sender: kGooseAgentName,
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
    await _upsertCard(
      dest,
      callId: callId,
      name: toolName,
      type: 'action_required',
      state: 'resolved',
      preview: '',
      ok: permission != 'deny_once' && permission != 'always_deny' && permission != 'cancel',
    );
    if (permission == 'always_allow') {
      await _rememberAlwaysAllow(toolName);
    }
    final live = _liveForDest(dest);
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
      );
    }
  }

  Future<void> _rememberAlwaysAllow(String toolName) async {
    await _ref.read(agentProfilesProvider.notifier).ensureLoaded();
    await _ref.read(agentSettingsProvider.notifier).ensureLoaded();
    final store = _ref.read(agentProfilesProvider.notifier);
    final goose = store.goose;
    if (goose == null || toolName.trim().isEmpty) {
      return;
    }
    final next = Map<String, String>.from(goose.permissionOverrides);
    next[toolName] = 'always_allow';
    await store.saveGoose(
      goose.copyWith(permissionOverrides: next),
      apiKey: _ref.read(agentSettingsProvider).apiKey,
    );
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

  Future<void> dispose() async {
    _toolTimeout?.cancel();
    for (final live in _lives.values) {
      await live.sub?.cancel();
      await live.session.close();
    }
    _lives.clear();
    _lru.clear();
  }
}

final chatAgentProvider = Provider<ChatAgent>((ref) {
  final agent = ChatAgent(ref);
  ref.onDispose(() {
    unawaited(agent.dispose());
  });
  return agent;
});
