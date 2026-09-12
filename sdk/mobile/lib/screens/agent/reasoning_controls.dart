/// Surface-driven reasoning widgets. Switch on [ReasoningSurfaceDto.kind] only.
library;

import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../../agent/catalog.dart';
import '../../copy.dart';
import '../../state/agent_profiles.dart';
import '../../widgets/kim_group.dart';

class ReasoningControls extends StatelessWidget {
  const ReasoningControls({
    super.key,
    required this.surface,
    required this.choice,
    required this.onChanged,
    this.advancedController,
  });

  final ReasoningSurfaceDto surface;
  final ReasoningChoice choice;
  final ValueChanged<ReasoningChoice> onChanged;
  final TextEditingController? advancedController;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    return KimGroupCard(
      children: [
        ..._kindBody(context, l10n),
        if (advancedController != null) ...[
          if (surface.kind != 'none') const Divider(height: 1),
          _AdvancedJsonTile(
            controller: advancedController!,
            onChanged: onChanged,
          ),
        ],
      ],
    );
  }

  List<Widget> _kindBody(BuildContext context, AppLocalizations l10n) {
    switch (surface.kind) {
      case 'always_on':
        return [
          ListTile(
            title: Text(l10n.agentReasoningAlwaysOn),
            subtitle: Text(
              (surface.note == null || surface.note!.isEmpty)
                  ? l10n.agentReasoningAlwaysOn
                  : surface.note!,
            ),
          ),
        ];
      case 'toggle':
        return [
          SwitchListTile(
            title: Text(l10n.agentReasoning),
            value: choice.on ?? surface.defaultOn ?? false,
            onChanged: (on) =>
                onChanged(ReasoningChoice(kind: 'toggle', on: on)),
          ),
        ];
      case 'effort_enum':
        return [
          _EffortEnumControl(
            surface: surface,
            choice: choice,
            onChanged: onChanged,
          ),
        ];
      case 'budget_tokens':
        return [
          _BudgetTokensControl(
            surface: surface,
            choice: choice,
            onChanged: onChanged,
          ),
        ];
      default:
        if (advancedController != null) {
          return const [];
        }
        return [
          ListTile(
            title: Text(l10n.agentReasoning),
            subtitle: Text(l10n.agentReasoningNone),
          ),
        ];
    }
  }
}

class _EffortEnumControl extends StatelessWidget {
  const _EffortEnumControl({
    required this.surface,
    required this.choice,
    required this.onChanged,
  });

  final ReasoningSurfaceDto surface;
  final ReasoningChoice choice;
  final ValueChanged<ReasoningChoice> onChanged;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    final allowed = surface.allowed;
    if (allowed.isEmpty) {
      return const SizedBox.shrink();
    }
    final selected = allowed.contains(choice.value)
        ? choice.value!
        : allowed.first;
    if (allowed.length > 5) {
      return Padding(
        padding: const EdgeInsets.fromLTRB(16, 8, 16, 12),
        child: DropdownButton<String>(
          value: selected,
          isExpanded: true,
          items: [
            for (final v in allowed)
              DropdownMenuItem(value: v, child: Text(effortLabel(l10n, v))),
          ],
          onChanged: (next) {
            if (next == null) {
              return;
            }
            onChanged(ReasoningChoice(kind: 'effort_enum', value: next));
          },
        ),
      );
    }
    return Padding(
      padding: const EdgeInsets.fromLTRB(12, 12, 12, 12),
      child: SegmentedButton<String>(
        segments: [
          for (final v in allowed)
            ButtonSegment(value: v, label: Text(effortLabel(l10n, v))),
        ],
        selected: {selected},
        onSelectionChanged: (next) {
          onChanged(ReasoningChoice(kind: 'effort_enum', value: next.first));
        },
      ),
    );
  }
}

class _BudgetTokensControl extends StatefulWidget {
  const _BudgetTokensControl({
    required this.surface,
    required this.choice,
    required this.onChanged,
  });

  final ReasoningSurfaceDto surface;
  final ReasoningChoice choice;
  final ValueChanged<ReasoningChoice> onChanged;

  @override
  State<_BudgetTokensControl> createState() => _BudgetTokensControlState();
}

class _BudgetTokensControlState extends State<_BudgetTokensControl> {
  late final TextEditingController _controller;

  @override
  void initState() {
    super.initState();
    _controller = TextEditingController(
      text:
          '${widget.choice.budget ?? widget.surface.defaultBudget ?? widget.surface.min ?? 0}',
    );
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    return ListTile(
      title: Text(l10n.agentReasoning),
      subtitle: TextField(
        controller: _controller,
        keyboardType: TextInputType.number,
        inputFormatters: [FilteringTextInputFormatter.digitsOnly],
        decoration: InputDecoration(
          border: InputBorder.none,
          hintText: '${widget.surface.min ?? 0}–${widget.surface.max ?? 0}',
        ),
        onSubmitted: (raw) {
          final parsed = int.tryParse(raw.trim());
          if (parsed == null) {
            return;
          }
          widget.onChanged(
            ReasoningChoice(kind: 'budget_tokens', budget: parsed),
          );
        },
      ),
    );
  }
}

class _AdvancedJsonTile extends StatelessWidget {
  const _AdvancedJsonTile({required this.controller, required this.onChanged});

  final TextEditingController controller;
  final ValueChanged<ReasoningChoice> onChanged;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    return ExpansionTile(
      title: Text(l10n.agentAdvancedJson),
      children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(16, 0, 16, 12),
          child: TextField(
            controller: controller,
            maxLines: 4,
            decoration: const InputDecoration(
              border: InputBorder.none,
              hintText: '{ }',
            ),
            onChanged: (raw) {
              final trimmed = raw.trim();
              if (trimmed.isEmpty) {
                return;
              }
              try {
                final decoded = jsonDecode(trimmed);
                if (decoded is Map) {
                  onChanged(
                    ReasoningChoice(
                      kind: 'advanced',
                      advanced: Map<String, Object?>.from(decoded),
                    ),
                  );
                }
              } catch (_) {}
            },
          ),
        ),
      ],
    );
  }
}

String effortLabel(AppLocalizations l10n, String value) {
  return switch (value.toLowerCase()) {
    'none' || 'off' || 'disabled' => l10n.agentReasoningOff,
    'low' => l10n.agentReasoningLow,
    'medium' || 'med' => l10n.agentReasoningMedium,
    'high' => l10n.agentReasoningHigh,
    'max' => l10n.agentReasoningMax,
    'minimal' => l10n.agentReasoningMinimal,
    'xhigh' => l10n.agentReasoningXhigh,
    _ => value,
  };
}
