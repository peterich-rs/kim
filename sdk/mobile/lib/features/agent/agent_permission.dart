library;

import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:kim_mobile/bridge/goose_bridge.dart';

const kPermissionAskBefore = 'ask_before';

const kAskBeforeSendMessage = 'send_message';
const kAskBeforeReadClipboard = 'read_clipboard';
const kAskBeforeWriteFile = 'write_file';
const kAskBeforeBash = 'bash';
const kAskBeforeDelegate = 'delegate';

const kAskBeforeTools = <String>[
  kAskBeforeSendMessage,
  kAskBeforeReadClipboard,
  kAskBeforeWriteFile,
  kAskBeforeBash,
  kAskBeforeDelegate,
];

bool permissionAsksBefore(Map<String, String> overrides, String tool) {
  return overrides[tool] == kPermissionAskBefore;
}

Map<String, String> setPermissionAskBefore(
  Map<String, String> overrides,
  String tool, {
  required bool ask,
}) {
  if (tool.isEmpty) {
    return Map<String, String>.from(overrides);
  }
  final next = Map<String, String>.from(overrides);
  if (ask) {
    next[tool] = kPermissionAskBefore;
  } else {
    next.remove(tool);
  }
  return next;
}

class AgentPermissionPrompt {
  const AgentPermissionPrompt({
    required this.callId,
    required this.name,
    required this.preview,
  });

  final String callId;
  final String name;
  final String preview;
}

class AgentPermissionState {
  const AgentPermissionState({this.byDest = const {}});

  final Map<String, List<AgentPermissionPrompt>> byDest;

  List<AgentPermissionPrompt> of(String dest) =>
      byDest[dest] ?? const <AgentPermissionPrompt>[];
}

class AgentPermissionHub extends Notifier<AgentPermissionState> {
  final _sessions = <String, AgentSessionPort>{};

  @override
  AgentPermissionState build() => const AgentPermissionState();

  void attach(String dest, AgentSessionPort session) {
    if (dest.isEmpty) {
      return;
    }
    _sessions[dest] = session;
  }

  void detach(String dest) {
    _sessions.remove(dest);
    if (!state.byDest.containsKey(dest)) {
      return;
    }
    final next = Map<String, List<AgentPermissionPrompt>>.from(state.byDest)
      ..remove(dest);
    state = AgentPermissionState(byDest: next);
  }

  void prompt(String dest, AgentPermissionPrompt request) {
    if (dest.isEmpty || request.callId.isEmpty) {
      return;
    }
    final current = List<AgentPermissionPrompt>.from(state.of(dest));
    if (current.any((p) => p.callId == request.callId)) {
      return;
    }
    current.add(request);
    final next = Map<String, List<AgentPermissionPrompt>>.from(state.byDest);
    next[dest] = current;
    state = AgentPermissionState(byDest: next);
  }

  Future<void> respond({
    required String dest,
    required String callId,
    required String permission,
  }) async {
    final session = _sessions[dest];
    if (session == null || callId.isEmpty) {
      return;
    }
    final remaining = [
      for (final p in state.of(dest))
        if (p.callId != callId) p,
    ];
    final next = Map<String, List<AgentPermissionPrompt>>.from(state.byDest);
    if (remaining.isEmpty) {
      next.remove(dest);
    } else {
      next[dest] = remaining;
    }
    state = AgentPermissionState(byDest: next);
    await session.respondPermission(callId: callId, permission: permission);
  }
}

final agentPermissionHubProvider =
    NotifierProvider<AgentPermissionHub, AgentPermissionState>(
      AgentPermissionHub.new,
    );
