library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:gap/gap.dart';
import 'package:go_router/go_router.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import '../../copy.dart';
import '../../core/haptics.dart';
import '../../state/agent.dart';
import '../../theme/kim_theme.dart';
import '../../widgets/empty_state.dart';
import 'agent_composer.dart';
import 'agent_tool_card.dart';

class AgentPage extends ConsumerStatefulWidget {
  const AgentPage({super.key});

  @override
  ConsumerState<AgentPage> createState() => _AgentPageState();
}

class _AgentPageState extends ConsumerState<AgentPage> {
  @override
  Widget build(BuildContext context) {
    final ref = this.ref;
    final agent = ref.watch(agentProvider);
    final wide = MediaQuery.sizeOf(context).width >= 900;
    final theme = Theme.of(context);
    final scheme = theme.colorScheme;

    return Scaffold(
      backgroundColor: KimTheme.canvasOf(context),
      body: Row(
        children: [
          if (wide)
            SizedBox(
              width: 280,
              child: _SessionRail(
                sessions: agent.sessions,
                activeId: agent.activeId,
                onNew: () => ref.read(agentProvider.notifier).newSession(),
                onSelect: (id) =>
                    ref.read(agentProvider.notifier).openSession(id),
              ),
            ),
          if (wide)
            VerticalDivider(
              width: 1,
              thickness: 1,
              color: KimTheme.hairlineOf(context),
            ),
          Expanded(
            child: Column(
              children: [
                _AgentAppBar(
                  title:
                      agent.sessions
                          .where((s) => s.id == agent.activeId)
                          .map((s) => s.title)
                          .firstOrNull ??
                      Copy.agent,
                  showSessions: !wide,
                  onNew: () => ref.read(agentProvider.notifier).newSession(),
                  onSettings: () => context.push('/agent/settings'),
                ),
                if (agent.error != null)
                  Material(
                    color: scheme.errorContainer,
                    child: Padding(
                      padding: const EdgeInsets.symmetric(
                        horizontal: 16,
                        vertical: 8,
                      ),
                      child: Row(
                        children: [
                          Icon(
                            LucideIcons.circleAlert,
                            size: 16,
                            color: scheme.onErrorContainer,
                          ),
                          const Gap(8),
                          Expanded(
                            child: Text(
                              agent.error!,
                              style: theme.textTheme.bodySmall?.copyWith(
                                color: scheme.onErrorContainer,
                              ),
                            ),
                          ),
                        ],
                      ),
                    ),
                  ),
                Expanded(
                  child: agent.items.isEmpty
                      ? EmptyState(
                          icon: LucideIcons.bot,
                          title: Copy.agentEmptyTitle,
                          subtitle: Copy.agentEmptyHint,
                          action: TextButton.icon(
                            onPressed: () =>
                                ref.read(agentProvider.notifier).newSession(),
                            icon: const Icon(LucideIcons.plus),
                            label: const Text(Copy.agentNewSession),
                          ),
                        )
                      : ListView.builder(
                          padding: const EdgeInsets.fromLTRB(16, 12, 16, 24),
                          itemCount: agent.items.length,
                          itemBuilder: (context, i) {
                            final item = agent.items[i];
                            return _Bubble(item: item);
                          },
                        ),
                ),
                AgentComposer(
                  busy: agent.busy,
                  onSend: (t) => ref.read(agentProvider.notifier).send(t),
                  onStop: () => ref.read(agentProvider.notifier).abort(),
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}

extension<T> on Iterable<T> {
  T? get firstOrNull {
    final it = iterator;
    if (!it.moveNext()) {
      return null;
    }
    return it.current;
  }
}

class _AgentAppBar extends StatelessWidget {
  const _AgentAppBar({
    required this.title,
    required this.showSessions,
    required this.onNew,
    required this.onSettings,
  });

  final String title;
  final bool showSessions;
  final VoidCallback onNew;
  final VoidCallback onSettings;

  @override
  Widget build(BuildContext context) {
    return Material(
      color: KimTheme.chromeOf(context),
      child: SafeArea(
        bottom: false,
        child: SizedBox(
          height: 56,
          child: Row(
            children: [
              const Gap(16),
              Expanded(
                child: Text(
                  title,
                  style: Theme.of(context).textTheme.titleMedium?.copyWith(
                    fontWeight: FontWeight.w700,
                    fontSize: KimTheme.fontTitle,
                  ),
                ),
              ),
              if (showSessions)
                IconButton(
                  tooltip: Copy.agentNewSession,
                  onPressed: () {
                    KimHaptics.selection();
                    onNew();
                  },
                  icon: const Icon(LucideIcons.plus),
                ),
              IconButton(
                tooltip: Copy.agentSettings,
                onPressed: () {
                  KimHaptics.selection();
                  onSettings();
                },
                icon: const Icon(LucideIcons.settings),
              ),
              const Gap(4),
            ],
          ),
        ),
      ),
    );
  }
}

class _SessionRail extends StatelessWidget {
  const _SessionRail({
    required this.sessions,
    required this.activeId,
    required this.onNew,
    required this.onSelect,
  });

  final List<AgentSessionMeta> sessions;
  final String? activeId;
  final VoidCallback onNew;
  final ValueChanged<String> onSelect;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final scheme = theme.colorScheme;
    return Material(
      color: KimTheme.chromeOf(context),
      child: SafeArea(
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Padding(
              padding: const EdgeInsets.fromLTRB(16, 16, 12, 8),
              child: Row(
                children: [
                  Expanded(
                    child: Text(
                      Copy.agentSessions,
                      style: theme.textTheme.titleMedium?.copyWith(
                        fontWeight: FontWeight.w700,
                      ),
                    ),
                  ),
                  IconButton(
                    tooltip: Copy.agentNewSession,
                    onPressed: onNew,
                    icon: const Icon(LucideIcons.plus),
                  ),
                ],
              ),
            ),
            Expanded(
              child: ListView.builder(
                itemCount: sessions.length,
                itemBuilder: (context, i) {
                  final s = sessions[i];
                  final selected = s.id == activeId;
                  return ListTile(
                    selected: selected,
                    selectedTileColor: scheme.primary.withValues(alpha: 0.08),
                    title: Text(
                      s.title,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: theme.textTheme.bodyMedium?.copyWith(
                        fontWeight: selected
                            ? FontWeight.w600
                            : FontWeight.w500,
                      ),
                    ),
                    subtitle: Text(
                      s.updatedAt.toLocal().toString().substring(0, 16),
                      style: theme.textTheme.labelSmall?.copyWith(
                        color: scheme.onSurfaceVariant,
                        fontSize: KimTheme.fontMeta,
                      ),
                    ),
                    onTap: () {
                      KimHaptics.selection();
                      onSelect(s.id);
                    },
                  );
                },
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _Bubble extends StatelessWidget {
  const _Bubble({required this.item});

  final AgentTranscriptItem item;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final scheme = theme.colorScheme;
    final mine = item.role == 'user';
    final bg = mine ? KimTheme.outgoing : KimTheme.raisedOf(context);
    final fg = mine ? Colors.white : scheme.onSurface;
    return Align(
      alignment: mine ? Alignment.centerRight : Alignment.centerLeft,
      child: ConstrainedBox(
        constraints: BoxConstraints(
          maxWidth: MediaQuery.sizeOf(context).width * (mine ? 0.72 : 0.85),
        ),
        child: Container(
          margin: const EdgeInsets.symmetric(vertical: 6),
          padding: const EdgeInsets.fromLTRB(14, 10, 14, 10),
          decoration: BoxDecoration(
            color: bg,
            borderRadius: BorderRadius.only(
              topLeft: const Radius.circular(KimTheme.radiusBubble),
              topRight: const Radius.circular(KimTheme.radiusBubble),
              bottomLeft: Radius.circular(
                mine ? KimTheme.radiusBubble : KimTheme.radiusBubbleTail,
              ),
              bottomRight: Radius.circular(
                mine ? KimTheme.radiusBubbleTail : KimTheme.radiusBubble,
              ),
            ),
            border: mine
                ? null
                : Border.all(color: KimTheme.hairlineOf(context)),
          ),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              if (item.text.isNotEmpty)
                Text(
                  item.text,
                  style: theme.textTheme.bodyMedium?.copyWith(
                    color: fg,
                    fontSize: KimTheme.fontBody,
                    height: 1.35,
                  ),
                ),
              for (final tool in item.tools) ...[
                const Gap(8),
                AgentToolCardView(card: tool, onDark: mine),
              ],
              if (item.streaming && item.text.isEmpty && item.tools.isEmpty)
                Text(
                  '…',
                  style: theme.textTheme.bodyMedium?.copyWith(color: fg),
                ),
            ],
          ),
        ),
      ),
    );
  }
}
