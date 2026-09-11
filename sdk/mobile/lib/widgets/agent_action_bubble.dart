library;

import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:gap/gap.dart';

import '../models/models.dart';
import '../theme/kim_theme.dart';

class AgentActionBubble extends StatelessWidget {
  const AgentActionBubble({super.key, required this.message});

  final KimChatMsg message;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    final card = AgentToolCard.parse(message.body);
    final pending = card.state == 'pending' || card.state == 'running';
    return Padding(
      padding: const EdgeInsets.fromLTRB(16, 8, 16, 8),
      child: DecoratedBox(
        decoration: BoxDecoration(
          color: scheme.surfaceContainer,
          borderRadius: BorderRadius.circular(KimTheme.radiusCard),
          border: Border.all(color: scheme.outlineVariant.withValues(alpha: 0.7)),
        ),
        child: Padding(
          padding: const EdgeInsets.fromLTRB(12, 10, 12, 10),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Row(
                children: [
                  if (pending)
                    const SizedBox(
                      width: 14,
                      height: 14,
                      child: CircularProgressIndicator(strokeWidth: 2),
                    )
                  else
                    Icon(
                      card.ok ? Icons.check_circle_outline : Icons.error_outline,
                      size: 16,
                      color: card.ok ? scheme.primary : scheme.error,
                    ),
                  const Gap(8),
                  Expanded(
                    child: Text(
                      card.name.isEmpty ? 'tool' : card.name,
                      style: Theme.of(context).textTheme.labelLarge,
                    ),
                  ),
                ],
              ),
              if (card.preview.isNotEmpty) ...[
                const Gap(6),
                Text(
                  card.preview,
                  maxLines: 4,
                  overflow: TextOverflow.ellipsis,
                  style: Theme.of(context).textTheme.bodySmall?.copyWith(
                    color: scheme.onSurfaceVariant,
                  ),
                ),
              ],
            ],
          ),
        ),
      ),
    );
  }
}

class AgentToolCard {
  const AgentToolCard({
    required this.callId,
    required this.name,
    required this.state,
    required this.preview,
    required this.ok,
  });

  final String callId;
  final String name;
  final String state;
  final String preview;
  final bool ok;

  factory AgentToolCard.parse(String body) {
    try {
      final raw = jsonDecode(body);
      if (raw is Map) {
        final state = '${raw['state'] ?? 'pending'}';
        return AgentToolCard(
          callId: '${raw['call_id'] ?? ''}',
          name: '${raw['name'] ?? ''}',
          state: state,
          preview: '${raw['preview'] ?? ''}',
          ok: raw['ok'] == true || state == 'ok',
        );
      }
    } catch (_) {}
    return const AgentToolCard(
      callId: '',
      name: '',
      state: 'error',
      preview: '',
      ok: false,
    );
  }

  String encode() {
    return jsonEncode({
      'v': 1,
      'type': 'tool',
      'call_id': callId,
      'name': name,
      'state': state,
      'preview': preview,
      'ok': ok,
    });
  }
}
