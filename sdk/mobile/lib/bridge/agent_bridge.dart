/// Forwards MobileAgent run requests to desktop `rust_agent`. No queue/LRU here.
library;

import 'dart:async';
import 'dart:convert';

import 'package:kim_mobile/features/agent/agent_presence.dart';
import 'package:kim_mobile/features/agent/agent_profiles.dart';
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

class AgentRunLoop {
  AgentRunLoop(this.client, this.goose, {this.sink, WorkspaceAccess? access})
    : access = access ?? workspaceAccess;

  final KimClientPort client;
  final AgentBridge goose;
  final AgentRunSink? sink;
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
          final output = promptOverride != null
              ? await promptOverride!(req)
              : await _promptGoose(req);
          sink?.finish(req.dest, failed: false);
          await client.submitAgentRun(
            AgentRunResultDto(
              dest: req.dest,
              profileId: req.profileId,
              epoch: req.epoch,
              output: output,
            ),
          );
        } catch (e, st) {
          KimLogger.warn('agent run', e, st);
          sink?.finish(req.dest, failed: true);
          await client.submitAgentRun(
            AgentRunResultDto(
              dest: req.dest,
              profileId: req.profileId,
              epoch: req.epoch,
              output: '',
              error: _runErrorText(e),
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

  Future<String> _promptGoose(AgentRunRequestDto req) async {
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
    final ws = await resolveAgentProjectRoot(
      profile: profile,
      paths: KimPaths.instance,
    );
    final session = await goose.open(
      sqlitePath: '',
      projectRoot: ws.path,
      opts: SessionOpenOpts(
        model: profile.model,
        llmBackend: account.vendorId,
        resumeOnOpen: false,
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
      ),
    );
    await session.prompt(text: req.text);
    await for (final ev in session.listen()) {
      if (ev.kind == 'assistant_finished') {
        return ev.message;
      }
      if (ev.kind == 'failed') {
        final detail = ev.message.trim();
        throw StateError(detail.isEmpty ? Copy.agentRunFailed : detail);
      }
    }
    throw StateError(Copy.agentRunFailed);
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
    StateError(:final message) => message,
    _ => error.toString(),
  };
}
