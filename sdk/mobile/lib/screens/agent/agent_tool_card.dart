library;

import 'package:flutter/material.dart';
import 'package:gap/gap.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import '../../copy.dart';
import '../../state/agent.dart';
import '../../theme/kim_theme.dart';

class AgentToolCardView extends StatefulWidget {
  const AgentToolCardView({super.key, required this.card, this.onDark = false});

  final AgentToolCard card;
  final bool onDark;

  @override
  State<AgentToolCardView> createState() => _AgentToolCardViewState();
}

class _AgentToolCardViewState extends State<AgentToolCardView> {
  var _open = false;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final card = widget.card;
    final statusLabel = switch (card.status) {
      'ok' => Copy.agentToolOk,
      'fail' => Copy.agentToolFail,
      _ => Copy.agentToolRunning,
    };
    final statusIcon = switch (card.status) {
      'ok' => LucideIcons.check,
      'fail' => LucideIcons.x,
      _ => LucideIcons.loaderCircle,
    };
    final fg = widget.onDark ? Colors.white : theme.colorScheme.onSurface;
    return DecoratedBox(
      decoration: BoxDecoration(
        color: (widget.onDark ? Colors.black : theme.colorScheme.surface)
            .withValues(alpha: widget.onDark ? 0.2 : 0.7),
        borderRadius: BorderRadius.circular(KimTheme.radiusControl),
        border: Border.all(color: fg.withValues(alpha: 0.2)),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          InkWell(
            onTap: () => setState(() => _open = !_open),
            borderRadius: BorderRadius.circular(KimTheme.radiusControl),
            child: Padding(
              padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 8),
              child: Row(
                children: [
                  Icon(statusIcon, size: 14, color: fg),
                  const Gap(8),
                  Expanded(
                    child: Text(
                      card.name,
                      style: theme.textTheme.labelLarge?.copyWith(
                        color: fg,
                        fontWeight: FontWeight.w600,
                      ),
                    ),
                  ),
                  Text(
                    statusLabel,
                    style: theme.textTheme.labelSmall?.copyWith(
                      color: fg.withValues(alpha: 0.8),
                      fontSize: KimTheme.fontMeta,
                    ),
                  ),
                  Icon(
                    _open ? LucideIcons.chevronUp : LucideIcons.chevronDown,
                    size: 14,
                    color: fg,
                  ),
                ],
              ),
            ),
          ),
          if (_open)
            Padding(
              padding: const EdgeInsets.fromLTRB(10, 0, 10, 10),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  if (card.args.isNotEmpty) ...[
                    Text(
                      'args',
                      style: theme.textTheme.labelSmall?.copyWith(color: fg),
                    ),
                    Text(
                      card.args,
                      style: theme.textTheme.bodySmall?.copyWith(
                        color: fg.withValues(alpha: 0.9),
                        fontFamily: 'monospace',
                      ),
                    ),
                  ],
                  if (card.result.isNotEmpty) ...[
                    const Gap(6),
                    Text(
                      'result',
                      style: theme.textTheme.labelSmall?.copyWith(color: fg),
                    ),
                    Text(
                      card.result,
                      style: theme.textTheme.bodySmall?.copyWith(
                        color: fg.withValues(alpha: 0.9),
                        fontFamily: 'monospace',
                      ),
                    ),
                  ],
                ],
              ),
            ),
        ],
      ),
    );
  }
}
