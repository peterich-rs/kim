library;

import 'dart:async';
import 'dart:io';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:uuid/uuid.dart';

import '../agent_bridge.dart';
import '../core/paths.dart';
import 'agent_settings.dart';

class AgentToolCard {
  AgentToolCard({
    required this.callId,
    required this.name,
    this.args = '',
    this.result = '',
    this.status = 'running',
  });

  final String callId;
  final String name;
  String args;
  String result;
  String status; // running | ok | fail
}

class AgentTranscriptItem {
  AgentTranscriptItem({
    required this.id,
    required this.role, // user | assistant
    this.text = '',
    this.tools = const [],
    this.streaming = false,
  });

  final String id;
  final String role;
  String text;
  List<AgentToolCard> tools;
  bool streaming;
}

class AgentSessionMeta {
  AgentSessionMeta({
    required this.id,
    required this.title,
    required this.sqlitePath,
    required this.updatedAt,
  });

  final String id;
  String title;
  final String sqlitePath;
  DateTime updatedAt;
}

class AgentState {
  const AgentState({
    this.sessions = const [],
    this.activeId,
    this.items = const [],
    this.busy = false,
    this.error,
  });

  final List<AgentSessionMeta> sessions;
  final String? activeId;
  final List<AgentTranscriptItem> items;
  final bool busy;
  final String? error;

  AgentState copyWith({
    List<AgentSessionMeta>? sessions,
    String? activeId,
    bool clearActive = false,
    List<AgentTranscriptItem>? items,
    bool? busy,
    String? error,
    bool clearError = false,
  }) {
    return AgentState(
      sessions: sessions ?? this.sessions,
      activeId: clearActive ? null : (activeId ?? this.activeId),
      items: items ?? this.items,
      busy: busy ?? this.busy,
      error: clearError ? null : (error ?? this.error),
    );
  }
}

final agentBridgeProvider = Provider<AgentBridge>((ref) => AgentBridge());

class AgentNotifier extends Notifier<AgentState> {
  AgentSession? _session;
  StreamSubscription<AgentUiEvent>? _sub;
  final _uuid = const Uuid();

  @override
  AgentState build() {
    ref.onDispose(() {
      unawaited(_tearDown());
    });
    Future.microtask(_bootstrap);
    return const AgentState();
  }

  Future<void> _bootstrap() async {
    final paths = KimPaths.instance;
    await paths.ensureAgentDirs();
    final sessionsDir = paths.agentSessions;
    final metas = <AgentSessionMeta>[];
    if (await sessionsDir.exists()) {
      await for (final ent in sessionsDir.list()) {
        if (ent is File && ent.path.endsWith('.sqlite')) {
          final id = ent.uri.pathSegments.last.replaceAll('.sqlite', '');
          metas.add(
            AgentSessionMeta(
              id: id,
              title: 'Session ${id.length > 8 ? id.substring(0, 8) : id}',
              sqlitePath: ent.path,
              updatedAt: await ent.lastModified(),
            ),
          );
        }
      }
    }
    metas.sort((a, b) => b.updatedAt.compareTo(a.updatedAt));
    // Do not auto-open FFI: IndexedStack builds Agent even when hidden;
    // widget tests have no kim_agent_ffi native assets.
    state = state.copyWith(sessions: metas);
    if (metas.isNotEmpty) {
      state = state.copyWith(activeId: metas.first.id);
    }
  }

  Future<void> _tearDown() async {
    await _sub?.cancel();
    _sub = null;
    final s = _session;
    _session = null;
    if (s != null) {
      try {
        await s.close();
      } catch (_) {}
    }
  }

  Future<void> newSession() async {
    final paths = KimPaths.instance;
    await paths.ensureAgentDirs();
    final id = _uuid.v4();
    final sqlite = '${paths.agentSessions.path}/$id.sqlite';
    final meta = AgentSessionMeta(
      id: id,
      title: '新会话',
      sqlitePath: sqlite,
      updatedAt: DateTime.now(),
    );
    state = state.copyWith(
      sessions: [meta, ...state.sessions],
      activeId: id,
      items: const [],
      clearError: true,
    );
    await _openPath(sqlite, resume: false);
  }

