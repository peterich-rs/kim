/// Forwards MobileAgent run requests to desktop `rust_agent`. No queue/LRU here.
library;

import 'dart:async';
import 'dart:convert';

import 'package:kim_mobile/features/agent/agent_permission.dart';
import 'package:kim_mobile/features/agent/agent_presence.dart';
import 'package:kim_mobile/features/agent/agent_profiles.dart';
import 'package:kim_mobile/features/agent/kim_im_tools.dart';
import 'package:kim_mobile/features/agent/host_support.dart';
import 'package:kim_mobile/features/agent/mention.dart';
import 'package:kim_mobile/features/agent/provider_accounts.dart';
import 'package:kim_mobile/features/agent/workspace.dart';
import 'package:kim_mobile/features/agent/workspace_access.dart';
import 'package:kim_mobile/bridge/goose_bridge.dart';
import 'package:kim_mobile/copy.dart';
import 'package:kim_mobile/core/logger.dart';
import 'package:kim_mobile/core/paths.dart';
import 'package:kim_mobile/core/settings.dart';
import 'package:kim_mobile/bridge/kim_bridge.dart';
import 'package:kim_mobile/src/rust/api/types.dart';
import 'package:shared_preferences/shared_preferences.dart';

/// One turn's outcome. Control flow uses [stopReason], not thrown strings.
class DriveResult {
  const DriveResult({
    required this.text,
    required this.stopReason,
    required this.replied,
    required this.visible,
    required this.recentlyActive,
  });

  final String text;
  final String stopReason;
  final bool replied;
  final bool visible;
  final bool recentlyActive;

  static const quiet = <String>{
    'completed',
    'side_effect',
    'empty',
    'idle_timeout',
    'hard_timeout',
    'poisoned',
  };

  factory DriveResult.fromText(String text) {
    final replied = text.trim().isNotEmpty;
    return DriveResult(
      text: text,
      stopReason: replied ? 'completed' : 'empty',
      replied: replied,
      visible: replied,
      recentlyActive: false,
    );
  }

  factory DriveResult.fromEvent(AgentUiEvent ev) {
    final reason = ev.stopReason.isEmpty ? 'completed' : ev.stopReason;
    final replied = reason == 'completed' && (ev.ok || ev.message.trim().isNotEmpty);
    return DriveResult(
      text: ev.message.trim(),
      stopReason: reason,
      replied: replied,
      visible: replied || reason == 'side_effect',
      recentlyActive: ev.recentlyActive,
    );
  }
}

/// Thrown from [AgentRunLoop.driveSession] for typed non-quiet stop reasons.
/// [AgentRunLoop.start] submits [result.stopReason] instead of hard-coding `failed`.
class DriveStop implements Exception {
  const DriveStop(this.result);

  final DriveResult result;

  @override
  String toString() =>
      result.text.trim().isEmpty ? result.stopReason : result.text;
}

class AgentRunLoop {
  AgentRunLoop(
    this.client,
    this.goose, {
    this.sink,
    this.permissions,
    WorkspaceAccess? access,
  }) : access = access ?? workspaceAccess;

  final KimClientPort client;
  final AgentBridge goose;
  final AgentRunSink? sink;
  final AgentPermissionHub? permissions;
  final WorkspaceAccess access;

  /// Tests inject a prompt stub. Production uses [rust_agent] `prompt`.
  Future<String> Function(AgentRunRequestDto req)? promptOverride;

  StreamSubscription<AgentRunRequestDto>? _sub;
  StreamController<AgentRunRequestDto>? _incoming;
  var _stopped = false;

