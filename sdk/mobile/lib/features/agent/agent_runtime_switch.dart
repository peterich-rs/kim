import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:gap/gap.dart';

import 'package:kim_mobile/design/kim_group.dart';
import 'package:kim_mobile/features/agent/agent_profiles.dart';
import 'package:kim_mobile/features/agent/host_support.dart';

/// Profile-scoped runtime picker. Default stays Goose. The next open uses the
/// selected runtime; an in-flight session is not migrated.
class AgentRuntimeSwitch extends ConsumerWidget {
  const AgentRuntimeSwitch({
    super.key,
    required this.codex,
    required this.onChanged,
  });

  final bool codex;
  final ValueChanged<bool>? onChanged;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    if (!agentHostSupported) {
      return const SizedBox.shrink();
    }
    final zh = Localizations.localeOf(context).languageCode.startsWith('zh');
    final theme = Theme.of(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Text(
          zh ? '运行时' : 'Runtime',
          style: theme.textTheme.titleSmall?.copyWith(
            color: theme.colorScheme.onSurfaceVariant,
          ),
        ),
        const Gap(8),
        KimGroupCard(
          children: [
            SwitchListTile(
              key: const Key('agent-runtime-codex'),
              title: Text(zh ? '用 Codex 跑这个助手' : 'Run this assistant on Codex'),
              subtitle: Text(
                zh ? '默认仍是 Goose。创建时选定即可；改完从下次打开会话生效，旧会话不会迁过去。' : 'Goose stays the default. Set at create time; applies on the next session open. Existing sessions are not migrated.',
              ),
              value: codex,
              onChanged: onChanged,
            ),
          ],
        ),
        const Gap(18),
      ],
    );
  }
}

/// Capabilities-page wrapper: reads/writes [AgentProfile.runtime].
class AgentProfileRuntimeSwitch extends ConsumerStatefulWidget {
  const AgentProfileRuntimeSwitch({super.key, required this.profileId});

  final String profileId;

  @override
  ConsumerState<AgentProfileRuntimeSwitch> createState() =>
      _AgentProfileRuntimeSwitchState();
}

class _AgentProfileRuntimeSwitchState
    extends ConsumerState<AgentProfileRuntimeSwitch> {
  Future<void> _set(AgentProfile profile, bool codex) async {
    final busy = runtimeSwitchBusyProvider(widget.profileId);
    if (ref.read(busy)) {
      return;
    }
    ref.read(busy.notifier).setBusy(true);
    try {
      final next = profile.copyWith(runtime: codex ? 'codex' : 'goose');
      await ref.read(agentProfilesProvider.notifier).saveEditor(next);
    } finally {
      if (mounted) {
        ref.read(busy.notifier).setBusy(false);
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    final busy = ref.watch(runtimeSwitchBusyProvider(widget.profileId));
    AgentProfile? profile;
    for (final p in ref.watch(agentProfilesProvider)) {
      if (p.id == widget.profileId) {
        profile = p;
        break;
      }
    }
    if (profile == null) {
      return const SizedBox.shrink();
    }
    final current = profile;
    return AgentRuntimeSwitch(
      codex: current.usesCodex,
      onChanged: busy ? null : (value) => unawaited(_set(current, value)),
    );
  }
}

class RuntimeSwitchBusy extends Notifier<bool> {
  RuntimeSwitchBusy(this.profileId);

  final String profileId;

  @override
  bool build() => false;

  void setBusy(bool value) => state = value;
}

final runtimeSwitchBusyProvider = NotifierProvider.autoDispose
    .family<RuntimeSwitchBusy, bool, String>(RuntimeSwitchBusy.new);
