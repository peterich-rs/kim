library;

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:gap/gap.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import 'package:kim_mobile/copy.dart';
import 'package:kim_mobile/design/empty_state.dart';
import 'package:kim_mobile/design/kim_group.dart';
import 'package:kim_mobile/features/agent/agent_create_form.dart';
import 'package:kim_mobile/features/agent/agent_runtime_switch.dart';
import 'package:kim_mobile/features/agent/catalog.dart';
import 'package:kim_mobile/features/agent/context_window.dart';
import 'package:kim_mobile/features/agent/context_window_controls.dart';
import 'package:kim_mobile/features/agent/create/agent_create_helpers.dart';
import 'package:kim_mobile/features/agent/provider_account_page.dart';
import 'package:kim_mobile/features/agent/provider_accounts.dart';
import 'package:kim_mobile/features/agent/reasoning_controls.dart';

/// Step 0: display name, runtime, provider, model, context, reasoning.
class AgentCreateStepBasics extends ConsumerWidget {
  const AgentCreateStepBasics({
    super.key,
    required this.displayName,
    required this.model,
  });

  final TextEditingController displayName;
  final TextEditingController model;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final draft = ref.watch(agentCreateFormProvider);
    final theme = Theme.of(context);
    final scheme = theme.colorScheme;
    final l10n = AppLocalizations.of(context);
    final accounts = ref.watch(providerAccountsProvider);
    final providerValue = draft.accountId.isNotEmpty ? draft.accountId : null;
    final form = ref.read(agentCreateFormProvider.notifier);

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        KimGroupCard(
          children: [
            ListTile(
              title: Text(l10n.agentDisplayName),
              subtitle: TextField(
                key: const Key('agent-name'),
                controller: displayName,
                decoration: InputDecoration(
                  border: InputBorder.none,
                  hintText: l10n.agentNameHint,
                ),
                onChanged: (_) => form.bump(),
              ),
            ),
          ],
        ),
        const Gap(18),
        AgentRuntimeSwitch(
          codex: draft.runtimeCodex,
          onChanged: form.setRuntimeCodex,
        ),
        Text(
          l10n.agentProvider,
          style: theme.textTheme.labelLarge?.copyWith(
            color: scheme.onSurfaceVariant,
          ),
        ),
        const Gap(8),
        if (accounts.isEmpty)
          EmptyState(
            icon: LucideIcons.key,
            title: l10n.agentEmptyProviders,
            subtitle: l10n.agentEmptyProvidersHint,
            action: FilledButton(
              key: const Key('agent-add-provider'),
              onPressed: () => unawaited(_openNewProvider(context, ref, model)),
              child: Text(l10n.agentAddAccount),
            ),
          )
        else
          KimGroupCard(
            children: [
              Padding(
                padding: const EdgeInsets.fromLTRB(16, 12, 16, 12),
                child: DropdownButton<String>(
                  key: const Key('agent-provider'),
                  value: () {
                    final items = agentCreateProviderItems(
                      ref: ref,
                      l10n: l10n,
                      draft: draft,
                    );
                    if (providerValue != null &&
                        items.any((i) => i.value == providerValue)) {
                      return providerValue;
                    }
                    return items.first.value;
                  }(),
                  isExpanded: true,
                  items: agentCreateProviderItems(
                    ref: ref,
                    l10n: l10n,
                    draft: draft,
                  ),
                  onChanged: (next) {
                    if (next == null) {
                      return;
                    }
                    _selectAccount(context, ref, model, next);
                  },
                ),
              ),
            ],
          ),
        if (draft.accountId.isNotEmpty) ...[
          const Gap(18),
          Text(
            Copy.agentModel,
            style: theme.textTheme.labelLarge?.copyWith(
              color: scheme.onSurfaceVariant,
            ),
          ),
          const Gap(8),
          KimGroupCard(
            children: [
              ListTile(
                title: Text(Copy.agentModel),
                subtitle: TextField(
                  key: const Key('agent-model'),
                  controller: model,
                  readOnly: true,
                  onTap: () => unawaited(_pickModel(context, ref, model)),
                  decoration: InputDecoration(
                    border: InputBorder.none,
                    hintText: l10n.agentModelOther,
                    suffixIcon: IconButton(
                      tooltip: l10n.agentPickModel,
                      onPressed: () =>
                          unawaited(_pickModel(context, ref, model)),
                      icon: const Icon(LucideIcons.chevronsUpDown, size: 18),
                    ),
                  ),
                ),
              ),
            ],
          ),
          const Gap(18),
          Text(
            l10n.agentContextWindow,
            style: theme.textTheme.labelLarge?.copyWith(
              color: scheme.onSurfaceVariant,
            ),
          ),
          const Gap(8),
          ContextWindowControls(
            tokens: draft.contextTokens,
            model: model.text.trim(),
            onChanged: form.setContextTokens,
          ),
          const Gap(18),
          Text(
            l10n.agentReasoning,
            style: theme.textTheme.labelLarge?.copyWith(
              color: scheme.onSurfaceVariant,
            ),
          ),
          const Gap(8),
          ReasoningControls(
            surface: draft.surface,
            choice: draft.choice,
            onChanged: form.setChoice,
          ),
        ],
      ],
    );
  }
}

