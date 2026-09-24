library;

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:gap/gap.dart';
import 'package:toastification/toastification.dart';

import 'package:kim_mobile/copy.dart';
import 'package:kim_mobile/core/layout.dart';
import 'package:kim_mobile/design/kim_header.dart';
import 'package:kim_mobile/design/kim_pinned_footer.dart';
import 'package:kim_mobile/features/agent/agent_create_form.dart';
import 'package:kim_mobile/features/agent/agent_profiles.dart';
import 'package:kim_mobile/features/agent/catalog.dart';
import 'package:kim_mobile/features/agent/context_window.dart';
import 'package:kim_mobile/features/agent/create/agent_create_helpers.dart';
import 'package:kim_mobile/features/agent/create/agent_create_step_basics.dart';
import 'package:kim_mobile/features/agent/create/agent_create_step_review.dart';
import 'package:kim_mobile/features/agent/create/agent_create_step_skills.dart';
import 'package:kim_mobile/features/agent/create/agent_create_step_tools.dart';
import 'package:kim_mobile/features/agent/provider_accounts.dart';
import 'package:kim_mobile/features/agent/skills_catalog.dart';
import 'package:kim_mobile/features/agent/workspace_access.dart';

class AgentCreatePage extends ConsumerStatefulWidget {
  const AgentCreatePage({super.key});

  @override
  ConsumerState<AgentCreatePage> createState() => _AgentCreatePageState();
}

class _AgentCreatePageState extends ConsumerState<AgentCreatePage> {
  late final TextEditingController _displayName;
  late final TextEditingController _model;
  late final TextEditingController _prompt;
  late final TextEditingController _mcp;

  AgentCreateDraft get _state => ref.read(agentCreateFormProvider);
  AgentCreateForm get _form => ref.read(agentCreateFormProvider.notifier);

  @override
  void initState() {
    super.initState();
    _displayName = TextEditingController();
    _model = TextEditingController();
    _prompt = TextEditingController();
    _mcp = TextEditingController();
    unawaited(_bootstrap());
  }

  @override
  void dispose() {
    _displayName.dispose();
    _model.dispose();
    _prompt.dispose();
    _mcp.dispose();
    super.dispose();
  }

  Future<void> _bootstrap() async {
    await ref.read(agentProfilesProvider.notifier).ensureLoaded();
    await ref.read(providerAccountsProvider.notifier).ensureLoaded();
    if (!mounted) {
      return;
    }
    try {
      final vendors = await ref.read(catalogRepositoryProvider).ensureVendors();
      if (mounted) {
        _form.setVendors(vendors);
      }
    } catch (_) {}
    if (!mounted) {
      return;
    }
    final accounts = ref.read(providerAccountsProvider);
    if (accounts.isNotEmpty) {
      final model = defaultModelForAccount(
        accounts.first,
        agentCreateVendorById(
          accounts.first.vendorId,
          ref.read(agentCreateFormProvider).vendors,
        ),
      );
      _model.text = model;
      _form.seedAccount(
        accountId: accounts.first.id,
        contextTokens: defaultContextTokens(model),
      );
    }
    await reloadAgentCreateSurface(
      ref: ref,
      model: _model.text.trim(),
      mounted: () => mounted,
    );
    await reloadAgentCreateSkills(ref: ref, mounted: () => mounted);
  }

  bool _basicsReady() {
    final name = _displayName.text.trim();
    if (name.isEmpty) {
      return false;
    }
    final accountId = _state.accountId;
    return accountId.isNotEmpty &&
        ref.read(providerAccountsProvider.notifier).byId(accountId) != null;
  }

  Future<void> _next() async {
    final l10n = AppLocalizations.of(context);
    if (_state.step == 0 && !_basicsReady()) {
      agentCreateToastError(
        context,
        _displayName.text.trim().isEmpty
            ? l10n.agentNameHint
            : l10n.agentNeedProvider,
      );
      return;
    }
    if (_state.step >= kAgentCreateStepCount - 1) {
      await _finish();
      return;
    }
    _form.next();
    if (_state.step == 2) {
      await reloadAgentCreateSkills(ref: ref, mounted: () => mounted);
    }
  }

  void _back() {
    if (_state.step == 0) {
      return;
    }
    _form.back();
  }

