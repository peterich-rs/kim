library;

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:gap/gap.dart';

import 'package:kim_mobile/features/agent/providers/agent_permission.dart';
import 'package:kim_mobile/copy.dart';
import 'package:kim_mobile/design/kim_theme.dart';

class AgentActionBubble extends ConsumerWidget {
  const AgentActionBubble({
    super.key,
    required this.dest,
    required this.callId,
    required this.name,
    required this.preview,
    this.pending = true,
    this.confirmation = true,
  });

  final String dest;
  final String callId;
  final String name;
  final String preview;
  final bool pending;
  final bool confirmation;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final scheme = Theme.of(context).colorScheme;
    return Padding(
      padding: const EdgeInsets.fromLTRB(16, 8, 16, 8),
      child: DecoratedBox(
        decoration: BoxDecoration(
          color: scheme.surfaceContainer,
          borderRadius: BorderRadius.circular(KimTheme.radiusCard),
          border: Border.all(
            color: scheme.outlineVariant.withValues(alpha: 0.7),
          ),
        ),
        child: Padding(
          padding: const EdgeInsets.fromLTRB(12, 10, 12, 10),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Row(
                children: [
                  if (pending && !confirmation)
                    const SizedBox(
                      width: 14,
                      height: 14,
                      child: CircularProgressIndicator(strokeWidth: 2),
                    )
                  else
                    Icon(
                      confirmation
                          ? Icons.shield_outlined
                          : Icons.check_circle_outline,
                      size: 16,
                      color: scheme.primary,
                    ),
                  const Gap(8),
                  Expanded(
                    child: Text(
                      name.isEmpty ? 'tool' : name,
                      style: Theme.of(context).textTheme.labelLarge,
                    ),
                  ),
                ],
              ),
              if (preview.isNotEmpty) ...[
                const Gap(6),
                Text(
                  preview,
                  maxLines: 4,
                  overflow: TextOverflow.ellipsis,
                  style: Theme.of(context).textTheme.bodySmall
                      ?.copyWith(color: scheme.onSurfaceVariant),
                ),
              ],
              if (confirmation) ...[
                const Gap(10),
                Row(
                  children: [
                    TextButton(
                      onPressed: pending
                          ? () => unawaited(
                              ref
                                  .read(agentPermissionHubProvider.notifier)
                                  .respond(
                                    dest: dest,
                                    callId: callId,
                                    permission: 'allow_once',
                                  ),
                            )
                          : null,
                      child: Text(Copy.agentAllow),
                    ),
                    TextButton(
                      onPressed: pending
                          ? () => unawaited(
                              ref
                                  .read(agentPermissionHubProvider.notifier)
                                  .respond(
                                    dest: dest,
                                    callId: callId,
                                    permission: 'always_allow',
                                  ),
                            )
                          : null,
                      child: Text(Copy.agentAlwaysAllow),
                    ),
                    TextButton(
                      onPressed: pending
                          ? () => unawaited(
                              ref
                                  .read(agentPermissionHubProvider.notifier)
                                  .respond(
                                    dest: dest,
                                    callId: callId,
                                    permission: 'deny_once',
                                  ),
                            )
                          : null,
                      child: Text(Copy.agentDeny),
                    ),
                  ],
                ),
              ],
            ],
          ),
        ),
      ),
    );
  }
}
