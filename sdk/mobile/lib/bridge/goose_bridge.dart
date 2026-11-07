/// Desktop catalog and skill queries. Turns run in `HostAgentRuntime`.
/// Model fetch / preview live on the client FFI (vault-backed).
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:kim_mobile/features/agent/data/host_support.dart';
import 'package:kim_mobile/src/rust_agent/api/catalog.dart' as catalog;
import 'package:kim_mobile/src/rust_agent/frb_generated.dart';

export 'package:kim_mobile/src/rust_agent/api/catalog.dart'
    show CapabilityEntry, CatalogValidate, Skill, Vendor;

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

  Future<List<String>> builtinProfiles() async {
    await ensure();
    return catalog.listBuiltinProfiles();
  }

  Future<List<String>> bundledProviders() async {
    await ensure();
    return catalog.listBundledProviders();
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

  Future<List<catalog.Skill>> skillAppCatalog() async {
    await ensure();
    return catalog.skillAppCatalog();
  }

  Future<List<catalog.Skill>> skillPortableList() async {
    await ensure();
    return catalog.skillPortableList();
  }
}
