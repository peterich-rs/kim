/// Forwards MobileAgent run requests to desktop `rust_agent`. No queue/LRU here.
library;

import 'dart:convert';

import 'package:kim_mobile/features/agent/agent_profiles.dart';
import 'package:kim_mobile/features/agent/host_support.dart';
import 'package:kim_mobile/features/agent/mention.dart';
import 'package:kim_mobile/features/agent/provider_accounts.dart';
import 'package:kim_mobile/features/agent/workspace.dart';
import 'package:kim_mobile/bridge/goose_bridge.dart';
import 'package:kim_mobile/core/logger.dart';
import 'package:kim_mobile/core/paths.dart';
import 'package:kim_mobile/core/settings.dart';
import 'package:kim_mobile/bridge/kim_bridge.dart';
import 'package:kim_mobile/src/rust/api/types.dart';

class AgentRunLoop {
  AgentRunLoop(this.client, this.goose);

  final KimClientPort client;
  final AgentBridge goose;

  /// Tests inject a prompt stub. Production uses [rust_agent] `prompt`.
  Future<String> Function(AgentRunRequestDto req)? promptOverride;

  Future<void> start() async {
    if (!agentHostSupported && promptOverride == null) {
      return;
    }
    await for (final req in client.watchAgentRun()) {
      try {
        final output = promptOverride != null
            ? await promptOverride!(req)
            : await _promptGoose(req);
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
        await client.submitAgentRun(
          AgentRunResultDto(
            dest: req.dest,
            profileId: req.profileId,
            epoch: req.epoch,
            output: '',
            error: e.toString(),
          ),
        );
      }
    }
  }

  Future<String> _promptGoose(AgentRunRequestDto req) async {
    await goose.ensure();
    if (!goose.isReady) {
      return '';
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
        profileJson: jsonEncode(profile.toHostJson(account)),
        thinkingEffort: profile.thinkingEffort,
        gooseMode: profile.mode,
        enableKimTools: false,
        enableApprovals: false,
        sessionId: '${req.dest}:${req.profileId}',
      ),
    );
    await session.prompt(text: req.text);
    await for (final ev in session.listen()) {
      if (ev.kind == 'assistant_finished' || ev.kind == 'failed') {
        return ev.message;
      }
    }
    return '';
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
    if (account.keyRef == 'agent.api_key.goose' || profile.id == kGooseAgentId) {
      final goose = await read('agent.api_key.goose');
      if (goose.isNotEmpty) {
        return goose;
      }
      return await read('agent.api_key');
    }
    return '';
  }
}