  Future<void> start() async {
    if (!agentHostSupported && promptOverride == null) {
      return;
    }
    final incoming = StreamController<AgentRunRequestDto>();
    _incoming = incoming;
    _sub = client.watchAgentRun().listen(
      (req) {
        if (_stopped || incoming.isClosed) {
          return;
        }
        incoming.add(req);
      },
      onError: (Object error, StackTrace st) {
        if (!_stopped && !incoming.isClosed) {
          incoming.addError(error, st);
        }
      },
      onDone: () {
        if (!incoming.isClosed) {
          incoming.close();
        }
      },
    );
    try {
      await for (final req in incoming.stream) {
        if (_stopped) {
          break;
        }
        try {
          sink?.begin(req.dest);
          final result = promptOverride != null
              ? DriveResult.fromText(await promptOverride!(req))
              : await _promptGoose(req);
          sink?.finish(req.dest, failed: false);
          await client.submitAgentRun(
            AgentRunResultDto(
              dest: req.dest,
              profileId: req.profileId,
              epoch: req.epoch,
              output: result.text,
              stopReason: result.stopReason,
              replied: result.replied,
              visible: result.visible,
              recentlyActive: result.recentlyActive,
            ),
          );
        } catch (e, st) {
          KimLogger.warn('agent run', e, st);
          sink?.finish(req.dest, failed: true);
          final typed = e is DriveStop ? e.result : null;
          await client.submitAgentRun(
            AgentRunResultDto(
              dest: req.dest,
              profileId: req.profileId,
              epoch: req.epoch,
              output: typed?.text ?? '',
              error: _runErrorText(e),
              stopReason: typed?.stopReason ?? 'failed',
              replied: typed?.replied ?? false,
              visible: typed?.visible ?? false,
              recentlyActive: typed?.recentlyActive ?? false,
            ),
          );
        }
      }
    } finally {
      await _sub?.cancel();
      _sub = null;
      if (!incoming.isClosed) {
        await incoming.close();
      }
      if (identical(_incoming, incoming)) {
        _incoming = null;
      }
    }
  }

  Future<void> stop() async {
    _stopped = true;
    await _sub?.cancel();
    _sub = null;
    final incoming = _incoming;
    _incoming = null;
    if (incoming != null && !incoming.isClosed) {
      await incoming.close();
    }
  }

  Future<DriveResult> _promptGoose(AgentRunRequestDto req) async {
    await goose.ensure();
    if (!goose.isReady) {
      throw StateError(Copy.agentHostNotReady);
    }
    final rows = await client.listAgentProfiles();
    AgentProfileDto? row;
    for (final r in rows) {
      if (r.profileId == req.profileId) {
        row = r;
        break;
      }
    }
    if (row == null) {
      throw StateError('agent profile ${req.profileId} not found');
    }
    var bodyJson = row.bodyJson;
    if (row.bodyBlob.isNotEmpty) {
      bodyJson = await client.specBlobToJson(row.bodyBlob);
    }
    if (bodyJson.trim().isEmpty) {
      throw StateError('agent profile ${req.profileId} has empty spec');
    }
    final decoded = jsonDecode(bodyJson);
    if (decoded is! Map) {
      throw StateError('agent profile ${req.profileId} spec is not an object');
    }
    var profile = AgentProfile.fromJson(Map<String, Object?>.from(decoded))
        .copyWith(serverAccount: row.serverAccount);
    final overlay = await client.getDeviceOverlay(req.profileId);
    if (overlay != null && overlay.workspacePath.isNotEmpty) {
      profile = profile.copyWith(
        workspace: profile.workspace.copyWith(
          path: overlay.workspacePath,
          bookmarkRef: overlay.workspaceBookmark,
        ),
      );
    }
    final skillPaths = await skillHostPaths(
      access: access,
      overlay: overlay?.userAgentsSkills ?? '',
    );
    final accounts = await client.listProviderAccounts();
    ProviderAccountDto? accountRow;
    for (final a in accounts) {
      if (a.id == profile.accountId) {
        accountRow = a;
        break;
      }
    }
    if (profile.accountId.isEmpty || accountRow == null) {
      throw MissingProviderAccount(profile.accountId);
    }
    final account = ProviderAccount(
      id: accountRow.id,
      vendorId: accountRow.vendorId,
      baseUrl: accountRow.baseUrl,
      keyRef: accountRow.keyRef,
      displayName: accountRow.displayName,
    );
    final apiKey = await _readApiKey(account, profile);
    if (apiKey.trim().isEmpty) {
      throw StateError('api key missing for ${profile.id}');
    }
    final paths = KimPaths.instance;
    await paths.ensureAgentDirs();
    final ws = await resolveAgentProjectRoot(profile: profile, paths: paths);
    final sessionFile = paths.agentSessionFile(
      dest: req.dest,
      profileId: profile.id,
    );
    final prefs = await SharedPreferences.getInstance();
    final harnessOn = prefs.getBool('agent.harness_v1') ?? false;
    final session = await goose.open(
      sqlitePath: sessionFile.path,
      projectRoot: ws.path,
      opts: SessionOpenOpts(
        model: profile.model,
        llmBackend: account.vendorId,
        resumeOnOpen: true,
        baseUrl: account.baseUrl,
        apiKey: apiKey,
        enableFsTools: profile.tools.fs,
        bashEnabled: profile.tools.bash,
        profileId: profile.id,
        profileJson: jsonEncode(
          profile.toHostJson(
            account,
            userAgentsSkills: skillPaths.userAgentsSkills,
          ),
        ),
        thinkingEffort: profile.thinkingEffort,
        gooseMode: profile.mode,
        enableKimTools: false,
        enableApprovals: false,
        sessionId: '${req.dest}:${req.profileId}',
        harnessJson: harnessOn ? '{"enabled":true}' : '{"enabled":false}',
      ),
    );
    return driveSession(
      session,
      dest: req.dest,
      text: req.text,
    );
  }

