/// Desktop catalog and skill queries. Turns run in `HostAgentRuntime`.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:kim_mobile/features/agent/host_support.dart';
import 'package:kim_mobile/src/rust_agent/api/catalog.dart' as catalog;
import 'package:kim_mobile/src/rust_agent/api/session.dart' as session;
import 'package:kim_mobile/src/rust_agent/frb_generated.dart';

export 'package:kim_mobile/src/rust_agent/api/catalog.dart'
    show
        AssembledPreview,
        CapabilityEntry,
        CatalogValidate,
        PreviewTool,
        Skill,
        Vendor;
export 'package:kim_mobile/src/rust_agent/api/session.dart'
    show AgentUiEvent, ResumeReport, SessionSnapshot;

/// Test double for the old session loop. Production does not open one.
abstract class AgentSessionPort {
  Stream<session.AgentUiEvent> listen();
  Future<String> prompt({required String text});
  Future<String> promptWithContext({
    required String text,
    required String contextJson,
  });
  Future<String> completeTool({
    required String callId,
    required String outputJson,
  });
  Future<String> respondPermission({
    required String callId,
    required String permission,
  });
  Future<void> close();
  Future<void> abort();
  Future<void> park();
  Future<void> steer({required String text});
  Future<void> reconfigure();
  Future<session.ResumeReport> resume();
  Future<session.SessionSnapshot> snapshot();
}

final agentBridgeProvider = Provider<AgentBridge>((ref) => AgentBridge());

class AgentBridge {
  static bool _inited = false;

  Future<void> ensure() async {
    if (_inited || !agentHostSupported) {
      return;
    }
    await AgentRustLib.init();
    _inited = true;
  }

  bool get isReady => _inited;

  Future<List<String>> fetchModels({
    required String vendor,
    required String baseUrl,
    required String apiKey,
  }) async {
    await ensure();
    return session.fetchSupportedModels(
      llmBackend: vendor,
      baseUrl: baseUrl,
      apiKey: apiKey,
    );
  }

  Future<List<String>> builtinProfiles() async {
    await ensure();
    return session.listBuiltinProfiles();
  }

  Future<List<String>> bundledProviders() async {
    await ensure();
    return session.listBundledProviders();
  }

  Future<List<catalog.Vendor>> catalogVendors() async {
    await ensure();
    return catalog.catalogVendors();
  }

  Future<catalog.ReasoningSurface> catalogSurface({
    required String vendor,
    required String model,
  }) async {
    await ensure();
    return catalog.catalogSurface(vendor: vendor, model: model);
  }

  Future<catalog.CatalogValidate> catalogValidateChoice({
    required String vendor,
    required String model,
    required String kind,
    required bool on,
    required String value,
    required int budget,
  }) async {
    await ensure();
    return catalog.catalogValidate(
      vendor: vendor,
      model: model,
      choiceKind: kind,
      on_: on,
      value: value,
      budget: budget,
    );
  }

  Future<List<catalog.Skill>> skillAppCatalog({required String cacheRoot}) async {
    await ensure();
    return catalog.skillAppCatalog(cacheRoot: cacheRoot);
  }

  Future<List<catalog.Skill>> skillPortableList({
    required String userRoot,
    required String projectRoot,
  }) async {
    await ensure();
    return catalog.skillPortableList(
      userRoot: userRoot,
      projectRoot: projectRoot,
    );
  }

  Future<catalog.AssembledPreview> previewAssembled({
    required String profileJson,
    required String projectRoot,
  }) async {
    await ensure();
    return catalog.previewAssembled(
      profileJson: profileJson,
      projectRoot: projectRoot,
    );
  }
}
