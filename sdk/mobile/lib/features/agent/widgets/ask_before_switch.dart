library;

import 'package:flutter/material.dart';

import 'package:kim_mobile/copy.dart';

class AskBeforeSwitch extends StatelessWidget {
  const AskBeforeSwitch({
    super.key,
    required this.tool,
    required this.value,
    required this.onChanged,
  });

  final String tool;
  final bool value;
  final ValueChanged<bool> onChanged;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    return SwitchListTile(
      key: Key('agent-ask-$tool'),
      title: Text(l10n.agentAskBefore),
      subtitle: Text(l10n.agentAskBeforeHint),
      value: value,
      onChanged: onChanged,
    );
  }
}
