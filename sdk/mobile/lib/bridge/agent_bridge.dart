/// Forwards MobileAgent run requests to desktop `rust_agent`. No queue/LRU here.
library;

import 'package:kim_mobile/features/agent/host_support.dart';
import 'package:kim_mobile/bridge/goose_bridge.dart';
import 'package:kim_mobile/core/logger.dart';
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
    final session = await goose.open(
      sqlitePath: '',
      projectRoot: '',
      opts: SessionOpenOpts(
        model: 'gpt-4o',
        llmBackend: 'openai',
        resumeOnOpen: false,
        baseUrl: '',
        apiKey: '',
        enableFsTools: false,
        bashEnabled: false,
        profileId: req.profileId,
        profileJson: '',
        thinkingEffort: '',
        gooseMode: '',
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
}
