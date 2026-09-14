/// Dart shell around `kim_agent_ffi` (hard isolation from [KimBridge] / IM).
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'agent/host_support.dart';
import 'src/rust_agent/api/session.dart';
import 'src/rust_agent/frb_generated.dart';

export 'src/rust_agent/api/session.dart'
    show
        AgentSession,
        AgentUiEvent,
        ResumeReportDto,
        SessionOpenOpts,
        SessionSnapshotDto;

/// Production FFI session or a test double. ChatAgent is the only caller.
abstract class AgentSessionPort {
  Stream<AgentUiEvent> listen();
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
  Future<void> reconfigure({required SessionOpenOpts opts});
  Future<ResumeReportDto> resume();
  SessionSnapshotDto snapshot();
}

class NativeAgentSession implements AgentSessionPort {
  NativeAgentSession(this._inner);

  final AgentSession _inner;

  @override
  Stream<AgentUiEvent> listen() => _inner.listen();

  @override
  Future<String> prompt({required String text}) => _inner.prompt(text: text);

  @override
  Future<String> promptWithContext({
    required String text,
    required String contextJson,
  }) => _inner.promptWithContext(text: text, contextJson: contextJson);

  @override
  Future<String> completeTool({
    required String callId,
    required String outputJson,
  }) => _inner.completeTool(callId: callId, outputJson: outputJson);

  @override
  Future<String> respondPermission({
    required String callId,
    required String permission,
  }) => _inner.respondPermission(callId: callId, permission: permission);

  @override
  Future<void> close() => _inner.close();

  @override
  Future<void> abort() => _inner.abort();

  @override
  Future<void> reconfigure({required SessionOpenOpts opts}) =>
      _inner.reconfigure(opts: opts);

  @override
  Future<ResumeReportDto> resume() => _inner.resume();

  @override
  SessionSnapshotDto snapshot() => _inner.snapshot();
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

  Future<AgentSessionPort> open({
    required String sqlitePath,
    required String projectRoot,
    required SessionOpenOpts opts,
  }) async {
    await ensure();
    final session = await sessionOpen(
      sqlitePath: sqlitePath,
      projectRoot: projectRoot,
      opts: opts,
    );
    return NativeAgentSession(session);
  }

  Future<List<String>> fetchModels(SessionOpenOpts opts) async {
    await ensure();
    return fetchSupportedModels(opts: opts);
  }

  Future<List<String>> builtinProfiles() async {
    await ensure();
    return listBuiltinProfiles();
  }

  Future<List<String>> bundledProviders() async {
    await ensure();
    return listBundledProviders();
  }

  Future<String> catalogVendorsJson() async {
    await ensure();
    return catalogVendors();
  }

  Future<String> catalogSurfaceJson({
    required String vendor,
    required String model,
  }) async {
    await ensure();
    return catalogSurface(vendor: vendor, model: model);
  }

  Future<String> catalogValidateChoice({
    required String vendor,
    required String model,
    required String choiceJson,
  }) async {
    await ensure();
    return catalogValidate(
      vendor: vendor,
      model: model,
      choiceJson: choiceJson,
    );
  }

  Future<String> skillAppCatalogJson({required String cacheRoot}) async {
    await ensure();
    return skillAppCatalog(cacheRoot: cacheRoot);
  }

  Future<String> skillPortableListJson({
    required String userRoot,
    required String projectRoot,
  }) async {
    await ensure();
    return skillPortableList(userRoot: userRoot, projectRoot: projectRoot);
  }

  /// Host FFI: `preview_assembled(profile_json, project_root) -> JSON`.
  ///
  /// Expected FRB after host Part A:
  /// `crateApiSessionPreviewAssembled` / generated `previewAssembled(...)`.
  /// Returns empty until that symbol is codegen'd — UI uses local tool projection.
  Future<String> previewAssembled({
    required String profileJson,
    required String projectRoot,
  }) async {
    await ensure();
    // Stub: wire to generated `previewAssembled` once rust_agent FRB lands.
    return '';
  }

  /// Host FFI: `capability_catalog_json() -> JSON`.
  ///
  /// Expected FRB after host Part A:
  /// `crateApiSessionCapabilityCatalogJson` / generated `capabilityCatalogJson()`.
  Future<String> capabilityCatalogJson() async {
    await ensure();
    // Stub: wire to generated `capabilityCatalogJson` once rust_agent FRB lands.
    return '[]';
  }
}
