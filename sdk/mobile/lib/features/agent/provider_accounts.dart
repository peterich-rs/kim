library;

import 'dart:convert';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:shared_preferences/shared_preferences.dart';

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
  });

  final String id;
  final String vendorId;
  final String baseUrl;
  final String keyRef;
  final String displayName;
  final List<String> models;

  ProviderAccount copyWith({
    String? vendorId,
    String? baseUrl,
    String? keyRef,
    String? displayName,
    List<String>? models,
  }) {
    return ProviderAccount(
      id: id,
      vendorId: vendorId ?? this.vendorId,
      baseUrl: baseUrl ?? this.baseUrl,
      keyRef: keyRef ?? this.keyRef,
      displayName: displayName ?? this.displayName,
      models: models ?? this.models,
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
    final prefs = await SharedPreferences.getInstance();
    final raw = prefs.getString(kProviderAccountsPref);
    var accounts = <ProviderAccount>[];
    if (raw != null && raw.isNotEmpty) {
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
        accounts = [];
      }
    }
    if (!ref.mounted) {
      return;
    }
    state = accounts;
  }

  Future<void> upsert(ProviderAccount account) async {
    await ensureLoaded();
    final next = [
      for (final a in state)
        if (a.id != account.id) a,
      account,
    ];
    await _persist(next);
  }

  Future<void> delete(String id) async {
    await ensureLoaded();
    await _persist([
      for (final a in state)
        if (a.id != id) a,
    ]);
  }

  Future<void> _persist(List<ProviderAccount> next) async {
    final prefs = await SharedPreferences.getInstance();
    await prefs.setString(
      kProviderAccountsPref,
      jsonEncode([for (final a in next) a.toJson()]),
    );
    state = next;
  }
}

final providerAccountsProvider =
    NotifierProvider<ProviderAccountStore, List<ProviderAccount>>(
      ProviderAccountStore.new,
    );
