/// Catalog rows from `catalog_vendors` / `catalog_surface`. Callers switch on
/// [ReasoningSurface.kind] only — never on vendor id.
library;

import 'dart:convert';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:shared_preferences/shared_preferences.dart';

import 'package:kim_mobile/bridge/goose_bridge.dart';
import 'package:kim_mobile/features/agent/agent_profiles.dart';
import 'package:kim_mobile/features/agent/provider_accounts.dart';

const kCatalogCachePrefix = 'agent.catalog_cache.';

/// `/v1/models` on gateways often includes routing aliases (`gpt-*`).
bool isSelectableModelId(String id) {
  final trimmed = id.trim();
  return trimmed.isNotEmpty && !trimmed.contains('*') && !trimmed.contains('?');
}

List<String> selectableModelIds(Iterable<String> ids) {
  final seen = <String>{};
  final out = <String>[];
  for (final raw in ids) {
    final id = raw.trim();
    if (!isSelectableModelId(id) || !seen.add(id)) {
      continue;
    }
    out.add(id);
  }
  return out;
}

/// Refresh success: keep hand-typed ids that the vendor did not return.
List<String> unionAccountModels(
  Iterable<String> fetched,
  Iterable<String> existing,
) => selectableModelIds([...fetched, ...existing]);

/// Catalog default if it is on the account, else first model, else empty.
String defaultModelForAccount(
  ProviderAccount account, [
  VendorSummary? vendor,
]) {
  final def = vendor?.defaultModel.trim() ?? '';
  if (def.isNotEmpty && account.models.contains(def)) {
    return def;
  }
  if (account.models.isNotEmpty) {
    return account.models.first;
  }
  return '';
}

/// Catalog seed ∪ vendor cache. Empty [existing] only — do not replace a live list.
Future<List<String>> migrateAccountModelIds({
  required String vendorId,
  required List<String> existing,
  List<String> catalogModels = const [],
  String defaultModel = '',
}) async {
  if (existing.isNotEmpty) {
    return selectableModelIds(existing);
  }
  final cached = await loadCatalogModelCache(vendorId);
  return selectableModelIds([...catalogModels, defaultModel, ...cached]);
}

bool isAllowedAgentBaseUrl(String raw) {
  final uri = Uri.tryParse(raw.trim());
  if (uri == null || !uri.hasScheme || uri.host.isEmpty) {
    return false;
  }
  if (uri.scheme == 'https') {
    return true;
  }
  if (uri.scheme == 'http') {
    final host = uri.host.toLowerCase();
    return host == '127.0.0.1' || host == 'localhost';
  }
  return false;
}

Future<List<String>> loadCatalogModelCache(String vendor) async {
  if (vendor.isEmpty) {
    return const [];
  }
  final prefs = await SharedPreferences.getInstance();
  final raw = prefs.getString('$kCatalogCachePrefix$vendor');
  if (raw == null || raw.isEmpty) {
    return const [];
  }
  try {
    final decoded = jsonDecode(raw);
    if (decoded is List) {
      return selectableModelIds([for (final m in decoded) '$m']);
    }
  } catch (_) {}
  return const [];
}

Future<void> saveCatalogModelCache(String vendor, List<String> models) async {
  if (vendor.isEmpty) {
    return;
  }
  final prefs = await SharedPreferences.getInstance();
  await prefs.setString('$kCatalogCachePrefix$vendor', jsonEncode(models));
}

class VendorSummary {
  const VendorSummary({
    required this.id,
    required this.displayName,
    required this.group,
    required this.sortRank,
    required this.defaultBaseUrl,
    this.altBaseUrls = const [],
    this.dynamicModels = false,
    this.customModel = true,
    this.defaultModel = '',
    this.models = const [],
  });

  final String id;
  final String displayName;

  /// `primary` | `gateway` | `other`
  final String group;
  final int sortRank;
  final String defaultBaseUrl;
  final List<String> altBaseUrls;
  final bool dynamicModels;
  final bool customModel;
  final String defaultModel;
  final List<String> models;

