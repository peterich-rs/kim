/// Host-tool preview card at the top of the capabilities sheet.
library;

import 'package:flutter/material.dart';

import 'package:kim_mobile/copy.dart';
import 'package:kim_mobile/design/kim_group.dart';

class CapabilitiesPreviewCard extends StatelessWidget {
  const CapabilitiesPreviewCard({
    super.key,
    required this.summary,
    required this.fromHost,
  });

  final String summary;
  final bool fromHost;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    return KimGroupCard(
      children: [
        ListTile(
          key: const Key('agent-capabilities-preview'),
          title: Text(l10n.agentCapabilitiesPreview),
          subtitle: Text(
            summary.isEmpty
                ? l10n.agentCapabilitiesPreviewEmpty
                : (fromHost
                      ? summary
                      : l10n.agentCapabilitiesPreviewLocal(summary)),
          ),
        ),
      ],
    );
  }
}
