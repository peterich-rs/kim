library;

import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:kim_mobile/features/agent/catalog.dart';

class ProviderAccountDraft {
  const ProviderAccountDraft({
    this.vendor = 'openai',
    this.models = const [],
    this.vendors = const [],
    this.fetching = false,
    this.hydrated = false,
    this.revision = 0,
  });

  final String vendor;
  final List<String> models;
  final List<VendorSummary> vendors;
  final bool fetching;
  final bool hydrated;
  final int revision;

  ProviderAccountDraft copyWith({
    String? vendor,
    List<String>? models,
    List<VendorSummary>? vendors,
    bool? fetching,
    bool? hydrated,
    int? revision,
  }) {
    return ProviderAccountDraft(
      vendor: vendor ?? this.vendor,
      models: models ?? this.models,
      vendors: vendors ?? this.vendors,
      fetching: fetching ?? this.fetching,
      hydrated: hydrated ?? this.hydrated,
      revision: revision ?? this.revision,
    );
  }
}

class ProviderAccountForm extends Notifier<ProviderAccountDraft> {
  @override
  ProviderAccountDraft build() => const ProviderAccountDraft();

  void bump() => state = state.copyWith(revision: state.revision + 1);

  void setVendors(List<VendorSummary> vendors) =>
      state = state.copyWith(vendors: vendors);

  void applyHydrated({required String vendor, required List<String> models}) {
    state = state.copyWith(hydrated: true, vendor: vendor, models: models);
  }

  void setModels(List<String> models) => state = state.copyWith(models: models);

  void setFetching(bool value) => state = state.copyWith(fetching: value);

  void setVendor(String vendor) => state = state.copyWith(vendor: vendor);
}

final providerAccountFormProvider =
    NotifierProvider.autoDispose<ProviderAccountForm, ProviderAccountDraft>(
      ProviderAccountForm.new,
    );
