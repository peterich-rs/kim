/// Vendor / credentials card + models card for the provider account page.
library;

import 'package:flutter/material.dart';

import 'package:kim_mobile/copy.dart';
import 'package:kim_mobile/design/kim_group.dart';
import 'package:kim_mobile/features/agent/data/catalog.dart';

class ProviderVendorCard extends StatelessWidget {
  const ProviderVendorCard({
    super.key,
    required this.isCreate,
    required this.vendor,
    required this.vendors,
    required this.selectedVendor,
    required this.name,
    required this.url,
    required this.keyController,
    required this.onVendorChanged,
    required this.onNameChanged,
    this.onUrlChanged,
  });

  final bool isCreate;
  final VendorSummary? vendor;
  final List<VendorSummary> vendors;
  final String selectedVendor;
  final TextEditingController name;
  final TextEditingController url;
  final TextEditingController keyController;
  final void Function(String next, VendorSummary? hit) onVendorChanged;
  final VoidCallback? onNameChanged;
  final VoidCallback? onUrlChanged;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    final urlChoices = <String>{
      if (vendor != null && vendor!.defaultBaseUrl.isNotEmpty)
        vendor!.defaultBaseUrl,
      if (vendor != null) ...vendor!.altBaseUrls,
    }.toList();
    return KimGroupCard(
      children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(16, 12, 16, 12),
          child: DropdownButton<String>(
            key: const Key('provider-vendor'),
            value: () {
              final ids = {for (final v in vendors) v.id};
              if (ids.contains(selectedVendor) || vendors.isEmpty) {
                return selectedVendor;
              }
              return vendors.first.id;
            }(),
            isExpanded: true,
            items: [
              for (final v in sortVendors(vendors))
                DropdownMenuItem(value: v.id, child: Text(v.displayName)),
              if (vendors.every((v) => v.id != selectedVendor) &&
                  selectedVendor.isNotEmpty)
                DropdownMenuItem(
                  value: selectedVendor,
                  child: Text(selectedVendor),
                ),
            ],
            onChanged: (next) {
              if (next == null) {
                return;
              }
              VendorSummary? hit;
              for (final v in vendors) {
                if (v.id == next) {
                  hit = v;
                  break;
                }
              }
              onVendorChanged(next, hit);
            },
          ),
        ),
        const Divider(height: 1),
        ListTile(
          title: Text(l10n.agentDisplayName),
          subtitle: TextField(
            key: const Key('provider-name'),
            controller: name,
            decoration: InputDecoration(
              border: InputBorder.none,
              hintText: vendor?.displayName ?? selectedVendor,
            ),
            onChanged: (_) => onNameChanged?.call(),
          ),
        ),
        const Divider(height: 1),
        ListTile(
          title: Text(Copy.agentBaseUrl),
          subtitle: TextField(
            key: const Key('provider-url'),
            controller: url,
            decoration: InputDecoration(
              border: InputBorder.none,
              hintText: vendor?.defaultBaseUrl.isNotEmpty == true
                  ? vendor!.defaultBaseUrl
                  : 'https://api.openai.com/v1',
            ),
          ),
        ),
        if (urlChoices.length > 1) ...[
          const Divider(height: 1),
          Padding(
            padding: const EdgeInsets.fromLTRB(16, 8, 16, 12),
            child: DropdownButton<String>(
              value: urlChoices.contains(url.text) ? url.text : null,
              hint: Text(l10n.agentAltUrl),
              isExpanded: true,
              items: [
                for (final u in urlChoices)
                  DropdownMenuItem(value: u, child: Text(u)),
              ],
              onChanged: (next) {
                if (next != null) {
                  url.text = next;
                  onUrlChanged?.call();
                }
              },
            ),
          ),
        ],
        const Divider(height: 1),
        ListTile(
          title: Text(Copy.agentApiKey),
          subtitle: TextField(
            key: const Key('provider-key'),
            controller: keyController,
            obscureText: true,
            decoration: InputDecoration(
              border: InputBorder.none,
              hintText: isCreate ? Copy.agentApiKeyHint : l10n.agentKeepKeyHint,
            ),
          ),
        ),
      ],
    );
  }
}

class ProviderModelsCard extends StatelessWidget {
  const ProviderModelsCard({
    super.key,
    required this.models,
    required this.fetching,
    required this.onRefresh,
  });

  final List<String> models;
  final bool fetching;
  final VoidCallback onRefresh;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    return KimGroupCard(
      children: [
        if (models.isEmpty)
          ListTile(title: Text(l10n.agentModelHint))
        else
          for (final m in models) ...[
            if (m != models.first) const Divider(height: 1),
            ListTile(dense: true, title: Text(m)),
          ],
        const Divider(height: 1),
        ListTile(
          title: Text(l10n.agentFetchModels),
          trailing: fetching
              ? const SizedBox(
                  width: 16,
                  height: 16,
                  child: CircularProgressIndicator(strokeWidth: 2),
                )
              : const Icon(Icons.refresh, size: 18),
          onTap: fetching ? null : onRefresh,
        ),
      ],
    );
  }
}
