library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:toastification/toastification.dart';

import 'package:kim_mobile/bridge/goose_bridge.dart';
import 'package:kim_mobile/copy.dart';
import 'package:kim_mobile/features/agent/agent_create_form.dart';
import 'package:kim_mobile/features/agent/catalog.dart';
import 'package:kim_mobile/features/agent/host_support.dart';
import 'package:kim_mobile/features/agent/provider_accounts.dart';
import 'package:kim_mobile/features/agent/skills_catalog.dart';
import 'package:kim_mobile/features/agent/workspace_access.dart';

const kAgentCreateNewProvider = '__new__';
const kAgentCreateStepCount = 4;

void agentCreateToastInfo(BuildContext context, String message) {
  toastification.show(
    context: context,
    type: ToastificationType.info,
    title: Text(message),
    autoCloseDuration: const Duration(seconds: 3),
  );
}

void agentCreateToastError(BuildContext context, String message) {
  toastification.show(
    context: context,
    type: ToastificationType.error,
    title: Text(message),
    autoCloseDuration: const Duration(seconds: 4),
  );
}

VendorSummary? agentCreateVendorById(String id, List<VendorSummary> vendors) {
  for (final v in vendors) {
    if (v.id == id) {
      return v;
    }
  }
  return null;
}

ProviderAccount? agentCreateSelectedAccount(WidgetRef ref, String accountId) {
  if (accountId.isEmpty) {
    return null;
  }
  return ref.read(providerAccountsProvider.notifier).byId(accountId);
}

List<DropdownMenuItem<String>> agentCreateProviderItems({
  required WidgetRef ref,
  required AppLocalizations l10n,
  required AgentCreateDraft draft,
}) {
  final accounts = ref.watch(providerAccountsProvider);
  return [
    for (final a in accounts)
      DropdownMenuItem(
        value: a.id,
        child: Text(
          a.displayName.isNotEmpty
              ? a.displayName
              : (agentCreateVendorById(
                      a.vendorId,
                      draft.vendors,
                    )?.displayName ??
                    a.vendorId),
        ),
      ),
    DropdownMenuItem(
      value: kAgentCreateNewProvider,
      child: Text(l10n.agentNewProvider),
    ),
  ];
}

String agentCreateStepTitle(AppLocalizations l10n, int step) {
  return switch (step) {
    0 => l10n.agentCreateStepBasics,
    1 => l10n.agentCreateStepTools,
    2 => l10n.agentCreateStepSkills,
    _ => l10n.agentCreateStepPrompt,
  };
}

Future<void> reloadAgentCreateSurface({
  required WidgetRef ref,
  required String model,
  required bool Function() mounted,
}) async {
  final draft = ref.read(agentCreateFormProvider);
  final account = agentCreateSelectedAccount(ref, draft.accountId);
  final vendorId = account?.vendorId ?? '';
  if (vendorId.isEmpty) {
    return;
  }
  try {
    final surface = await ref
        .read(catalogRepositoryProvider)
        .surface(vendor: vendorId, model: model);
    if (!mounted()) {
      return;
    }
    final aligned = alignChoice(surface, draft.choice);
    ref
        .read(agentCreateFormProvider.notifier)
        .setSurface(surface: surface, choice: aligned.choice);
  } catch (_) {}
}

Future<void> reloadAgentCreateSkills({
  required WidgetRef ref,
  required bool Function() mounted,
}) async {
  if (!agentHostSupported) {
    return;
  }
  final draft = ref.read(agentCreateFormProvider);
  try {
    final bridge = ref.read(agentBridgeProvider);
    final app = await loadAppSkillCatalog(bridge: bridge);
    final userRoot = await workspaceAccess.realUserAgentsSkills() ?? '';
    final portable = await loadPortableSkills(
      bridge: bridge,
      userRoot: userRoot,
      projectRoot: draft.kindRepo ? draft.repoPath : '',
    );
    if (!mounted()) {
      return;
    }
    ref
        .read(agentCreateFormProvider.notifier)
        .replaceCatalog(app: app, portable: portable);
  } catch (_) {}
}
