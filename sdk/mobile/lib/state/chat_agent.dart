/// Runs the local Goose host when an IM message @mentions 助手.
library;

import 'dart:async';
import 'dart:convert';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:uuid/uuid.dart';

import '../agent/mention.dart';
import '../agent_bridge.dart';
import '../core/paths.dart';
import '../models/models.dart';
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
    final session = await bridge.open(
      sqlitePath: dest,
      projectRoot: paths.agentWorkspace.path,
      opts: settings.toOpts(resumeOnOpen: true),
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
        default:
          break;
      }
    });
  }

  Future<void> _upsertToolCard(String dest, AgentUiEvent ev) async {
    final callId = ev.callId.trim();
    if (callId.isEmpty) {
      return;
    }
    final account = _ref.read(authProvider).account;
    if (account.isEmpty) {
      return;
    }
    final running = ev.kind == 'tool_started';
    final card = {
      'v': 1,
      'type': 'tool',
      'call_id': callId,
      'name': ev.name,
      'state': running ? 'running' : (ev.ok ? 'ok' : 'error'),
      'preview': ev.outputPreview,
      'ok': !running && ev.ok,
    };
    final key = 'agent-card-$callId';
    final now = DateTime.now().millisecondsSinceEpoch;
    final existing = _ref
        .read(threadMessagesProvider(dest))
        .items
        .where((m) => m.key == key)
        .toList();
    final msg = existing.isEmpty
        ? KimChatMsg(
            key: key,
            dest: dest,
            sender: kGooseAgentName,
            body: jsonEncode(card),
            at: now,
            kind: KimMsgKind.agentCard,
          )
        : existing.first.copyWith(body: jsonEncode(card));
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
