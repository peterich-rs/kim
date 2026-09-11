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

class ChatAgent {
  ChatAgent(this._ref);

  final Ref _ref;
  final _uuid = const Uuid();
  AgentSessionPort? _session;
  String? _sessionDest;
  StreamSubscription<AgentUiEvent>? _sub;
  final _seenToolCalls = <String>{};
  Timer? _toolTimeout;

  /// Direct DM with the local Goose contact — every line is a prompt.
  Future<void> sendDirect({required String dest, required String text}) async {
    final body = text.trim();
    if (body.isEmpty) {
      return;
    }
    await _appendLocal(dest, body, fromAgent: false);
    await _prompt(dest, body);
  }

  Future<void> onOutgoingText({
    required String dest,
    required String text,
  }) async {
    if (isGooseAgentDest(dest)) {
      await sendDirect(dest: dest, text: text);
      return;
    }
    if (!mentionsGooseAgent(text)) {
      return;
    }
    await _prompt(dest, text);
  }

  Future<void> _prompt(String dest, String text) async {
    await _ref.read(agentSettingsProvider.notifier).ensureLoaded();
    final settings = _ref.read(agentSettingsProvider);
    if (settings.apiKey.trim().isEmpty) {
      await _appendLocal(
        dest,
        '未配置 API Key。打开「我 → Agent 设置」填入 OpenAI 或 Anthropic 密钥。',
      );
      return;
    }
    try {
      await _ensureSession(dest, settings);
      final session = _session;
      if (session == null) {
        return;
      }
      if (!isGooseAgentDest(dest)) {
        final ctx = _contextJson(dest);
        if (ctx.isNotEmpty) {
          await session.promptWithContext(text: text, contextJson: ctx);
          return;
        }
      }
      await session.prompt(text: text);
    } catch (e) {
      await _appendLocal(dest, 'Goose 调用失败：$e');
    }
  }

  Future<void> _ensureSession(String dest, AgentSettings settings) async {
    if (_sessionDest == dest && _session != null) {
      return;
    }
    await _sub?.cancel();
    await _session?.close();
    _session = null;
    final paths = KimPaths.instance;
    await paths.ensureAgentDirs();
    final bridge = _ref.read(agentBridgeProvider);
    await bridge.ensure();
    final opts = await _openOpts(settings, dest);
    final fileDest = dest.replaceAll('/', '_').replaceAll('\\', '_');
    final sessionFile =
        '${paths.agentSessions.path}/${fileDest}__${opts.profileId.isEmpty ? 'goose' : opts.profileId}.json';
    final session = await bridge.open(
      sqlitePath: sessionFile,
      projectRoot: paths.agentWorkspace.path,
      opts: opts,
    );
    _session = session;
    _sessionDest = dest;
    _sub = session.listen().listen((ev) {
      switch (ev.kind) {
        case 'assistant_finished':
          if (ev.message.trim().isNotEmpty) {
            unawaited(_appendLocal(dest, ev.message.trim()));
          }
        case 'failed':
          if (ev.message.trim().isNotEmpty) {
            unawaited(_appendLocal(dest, ev.message.trim()));
          }
        case 'tool_started':
        case 'tool_finished':
          unawaited(_upsertToolCard(dest, ev));
        case 'tool_request':
          unawaited(_onToolRequest(dest, ev));
        case 'action_required':
          unawaited(_appendActionCard(dest, ev));
        default:
          break;
      }
    });
  }

  Future<SessionOpenOpts> _openOpts(AgentSettings settings, String dest) async {
    await _ref.read(agentProfilesProvider.notifier).ensureLoaded();
    final goose = _ref.read(agentProfilesProvider.notifier).goose;
    final base = settings.toOpts(resumeOnOpen: true);
    if (goose == null) {
      return SessionOpenOpts(
        model: base.model,
        llmBackend: base.llmBackend,
        resumeOnOpen: base.resumeOnOpen,
        baseUrl: base.baseUrl,
        apiKey: base.apiKey,
        enableFsTools: base.enableFsTools,
        bashEnabled: base.bashEnabled,
        profileId: base.profileId,
        profileJson: '',
        thinkingEffort: base.thinkingEffort,
        gooseMode: base.gooseMode,
        enableKimTools: true,
        enableApprovals: true,
        sessionId: '$dest::goose',
      );
    }
    return SessionOpenOpts(
      model: goose.model,
      llmBackend: goose.providerKind,
      resumeOnOpen: true,
      baseUrl: goose.baseUrl,
      apiKey: settings.apiKey,
      enableFsTools: goose.tools.fs,
      bashEnabled: goose.tools.bash,
      profileId: goose.id,
      profileJson: jsonEncode(goose.toJson()),
      thinkingEffort: goose.thinkingEffort,
      gooseMode: goose.mode,
      enableKimTools: true,
      enableApprovals: true,
      sessionId: '$dest::${goose.id}',
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

  Future<void> _onToolRequest(String dest, AgentUiEvent ev) async {
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
      profileId: 'goose',
      callId: callId,
    );
    final session = _session;
    if (session == null) {
      return;
    }
    try {
      await session.completeTool(callId: callId, outputJson: out);
    } catch (_) {}
  }

  void _armToolTimeout(String dest, String callId) {
    _toolTimeout?.cancel();
    _toolTimeout = Timer(const Duration(minutes: 10), () {
      final session = _session;
      if (session == null) {
        return;
      }
      unawaited(
        session.completeTool(
          callId: callId,
          outputJson: '{"ok":false,"error":"timeout"}',
        ),
      );
    });
  }

  Future<void> _upsertToolCard(String dest, AgentUiEvent ev) async {
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
    final session = _session;
    if (session == null) {
      return;
    }
    try {
      await session.respondPermission(callId: callId, permission: permission);
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
            sender: kGooseAgentName,
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
  }) async {
    if (body.isEmpty) {
      return;
    }
    final account = _ref.read(authProvider).account;
    if (account.isEmpty) {
      return;
    }
    final now = DateTime.now().millisecondsSinceEpoch;
    final sender = fromAgent ? kGooseAgentName : account;
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
    await _sub?.cancel();
    _sub = null;
    await _session?.close();
    _session = null;
  }
}

final chatAgentProvider = Provider<ChatAgent>((ref) {
  final agent = ChatAgent(ref);
  ref.onDispose(() {
    unawaited(agent.dispose());
  });
  return agent;
});