  factory VendorSummary.fromJson(Map<String, Object?> json) {
    final alts = json['alt_base_urls'];
    final models = json['models'];
    return VendorSummary(
      id: '${json['id'] ?? ''}',
      displayName: '${json['display_name'] ?? json['id'] ?? ''}',
      group: '${json['group'] ?? 'other'}',
      sortRank: json['sort_rank'] is int ? json['sort_rank'] as int : 0,
      defaultBaseUrl: '${json['default_base_url'] ?? ''}',
      altBaseUrls: alts is List ? [for (final u in alts) '$u'] : const [],
      dynamicModels: json['dynamic_models'] == true,
      customModel: json['custom_model'] != false,
      defaultModel: '${json['default_model'] ?? ''}',
      models: models is List ? [for (final m in models) '$m'] : const [],
    );
  }

  static List<VendorSummary> listFromJson(String raw) {
    final decoded = jsonDecode(raw);
    if (decoded is! List) {
      return const [];
    }
    return [
      for (final item in decoded)
        if (item is Map)
          VendorSummary.fromJson(Map<String, Object?>.from(item)),
    ];
  }
}

class ReasoningSurface {
  const ReasoningSurface({
    required this.kind,
    this.note,
    this.defaultOn,
    this.allowed = const [],
    this.defaultValue,
    this.min,
    this.max,
    this.defaultBudget,
  });

  /// `none` | `always_on` | `toggle` | `effort_enum` | `budget_tokens`
  final String kind;
  final String? note;
  final bool? defaultOn;
  final List<String> allowed;
  final String? defaultValue;
  final int? min;
  final int? max;
  final int? defaultBudget;

  factory ReasoningSurface.fromJson(Map<String, Object?> json) {
    final allowed = json['allowed'];
    return ReasoningSurface(
      kind: '${json['kind'] ?? 'none'}',
      note: json['note'] is String ? json['note'] as String : null,
      defaultOn: json['default_on'] as bool?,
      allowed: allowed is List ? [for (final a in allowed) '$a'] : const [],
      defaultValue: json['default'] is String
          ? json['default'] as String
          : null,
      min: json['min'] is int ? json['min'] as int : null,
      max: json['max'] is int ? json['max'] as int : null,
      defaultBudget: json['default'] is int ? json['default'] as int : null,
    );
  }

  static ReasoningSurface fromJsonString(String raw) {
    final decoded = jsonDecode(raw);
    if (decoded is! Map) {
      return const ReasoningSurface(kind: 'none');
    }
    return ReasoningSurface.fromJson(Map<String, Object?>.from(decoded));
  }
}

int vendorGroupRank(String group) {
  return switch (group) {
    'primary' => 0,
    'gateway' => 1,
    _ => 2,
  };
}

/// Primary first, then gateway, then other. Within a group, [sortRank] only.
List<VendorSummary> sortVendors(Iterable<VendorSummary> vendors) {
  final out = [...vendors];
  out.sort((a, b) {
    final g = vendorGroupRank(a.group).compareTo(vendorGroupRank(b.group));
    if (g != 0) {
      return g;
    }
    return a.sortRank.compareTo(b.sortRank);
  });
  return out;
}

List<VendorSummary> vendorsInGroup(
  Iterable<VendorSummary> vendors,
  String group,
) {
  return [
    for (final v in sortVendors(vendors))
      if (v.group == group) v,
  ];
}

ReasoningChoice defaultChoiceFor(ReasoningSurface surface) {
  switch (surface.kind) {
    case 'always_on':
      return const ReasoningChoice(kind: 'always_on');
    case 'toggle':
      return ReasoningChoice(kind: 'toggle', on: surface.defaultOn ?? false);
    case 'effort_enum':
      final value =
          surface.defaultValue ??
          (surface.allowed.isNotEmpty ? surface.allowed.first : '');
      return ReasoningChoice(kind: 'effort_enum', value: value);
    case 'budget_tokens':
      return ReasoningChoice(
        kind: 'budget_tokens',
        budget: surface.defaultBudget ?? surface.min ?? 0,
      );
    default:
      return const ReasoningChoice(kind: 'none');
  }
}

class AlignedChoice {
  const AlignedChoice({required this.choice, required this.dropped});

  final ReasoningChoice choice;
  final bool dropped;
}