  /// Visible for tests. Host yields deferred IM tools to Dart; ignoring
  /// `tool_request` leaves the Goose turn running forever.
  Future<DriveResult> driveSession(
    AgentSessionPort session, {
    required String dest,
    required String text,
    KimImTools? tools,
  }) async {
    final im = tools ?? KimImTools(client);
    permissions?.attach(dest, session);
    final inbox = StreamController<AgentUiEvent>();
    final sub = session.listen().listen(
      inbox.add,
      onError: inbox.addError,
      onDone: inbox.close,
    );
    try {
      final pending = await session.resume();
      var prompted = pending.resumedOps.isEmpty;
      if (prompted) {
        await session.prompt(text: text);
      }
      await for (final ev in inbox.stream) {
        if (ev.kind == 'action_required') {
          permissions?.prompt(
            dest,
            AgentPermissionPrompt(
              callId: ev.callId,
              name: ev.name,
              preview: ev.message.isNotEmpty ? ev.message : ev.outputPreview,
            ),
          );
          continue;
        }
        if (ev.kind == 'tool_request') {
          final output = await im.execute(
            name: ev.name,
            argumentsJson: ev.argumentsJson,
            currentDest: dest,
          );
          await session.completeTool(callId: ev.callId, outputJson: output);
          continue;
        }
        final quiet =
            DriveResult.quiet.contains(ev.stopReason) ||
            (ev.kind == 'assistant_finished' && ev.stopReason.isEmpty);
        if (quiet) {
          if (!prompted) {
            prompted = true;
            await session.prompt(text: text);
            continue;
          }
          return DriveResult.fromEvent(ev);
        }
        if (ev.kind == 'failed' || ev.kind == 'aborted') {
          if (!prompted && ev.stopReason == 'yield_abandoned') {
            prompted = true;
            await session.prompt(text: text);
            continue;
          }
          throw DriveStop(DriveResult.fromEvent(ev));
        }
      }
      throw StateError(Copy.agentRunFailed);
    } finally {
      await sub.cancel();
      if (!inbox.isClosed) {
        await inbox.close();
      }
      permissions?.detach(dest);
      try {
        await session.close();
      } catch (e, st) {
        KimLogger.warn('agent session close', e, st);
      }
    }
  }

  Future<String> _readApiKey(
    ProviderAccount account,
    AgentProfile profile,
  ) async {
    final secure = SettingsStore.productionSecureStorage();
    Future<String> read(String key) async {
      try {
        return await secure.read(key: key) ?? '';
      } catch (_) {
        return '';
      }
    }

    final keyed = await read(account.keyRef);
    if (keyed.isNotEmpty) {
      return keyed;
    }
    if (account.keyRef == 'agent.api_key.goose' ||
        profile.id == kGooseAgentId) {
      final goose = await read('agent.api_key.goose');
      if (goose.isNotEmpty) {
        return goose;
      }
      return await read('agent.api_key');
    }
    return '';
  }
}

String _runErrorText(Object error) {
  return switch (error) {
    DriveStop() => error.toString(),
    StateError(:final message) => message,
    _ => error.toString(),
  };
}
