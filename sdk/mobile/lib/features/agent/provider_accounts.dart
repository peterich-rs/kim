library;

import 'dart:convert';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:shared_preferences/shared_preferences.dart';

import 'package:kim_mobile/bridge/kim_bridge.dart';
import 'package:kim_mobile/core/logger.dart';
import 'package:kim_mobile/features/session/providers.dart';
import 'package:kim_mobile/src/rust/api/types.dart' as rust_types;

const kProviderAccountsPref = 'agent.provider_accounts';
const kGooseAccountId = 'acct-goose';
const kAccountKeyPrefix = 'agent.api_key.acct.';
const kGooseKeyRef = 'agent.api_key.goose';

String canonicalizeVendorId(String raw) {
  switch (raw.trim().toLowerCase()) {
    case 'custom_deepseek':
      return 'deepseek';
    case 'alibaba':
      return 'qwen';
    case 'openai_compatible':
      return 'openai_compatible';
    case 'xai':
    case 'grok':
      return 'xai';
    case 'openai':
    case 'responses_http':
    case 'live':
    case 'responses':
    case '':
      return 'openai';
    case 'anthropic':
    case 'messages':
      return 'anthropic';
    default:
      return raw.trim().toLowerCase();
  }
}

class MissingProviderAccount implements Exception {
  const MissingProviderAccount([this.accountId = '']);

  final String accountId;

  @override
  String toString() => '厂商账号已删除';
}

class ProviderAccount {
  const ProviderAccount({
    required this.id,
    required this.vendorId,
    required this.baseUrl,
    required this.keyRef,
    this.displayName = '',
    this.models = const [],
    this.updatedAt = 0,
    this.deletedAt = 0,
  });

  final String id;
  final String vendorId;
  final String baseUrl;
  final String keyRef;
  final String displayName;
  final List<String> models;
  final int updatedAt;
  final int deletedAt;

  ProviderAccount copyWith({
    String? vendorId,
    String? baseUrl,
    String? keyRef,
    String? displayName,
    List<String>? models,
    int? updatedAt,
    int? deletedAt,
  }) {
    return ProviderAccount(
      id: id,
      vendorId: vendorId ?? this.vendorId,
      baseUrl: baseUrl ?? this.baseUrl,
      keyRef: keyRef ?? this.keyRef,
      displayName: displayName ?? this.displayName,
      models: models ?? this.models,
      updatedAt: updatedAt ?? this.updatedAt,
      deletedAt: deletedAt ?? this.deletedAt,
    );
  }

  Map<String, Object?> toJson() => {
    'id': id,
    'vendor_id': vendorId,
    'base_url': baseUrl,
    'key_ref': keyRef,
    'display_name': displayName,
    'models': models,
  };

  factory ProviderAccount.fromJson(Map<String, Object?> json) {
    final modelsRaw = json['models'];
    return ProviderAccount(
      id: json['id'] as String? ?? '',
      vendorId: canonicalizeVendorId(json['vendor_id'] as String? ?? ''),
      baseUrl: json['base_url'] as String? ?? '',
      keyRef: json['key_ref'] as String? ?? '',
      displayName: json['display_name'] as String? ?? '',
      models: modelsRaw is List ? [for (final m in modelsRaw) '$m'] : const [],
    );
  }

  static ProviderAccount fromLegacyProfile({
    required String profileId,
    required String providerKind,
    required String baseUrl,
    required String keyRef,
  }) {
    final id = profileId == 'goose' ? kGooseAccountId : 'acct-$profileId';
    final vendor = canonicalizeVendorId(providerKind);
    return ProviderAccount(
      id: id,
      vendorId: vendor,
      baseUrl: baseUrl,
      keyRef: keyRef.isNotEmpty
          ? keyRef
          : (profileId == 'goose' ? kGooseKeyRef : '$kAccountKeyPrefix$id'),
      displayName: vendor,
    );
  }
}

class ProviderAccountStore extends Notifier<List<ProviderAccount>> {
  Future<void>? _load;

  @override
  List<ProviderAccount> build() {
    _load = _reload();
    return const [];
  }

  Future<void> ensureLoaded() async {
    await (_load ?? _reload());
  }

  ProviderAccount? byId(String id) {
    if (id.isEmpty) {
      return null;
    }
    for (final a in state) {
      if (a.id == id) {
        return a;
      }
    }
    return null;
  }

  Future<void> _reload() async {
    var accounts = <ProviderAccount>[];
    try {
      final client = ref.read(clientPortProvider);
      final rows = await client.listProviderAccounts();
      accounts = [
        for (final row in rows)
          if (row.id.isNotEmpty)
            ProviderAccount(
              id: row.id,
              vendorId: canonicalizeVendorId(row.vendorId),
              baseUrl: row.baseUrl,
              keyRef: row.keyRef,
              displayName: row.displayName,
              models: _modelsFromJson(row.modelsJson),
              updatedAt: row.updatedAt.toInt(),
              deletedAt: row.deletedAt.toInt(),
            ),
      ];
      if (accounts.isEmpty) {
        accounts = await _importPrefsOnce(client);
      }
    } catch (e, st) {
      KimLogger.warn('provider accounts load', e, st);
    }
    if (!ref.mounted) {
      return;
    }
    state = accounts;
  }

  Future<List<ProviderAccount>> _importPrefsOnce(KimClientPort client) async {
    final prefs = await SharedPreferences.getInstance();
    final raw = prefs.getString(kProviderAccountsPref);
    if (raw == null || raw.isEmpty) {
      return const [];
    }
    final accounts = <ProviderAccount>[];
    try {
      final decoded = jsonDecode(raw);
      if (decoded is List) {
        for (final item in decoded) {
          if (item is Map) {
            accounts.add(
              ProviderAccount.fromJson(Map<String, Object?>.from(item)),
            );
          }
        }
      }
    } catch (_) {
      return const [];
    }
    for (final a in accounts) {
      await client.upsertProviderAccount(_toRow(a));
    }
    await prefs.remove(kProviderAccountsPref);
    return accounts;
  }

  Future<void> upsert(ProviderAccount account) async {
    await ensureLoaded();
    await _upsertOne(
      account.copyWith(updatedAt: DateTime.now().millisecondsSinceEpoch),
    );
  }

  Future<void> delete(String id) async {
    await ensureLoaded();
    try {
      await ref.read(clientPortProvider).deleteProviderAccount(id);
    } catch (e, st) {
      KimLogger.warn('provider account delete', e, st);
    }
    state = [
      for (final a in state)
        if (a.id != id) a,
    ];
  }

  Future<void> _upsertOne(ProviderAccount account) async {
    try {
      await ref.read(clientPortProvider).upsertProviderAccount(_toRow(account));
    } catch (e, st) {
      KimLogger.warn('provider accounts persist', e, st);
    }
    final exists = state.any((a) => a.id == account.id);
    state = [
      for (final a in state)
        if (a.id == account.id) account else a,
      if (!exists) account,
    ];
  }

  rust_types.ProviderAccount _toRow(ProviderAccount a) {
    return rust_types.ProviderAccount(
      id: a.id,
      vendorId: a.vendorId,
      baseUrl: a.baseUrl,
      keyRef: a.keyRef,
      displayName: a.displayName,
      modelsJson: jsonEncode(a.models),
      updatedAt: a.updatedAt,
      deletedAt: a.deletedAt,
    );
  }
}

List<String> _modelsFromJson(String raw) {
  if (raw.trim().isEmpty) {
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

final providerAccountsProvider =
    NotifierProvider<ProviderAccountStore, List<ProviderAccount>>(
      ProviderAccountStore.new,
    );