/// If [current] does not fit [surface], fall back to the surface default.
AlignedChoice alignChoice(
  ReasoningSurface surface,
  ReasoningChoice? current,
) {
  if (current == null) {
    return AlignedChoice(choice: defaultChoiceFor(surface), dropped: false);
  }
  if (current.kind == 'advanced') {
    return AlignedChoice(choice: current, dropped: false);
  }
  if (current.kind != surface.kind) {
    return AlignedChoice(choice: defaultChoiceFor(surface), dropped: true);
  }
  if (surface.kind == 'effort_enum') {
    final value = current.value ?? '';
    final ok = surface.allowed.any(
      (a) => a.toLowerCase() == value.toLowerCase(),
    );
    if (!ok) {
      return AlignedChoice(choice: defaultChoiceFor(surface), dropped: true);
    }
  }
  if (surface.kind == 'budget_tokens') {
    final budget = current.budget ?? 0;
    final min = surface.min ?? 0;
    final max = surface.max ?? 1 << 30;
    if (budget < min || budget > max) {
      return AlignedChoice(choice: defaultChoiceFor(surface), dropped: true);
    }
  }
  return AlignedChoice(choice: current, dropped: false);
}

class CatalogValidateResult {
  const CatalogValidateResult({required this.choice, this.dropped = const []});

  final ReasoningChoice choice;
  final List<String> dropped;

  factory CatalogValidateResult.fromJsonString(String raw) {
    final decoded = jsonDecode(raw);
    if (decoded is! Map) {
      return const CatalogValidateResult(choice: ReasoningChoice(kind: 'none'));
    }
    final map = Map<String, Object?>.from(decoded);
    final choiceRaw = map['choice'];
    final droppedRaw = map['dropped'];
    return CatalogValidateResult(
      choice: choiceRaw is Map
          ? ReasoningChoice.fromJson(Map<String, Object?>.from(choiceRaw))
          : const ReasoningChoice(kind: 'none'),
      dropped: droppedRaw is List
          ? [for (final n in droppedRaw) '$n']
          : const [],
    );
  }
}

class CatalogRepository {
  CatalogRepository(this._bridge);

  final AgentBridge _bridge;
  List<VendorSummary> vendors = const [];

  Future<List<VendorSummary>> ensureVendors() async {
    if (vendors.isNotEmpty) {
      return vendors;
    }
    final rows = await _bridge.catalogVendors();
    vendors = sortVendors([
      for (final row in rows)
        VendorSummary(
          id: row.id,
          displayName: row.displayName,
          group: row.group,
          sortRank: row.sortRank,
          defaultBaseUrl: row.defaultBaseUrl,
          altBaseUrls: row.altBaseUrls,
          dynamicModels: row.dynamicModels,
          customModel: row.customModel,
          defaultModel: row.defaultModel,
          models: row.models,
        ),
    ]);
    return vendors;
  }

  Future<ReasoningSurface> surface({
    required String vendor,
    required String model,
  }) async {
    final row = await _bridge.catalogSurface(vendor: vendor, model: model);
    return ReasoningSurface(
      kind: row.kind,
      note: row.note.isEmpty ? null : row.note,
      defaultOn: row.kind == 'toggle' ? row.defaultOn : null,
      allowed: row.allowed,
      defaultValue: row.defaultValue.isEmpty ? null : row.defaultValue,
      min: row.kind == 'budget_tokens' ? row.min : null,
      max: row.kind == 'budget_tokens' ? row.max : null,
      defaultBudget: row.kind == 'budget_tokens' ? row.defaultBudget : null,
    );
  }

  Future<CatalogValidateResult> validate({
    required String vendor,
    required String model,
    required ReasoningChoice choice,
  }) async {
    final row = await _bridge.catalogValidateChoice(
      vendor: vendor,
      model: model,
      kind: choice.kind,
      on: choice.on ?? false,
      value: choice.value ?? '',
      budget: choice.budget ?? 0,
    );
    return CatalogValidateResult(
      choice: ReasoningChoice(
        kind: row.kind,
        on: row.kind == 'toggle' ? row.on_ : null,
        value: row.value.isEmpty ? null : row.value,
        budget: row.kind == 'budget_tokens' ? row.budget : null,
      ),
      dropped: row.dropped,
    );
  }
}

final catalogRepositoryProvider = Provider<CatalogRepository>((ref) {
  return CatalogRepository(ref.watch(agentBridgeProvider));
});
