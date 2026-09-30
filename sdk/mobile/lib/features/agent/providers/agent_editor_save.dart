/// Save path for the agent editor page: validate, align reasoning, persist,
/// toast, pop-on-create.
library;

import 'package:flutter/material.dart';
import 'package:toastification/toastification.dart';

import 'package:kim_mobile/copy.dart';
import 'package:kim_mobile/features/agent/data/catalog.dart';
import 'package:kim_mobile/features/agent/providers/agent_profiles.dart';
import 'package:kim_mobile/features/agent/providers/provider_accounts.dart';
import 'package:kim_mobile/features/agent/providers/agent_settings_form.dart';

class AgentEditorSave {
  AgentEditorSave({
    required this.context,
    required this.form,
    required this.ensureStores,
    required this.draftNew,
    required this.saveEditor,
    required this.validateChoice,
    required this.toastError,
    required this.toastInfo,
    required this.displayName,
    required this.aliases,
    required this.prompt,
    required this.model,
  });

  final BuildContext context;
  final AgentSettingsForm form;
  final Future<void> Function() ensureStores;
  final AgentProfile Function({
    required String accountId,
    required String model,
  })
  draftNew;
  final Future<void> Function(AgentProfile profile) saveEditor;
  final Future<CatalogValidateResult?> Function({
    required String vendor,
    required String model,
    required ReasoningChoice choice,
  })
  validateChoice;
  final void Function(String message) toastError;
  final void Function(String message) toastInfo;
  final TextEditingController displayName;
  final TextEditingController aliases;
  final TextEditingController prompt;
  final TextEditingController model;

  /// Returns true when the page should pop (created).
  Future<bool> save({
    required bool isCreate,
    required AgentProfile? existing,
    required String accountId,
    required ProviderAccount? account,
    required VendorSummary? Function(String vendorId) vendorById,
    required int contextTokens,
    required ReasoningChoice choice,
    required ReasoningSurface surface,
  }) async {
    final l10n = AppLocalizations.of(context);
    final name = displayName.text.trim();
    if (name.isEmpty) {
      toastError(l10n.agentNameHint);
      return false;
    }
    if (account == null) {
      toastError(l10n.agentNeedProvider);
      return false;
    }
    var nextChoice = choice;
    final result = await validateChoice(
      vendor: account.vendorId,
      model: model.text.trim(),
      choice: nextChoice,
    );
    if (result != null) {
      nextChoice = result.choice;
      if (result.dropped.isNotEmpty) {
        toastInfo(Copy.agentReasoningDropped);
      }
    } else {
      final aligned = alignChoice(surface, nextChoice);
      nextChoice = aligned.choice;
      if (aligned.dropped) {
        toastInfo(Copy.agentReasoningDropped);
      }
    }
    final resolvedModel = model.text.trim().isEmpty
        ? defaultModelForAccount(account, vendorById(account.vendorId))
        : model.text.trim();
    await ensureStores();
    final AgentProfile next;
    final creating = isCreate && existing == null;
    if (creating) {
      final draft = draftNew(accountId: account.id, model: resolvedModel);
      next = draft.copyWith(
        displayName: name,
        systemPrompt: prompt.text,
        reasoning: nextChoice,
        thinkingEffort: nextChoice.value ?? '',
        contextTokens: contextTokens,
      );
      form.setProfile(next);
    } else {
      if (existing == null) {
        return false;
      }
      final aliasList = [
        for (final part in aliases.text.split(RegExp(r'[,，\s]+')))
          if (part.trim().isNotEmpty) part.trim(),
      ];
      next = existing.copyWith(
        displayName: name,
        aliases: aliasList,
        accountId: account.id,
        model: resolvedModel,
        thinkingEffort: nextChoice.value ?? '',
        reasoning: nextChoice,
        contextTokens: contextTokens,
        systemPrompt: prompt.text,
      );
    }
    await saveEditor(next);
    if (!context.mounted) {
      return false;
    }
    form.setChoice(nextChoice);
    toastification.show(
      context: context,
      type: ToastificationType.success,
      title: Text(Copy.agentSaved),
      autoCloseDuration: const Duration(seconds: 2),
    );
    return creating;
  }
}
