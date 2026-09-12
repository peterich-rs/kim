/// Catalog DTOs from `catalog_vendors` / `catalog_surface`. Callers switch on
/// [ReasoningSurfaceDto.kind] only — never on vendor id.
library;

import 'dart:convert';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:shared_preferences/shared_preferences.dart';

import '../agent_bridge.dart';
import '../state/agent_profiles.dart';

const kCatalogCachePrefix = 'agent.catalog_cache.';

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
      return [for (final m in decoded) '$m'];
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

class VendorSummaryDto {
  const VendorSummaryDto({
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

  factory VendorSummaryDto.fromJson(Map<String, Object?> json) {
    final alts = json['alt_base_urls'];
    final models = json['models'];
    return VendorSummaryDto(
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

  static List<VendorSummaryDto> listFromJson(String raw) {
    final decoded = jsonDecode(raw);
    if (decoded is! List) {
      return const [];
    }
    return [
      for (final item in decoded)
        if (item is Map)
          VendorSummaryDto.fromJson(Map<String, Object?>.from(item)),
    ];
  }
}

class ReasoningSurfaceDto {
  const ReasoningSurfaceDto({
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

  factory ReasoningSurfaceDto.fromJson(Map<String, Object?> json) {
    final allowed = json['allowed'];
    return ReasoningSurfaceDto(
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

  static ReasoningSurfaceDto fromJsonString(String raw) {
    final decoded = jsonDecode(raw);
    if (decoded is! Map) {
      return const ReasoningSurfaceDto(kind: 'none');
    }
    return ReasoningSurfaceDto.fromJson(Map<String, Object?>.from(decoded));
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
List<VendorSummaryDto> sortVendors(Iterable<VendorSummaryDto> vendors) {
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

List<VendorSummaryDto> vendorsInGroup(
  Iterable<VendorSummaryDto> vendors,
  String group,
) {
  return [
    for (final v in sortVendors(vendors))
      if (v.group == group) v,
  ];
}

ReasoningChoice defaultChoiceFor(ReasoningSurfaceDto surface) {
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
  ReasoningSurfaceDto surface,
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

class CatalogRepository {
  CatalogRepository(this._bridge);

  final AgentBridge _bridge;
  List<VendorSummaryDto> vendors = const [];

  Future<List<VendorSummaryDto>> ensureVendors() async {
    if (vendors.isNotEmpty) {
      return vendors;
    }
    final raw = await _bridge.catalogVendorsJson();
    vendors = sortVendors(VendorSummaryDto.listFromJson(raw));
    return vendors;
  }

  Future<ReasoningSurfaceDto> surface({
    required String vendor,
    required String model,
  }) async {
    final raw = await _bridge.catalogSurfaceJson(vendor: vendor, model: model);
    return ReasoningSurfaceDto.fromJsonString(raw);
  }

  Future<void> validate({
    required String vendor,
    required String model,
    required ReasoningChoice choice,
  }) {
    return _bridge.catalogValidateChoice(
      vendor: vendor,
      model: model,
      choiceJson: jsonEncode(choice.toJson()),
    );
  }
}

final catalogRepositoryProvider = Provider<CatalogRepository>((ref) {
  return CatalogRepository(ref.watch(agentBridgeProvider));
});