  Future<void> _finish() async {
    if (_state.saving) {
      return;
    }
    final l10n = AppLocalizations.of(context);
    if (!_basicsReady()) {
      _form.setStep(0);
      agentCreateToastError(
        context,
        _displayName.text.trim().isEmpty
            ? l10n.agentNameHint
            : l10n.agentNeedProvider,
      );
      return;
    }
    final account = agentCreateSelectedAccount(ref, _state.accountId);
    if (account == null) {
      agentCreateToastError(context, l10n.agentNeedProvider);
      return;
    }
    _form.setSaving(true);
    var choice = _state.choice;
    try {
      final result = await ref
          .read(catalogRepositoryProvider)
          .validate(
            vendor: account.vendorId,
            model: _model.text.trim(),
            choice: choice,
          );
      choice = result.choice;
    } catch (_) {
      choice = alignChoice(_state.surface, choice).choice;
    }
    final model = _model.text.trim().isEmpty
        ? defaultModelForAccount(
            account,
            agentCreateVendorById(account.vendorId, _state.vendors),
          )
        : _model.text.trim();
    final store = ref.read(agentProfilesProvider.notifier);
    await store.ensureLoaded();
    final caps = mergeCapsWithMcpLines(_state.caps, _mcp.text);
    final skills = [
      for (final skill in _state.app)
        if (_state.appSelected.contains(skill.id)) appSkillRef(skill),
    ];
    final denylist = [
      for (final skill in _state.portable)
        if (!_state.portableSelected.contains(skill.id)) skill.id,
    ];
    final workspace = _state.kindRepo
        ? WorkspaceSpec(
            kind: WorkspaceSpec.kindRepo,
            path: _state.repoPath,
            bookmarkRef: _state.bookmark,
          )
        : WorkspaceSpec.sandbox;
    final next = store
        .draftNew(accountId: account.id, model: model)
        .copyWith(
          displayName: _displayName.text.trim(),
          systemPrompt: _prompt.text,
          reasoning: choice,
          thinkingEffort: choice.value ?? '',
          contextTokens: _state.contextTokens,
          workspace: workspace,
          skills: skills,
          portableDenylist: denylist,
          permissionOverrides: Map<String, String>.from(_state.perms),
          runtime: _state.runtimeCodex ? 'codex' : 'goose',
        )
        .withCapabilities(caps);
    try {
      await store.saveEditor(next);
      if (_state.bookmark.isNotEmpty) {
        await workspaceAccess.saveBookmark(next.id, _state.bookmark);
      }
    } catch (err) {
      _form.setSaving(false);
      if (mounted) {
        agentCreateToastError(context, err.toString());
      }
      return;
    }
    if (!mounted) {
      return;
    }
    toastification.show(
      context: context,
      type: ToastificationType.success,
      title: Text(Copy.agentSaved),
      autoCloseDuration: const Duration(seconds: 2),
    );
    await Navigator.of(context).maybePop();
  }

  @override
  Widget build(BuildContext context) {
    final step = ref.watch(agentCreateFormProvider.select((d) => d.step));
    final saving = ref.watch(agentCreateFormProvider.select((d) => d.saving));
    ref.watch(providerAccountsProvider);
    final theme = Theme.of(context);
    final scheme = theme.colorScheme;
    final l10n = AppLocalizations.of(context);
    final last = step >= kAgentCreateStepCount - 1;
    return Scaffold(
      bottomNavigationBar: KimPinnedFooter(
        child: Row(
          children: [
            if (step > 0) ...[
              OutlinedButton(
                key: const Key('agent-back'),
                onPressed: saving ? null : _back,
                child: Text(l10n.back),
              ),
              const Gap(12),
            ],
            Expanded(
              child: FilledButton(
                key: Key(last ? 'agent-save' : 'agent-next'),
                onPressed: saving ? null : () => unawaited(_next()),
                child: Text(
                  last ? l10n.agentCreateFinish : l10n.agentCreateNext,
                ),
              ),
            ),
          ],
        ),
      ),
      body: CustomScrollView(
        slivers: [
          KimSliverHeader(title: l10n.agentCreate),
          KimBodySliver(
            sliver: SliverList.list(
              children: [
                Text(
                  '${l10n.agentCreateProgress(step + 1, kAgentCreateStepCount)}  ${agentCreateStepTitle(l10n, step)}',
                  style: theme.textTheme.labelLarge?.copyWith(
                    color: scheme.onSurfaceVariant,
                  ),
                ),
                const Gap(16),
                switch (step) {
                  0 => AgentCreateStepBasics(
                    displayName: _displayName,
                    model: _model,
                  ),
                  1 => AgentCreateStepTools(mcp: _mcp),
                  2 => AgentCreateStepSkills(
                    displayName: _displayName,
                    model: _model,
                    prompt: _prompt,
                    mcp: _mcp,
                  ),
                  _ => AgentCreateStepReview(prompt: _prompt),
                },
              ],
            ),
          ),
        ],
      ),
    );
  }
}