  Future<void> openSession(String id) async {
    AgentSessionMeta? meta;
    for (final s in state.sessions) {
      if (s.id == id) {
        meta = s;
        break;
      }
    }
    if (meta == null) {
      return;
    }
    state = state.copyWith(activeId: id, items: const [], clearError: true);
    await _openPath(meta.sqlitePath, resume: true);
  }

  Future<void> _openPath(String sqlite, {required bool resume}) async {
    await _tearDown();
    final settings = ref.read(agentSettingsProvider);
    if (settings.isLive && settings.apiKey.trim().isEmpty) {
      state = state.copyWith(error: 'Live 模式需要 API Key');
      return;
    }
    final bridge = ref.read(agentBridgeProvider);
    final paths = KimPaths.instance;
    try {
      await bridge.ensure();
      final session = await bridge.open(
        sqlitePath: sqlite,
        projectRoot: paths.agentWorkspace.path,
        opts: settings.toOpts(resumeOnOpen: resume),
      );
      _session = session;
      _sub = session.listen().listen(
        _onEvent,
        onError: (Object e) {
          state = state.copyWith(error: '$e', busy: false);
        },
      );
    } catch (e) {
      state = state.copyWith(error: 'Agent FFI unavailable: $e', busy: false);
    }
  }

  void _onEvent(AgentUiEvent ev) {
    switch (ev.kind) {
      case 'operation_started':
        state = state.copyWith(busy: true, clearError: true);
      case 'assistant_text_delta':
        _appendAssistantDelta(ev.delta);
      case 'tool_call_started':
        _upsertTool(
          AgentToolCard(callId: ev.callId, name: ev.name, status: 'running'),
        );
      case 'tool_call_args_delta':
        _appendToolArgs(ev.callId, ev.delta);
      case 'tool_call_finished':
        _upsertTool(
          AgentToolCard(
            callId: ev.callId,
            name: ev.name,
            args: ev.argumentsJson,
            status: 'running',
          ),
        );
      case 'tool_result':
        _finishTool(ev.callId, ev.ok, ev.outputPreview);
      case 'operation_completed':
      case 'operation_aborted':
      case 'operation_failed':
        _finalizeAssistant();
        state = state.copyWith(
          busy: false,
          error: ev.kind == 'operation_failed' ? ev.message : null,
        );
      default:
        break;
    }
  }

  void _appendAssistantDelta(String delta) {
    final items = [...state.items];
    if (items.isEmpty ||
        items.last.role != 'assistant' ||
        !items.last.streaming) {
      items.add(
        AgentTranscriptItem(
          id: _uuid.v4(),
          role: 'assistant',
          text: delta,
          streaming: true,
          tools: [],
        ),
      );
    } else {
      final last = items.last;
      items[items.length - 1] = AgentTranscriptItem(
        id: last.id,
        role: last.role,
        text: last.text + delta,
        streaming: true,
        tools: last.tools,
      );
    }
    state = state.copyWith(items: items);
  }

  void _upsertTool(AgentToolCard card) {
    final items = [...state.items];
    if (items.isEmpty || items.last.role != 'assistant') {
      items.add(
        AgentTranscriptItem(
          id: _uuid.v4(),
          role: 'assistant',
          streaming: true,
          tools: [card],
        ),
      );
    } else {
      final last = items.last;
      final tools = [...last.tools];
      final idx = tools.indexWhere((t) => t.callId == card.callId);
      if (idx >= 0) {
        final prev = tools[idx];
        tools[idx] = AgentToolCard(
          callId: card.callId,
          name: card.name.isEmpty ? prev.name : card.name,
          args: card.args.isEmpty ? prev.args : card.args,
          result: card.result.isEmpty ? prev.result : card.result,
          status: card.status,
        );
      } else {
        tools.add(card);
      }
      items[items.length - 1] = AgentTranscriptItem(
        id: last.id,
        role: last.role,
        text: last.text,
        streaming: true,
        tools: tools,
      );
    }
    state = state.copyWith(items: items);
  }