Future<void> _openNewProvider(
  BuildContext context,
  WidgetRef ref,
  TextEditingController model,
) async {
  final id = await openProviderAccountEditor(context);
  if (!context.mounted || id == null || id.isEmpty) {
    return;
  }
  _selectAccount(context, ref, model, id);
}

void _selectAccount(
  BuildContext context,
  WidgetRef ref,
  TextEditingController model,
  String id,
) {
  if (id == kAgentCreateNewProvider) {
    unawaited(_openNewProvider(context, ref, model));
    return;
  }
  final account = ref.read(providerAccountsProvider.notifier).byId(id);
  if (account == null) {
    return;
  }
  final draft = ref.read(agentCreateFormProvider);
  final form = ref.read(agentCreateFormProvider.notifier);
  var nextModel = model.text.trim();
  var fell = false;
  if (account.models.isNotEmpty && !account.models.contains(nextModel)) {
    nextModel = defaultModelForAccount(
      account,
      agentCreateVendorById(account.vendorId, draft.vendors),
    );
    fell = true;
  }
  model.text = nextModel;
  form.bindAccount(id: id, contextTokens: defaultContextTokens(nextModel));
  if (fell && context.mounted) {
    agentCreateToastInfo(
      context,
      AppLocalizations.of(context).agentModelFallback(nextModel),
    );
  }
  unawaited(
    reloadAgentCreateSurface(
      ref: ref,
      model: model.text.trim(),
      mounted: () => context.mounted,
    ),
  );
}

List<String> _modelOptions(WidgetRef ref, TextEditingController model) {
  final draft = ref.read(agentCreateFormProvider);
  final account = agentCreateSelectedAccount(ref, draft.accountId);
  if (account != null) {
    return selectableModelIds([
      ...account.models,
      ...draft.pendingModels,
      model.text,
    ]);
  }
  return selectableModelIds([...draft.pendingModels, model.text]);
}

Future<void> _pickModel(
  BuildContext context,
  WidgetRef ref,
  TextEditingController model,
) async {
  final models = _modelOptions(ref, model);
  if (!context.mounted) {
    return;
  }
  final l10n = AppLocalizations.of(context);
  final selected = model.text.trim();
  final picked = await showModalBottomSheet<String>(
    context: context,
    showDragHandle: true,
    isScrollControlled: true,
    builder: (ctx) {
      final height = MediaQuery.sizeOf(ctx).height * 0.55;
      return SafeArea(
        child: SizedBox(
          height: height,
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Padding(
                padding: const EdgeInsets.fromLTRB(16, 4, 16, 8),
                child: Text(
                  l10n.agentModel,
                  style: Theme.of(ctx).textTheme.titleMedium,
                ),
              ),
              Expanded(
                child: ListView(
                  children: [
                    for (final id in models)
                      ListTile(
                        title: Text(id),
                        selected: id == selected,
                        onTap: () => Navigator.pop(ctx, id),
                      ),
                    ListTile(
                      title: Text(l10n.agentModelOther),
                      onTap: () => Navigator.pop(ctx, kAgentCreateNewProvider),
                    ),
                  ],
                ),
              ),
            ],
          ),
        ),
      );
    },
  );
  if (picked == null || !context.mounted) {
    return;
  }
  if (picked == kAgentCreateNewProvider) {
    await _otherModel(context, ref, model);
    return;
  }
  model.text = picked;
  ref
      .read(agentCreateFormProvider.notifier)
      .setContextTokens(defaultContextTokens(picked));
  unawaited(
    reloadAgentCreateSurface(
      ref: ref,
      model: model.text.trim(),
      mounted: () => context.mounted,
    ),
  );
}

Future<void> _otherModel(
  BuildContext context,
  WidgetRef ref,
  TextEditingController model,
) async {
  final l10n = AppLocalizations.of(context);
  final controller = TextEditingController();
  final raw = await showDialog<String>(
    context: context,
    builder: (ctx) {
      return AlertDialog(
        title: Text(l10n.agentModelOther),
        content: TextField(
          controller: controller,
          autofocus: true,
          decoration: InputDecoration(hintText: l10n.agentModel),
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(ctx),
            child: Text(Copy.cancel),
          ),
          FilledButton(
            onPressed: () => Navigator.pop(ctx, controller.text.trim()),
            child: Text(Copy.save),
          ),
        ],
      );
    },
  );
  controller.dispose();
  if (raw == null || !isSelectableModelId(raw) || !context.mounted) {
    return;
  }
  final draft = ref.read(agentCreateFormProvider);
  final form = ref.read(agentCreateFormProvider.notifier);
  final account = agentCreateSelectedAccount(ref, draft.accountId);
  if (account != null) {
    final models = selectableModelIds([...account.models, raw]);
    await ref
        .read(providerAccountsProvider.notifier)
        .upsert(account.copyWith(models: models));
    model.text = raw;
    form.setContextTokens(defaultContextTokens(raw));
  } else {
    model.text = raw;
    form.rememberCustomModel(
      pendingModels: selectableModelIds([...draft.pendingModels, raw]),
      contextTokens: defaultContextTokens(raw),
    );
  }
  unawaited(
    reloadAgentCreateSurface(
      ref: ref,
      model: model.text.trim(),
      mounted: () => context.mounted,
    ),
  );
}
