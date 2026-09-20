/// Context-window picker. Defaults come from [defaultContextTokens].
library;

import 'package:flutter/material.dart';

import 'package:kim_mobile/copy.dart';
import 'package:kim_mobile/design/kim_group.dart';
import 'package:kim_mobile/features/agent/context_window.dart';

const _kCustom = -1;

class ContextWindowControls extends StatelessWidget {
  const ContextWindowControls({
    super.key,
    required this.tokens,
    required this.model,
    required this.onChanged,
  });

  final int tokens;
  final String model;
  final ValueChanged<int> onChanged;

  int get _modelDefault => defaultContextTokens(model);

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    final options = contextTokenOptions(
      modelDefault: _modelDefault,
      current: tokens,
    );
    final selected = options.contains(tokens) ? tokens : _modelDefault;
    return KimGroupCard(
      children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(16, 12, 16, 12),
          child: DropdownButton<int>(
            key: const Key('agent-context'),
            value: selected,
            isExpanded: true,
            items: [
              for (final n in options)
                DropdownMenuItem(
                  value: n,
                  child: Text(
                    n == _modelDefault
                        ? l10n.agentContextWindowDefault(formatContextTokens(n))
                        : formatContextTokens(n),
                  ),
                ),
              DropdownMenuItem(
                value: _kCustom,
                child: Text(l10n.agentContextWindowCustom),
              ),
            ],
            onChanged: (next) {
              if (next == null) {
                return;
              }
              if (next == _kCustom) {
                _custom(context);
                return;
              }
              onChanged(next);
            },
          ),
        ),
      ],
    );
  }

  Future<void> _custom(BuildContext context) async {
    final l10n = AppLocalizations.of(context);
    final controller = TextEditingController(text: '$tokens');
    final raw = await showDialog<String>(
      context: context,
      builder: (ctx) {
        return AlertDialog(
          title: Text(l10n.agentContextWindowCustom),
          content: TextField(
            controller: controller,
            autofocus: true,
            keyboardType: TextInputType.text,
            decoration: InputDecoration(
              hintText: formatContextTokens(_modelDefault),
              helperText: l10n.agentContextWindowHint,
            ),
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
    final parsed = raw == null ? null : parseContextTokens(raw);
    if (parsed == null) {
      return;
    }
    onChanged(parsed);
  }
}
