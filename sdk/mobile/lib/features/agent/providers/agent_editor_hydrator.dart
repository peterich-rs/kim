/// Hydration + reasoning-surface reload for the agent editor page.
library;

import 'package:flutter/widgets.dart';

import 'package:kim_mobile/features/agent/data/catalog.dart';
import 'package:kim_mobile/features/agent/providers/agent_profiles.dart';
import 'package:kim_mobile/features/agent/providers/agent_settings_form.dart';
import 'package:kim_mobile/features/agent/providers/provider_accounts.dart';
import 'package:kim_mobile/features/agent/widgets/context_window.dart';

class AgentEditorHydrator {
  AgentEditorHydrator({
    required this.context,
    required this.form,
    required this.draft,
    required this.profiles,
    required this.accounts,
    required this.ensureVendors,
    required this.surfaceFor,
    required this.defaultModelFor,
    required this.displayName,
    required this.aliases,
    required this.prompt,
    required this.model,
  });

  final BuildContext context;
  final AgentSettingsForm form;
  final AgentSettingsDraft Function() draft;
  final List<AgentProfile> Function() profiles;
  final List<ProviderAccount> Function() accounts;
  final Future<List<VendorSummary>> Function() ensureVendors;
  final Future<ReasoningSurface> Function({
    required String vendor,
    required String model,
  })
  surfaceFor;
  final String Function(ProviderAccount account) defaultModelFor;
  final TextEditingController displayName;
  final TextEditingController aliases;
  final TextEditingController prompt;
  final TextEditingController model;

  AgentSettingsDraft get _draft => draft();

  Future<void> bootstrap({required bool isCreate}) async {
    if (isCreate) {
      return;
    }
    try {
      final vendors = await ensureVendors();
      if (context.mounted) {
        form.setVendors(vendors);
      }
    } catch (_) {}
    await hydrate(isCreate: isCreate);
  }

  Future<void> hydrate({required bool isCreate}) async {
    if (_draft.loaded) {
      return;
    }
    final accountRows = accounts();
    if (isCreate) {
      if (accountRows.isNotEmpty) {
        final resolved = defaultModelFor(accountRows.first);
        model.text = resolved;
        form.seedCreate(
          accountId: accountRows.first.id,
          contextTokens: defaultContextTokens(resolved),
        );
      } else {
        form.markLoaded();
      }
      return;
    }
    final profile = _profileFor(_draft.profile);
    if (profile == null) {
      form.markLoaded();
      return;
    }
    displayName.text = profile.displayName;
    aliases.text = profile.aliases.join(', ');
    prompt.text = profile.systemPrompt;
    model.text = profile.model;
    form.hydrateEditor(
      accountId: profile.accountId,
      contextTokens:
          profile.contextTokens ?? defaultContextTokens(profile.model),
      choice:
          profile.reasoning ??
          ReasoningChoice.fromThinkingEffort(profile.thinkingEffort) ??
          const ReasoningChoice(kind: 'none'),
    );
  }

  AgentProfile? _profileFor(AgentProfile? stored) {
    if (stored != null) {
      for (final p in profiles()) {
        if (p.id == stored.id) {
          return p;
        }
      }
      return stored;
    }
    return null;
  }
}