  void _appendToolArgs(String callId, String delta) {
    final items = [...state.items];
    if (items.isEmpty) {
      return;
    }
    final last = items.last;
    final tools = [...last.tools];
    final idx = tools.indexWhere((t) => t.callId == callId);
    if (idx < 0) {
      return;
    }
    final t = tools[idx];
    tools[idx] = AgentToolCard(
      callId: t.callId,
      name: t.name,
      args: t.args + delta,
      result: t.result,
      status: t.status,
    );
    items[items.length - 1] = AgentTranscriptItem(
      id: last.id,
      role: last.role,
      text: last.text,
      streaming: last.streaming,
      tools: tools,
    );
    state = state.copyWith(items: items);
  }

  void _finishTool(String callId, bool ok, String preview) {
    final items = [...state.items];
    if (items.isEmpty) {
      return;
    }
    final last = items.last;
    final tools = [...last.tools];
    final idx = tools.indexWhere((t) => t.callId == callId);
    if (idx < 0) {
      return;
    }
    final t = tools[idx];
    tools[idx] = AgentToolCard(
      callId: t.callId,
      name: t.name,
      args: t.args,
      result: preview,
      status: ok ? 'ok' : 'fail',
    );
    items[items.length - 1] = AgentTranscriptItem(
      id: last.id,
      role: last.role,
      text: last.text,
      streaming: last.streaming,
      tools: tools,
    );
    state = state.copyWith(items: items);
  }

  void _finalizeAssistant() {
    final items = [...state.items];
    if (items.isNotEmpty && items.last.role == 'assistant') {
      final last = items.last;
      items[items.length - 1] = AgentTranscriptItem(
        id: last.id,
        role: last.role,
        text: last.text,
        streaming: false,
        tools: last.tools,
      );
      state = state.copyWith(items: items);
    }
  }

  Future<void> send(String text) async {
    final trimmed = text.trim();
    if (trimmed.isEmpty || state.busy) {
      return;
    }
    if (_session == null) {
      if (state.activeId == null) {
        await newSession();
      } else {
        await openSession(state.activeId!);
      }
    }
    final session = _session;
    if (session == null) {
      return;
    }
    final items = [
      ...state.items,
      AgentTranscriptItem(id: _uuid.v4(), role: 'user', text: trimmed),
    ];
    state = state.copyWith(items: items, busy: true, clearError: true);
    // Update title from first user message
    final active = state.activeId;
    if (active != null) {
      final sessions = [
        for (final s in state.sessions)
          if (s.id == active)
            AgentSessionMeta(
              id: s.id,
              title: trimmed.length > 24
                  ? '${trimmed.substring(0, 24)}…'
                  : trimmed,
              sqlitePath: s.sqlitePath,
              updatedAt: DateTime.now(),
            )
          else
            s,
      ];
      state = state.copyWith(sessions: sessions);
    }
    try {
      await session.prompt(text: trimmed);
    } catch (e) {
      state = state.copyWith(busy: false, error: '$e');
    }
  }

  Future<void> abort() async {
    try {
      await _session?.abort();
    } catch (e) {
      state = state.copyWith(error: '$e', busy: false);
    }
  }

  /// Apply provider settings without wiping SQLite tree.
  Future<void> applySettings() async {
    final settings = ref.read(agentSettingsProvider);
    final session = _session;
    if (session == null) {
      return;
    }
    if (settings.isLive && settings.apiKey.trim().isEmpty) {
      state = state.copyWith(error: 'Live 模式需要 API Key');
      return;
    }
    try {
      await session.reconfigure(opts: settings.toOpts(resumeOnOpen: false));
      state = state.copyWith(clearError: true);
    } catch (e) {
      state = state.copyWith(error: '$e');
    }
  }
}

final agentProvider = NotifierProvider<AgentNotifier, AgentState>(
  AgentNotifier.new,
);
