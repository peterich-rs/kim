/// Model picker sheet + custom-model dialog for the agent editor.
library;

import 'dart:async';

import 'package:flutter/material.dart';

import 'package:kim_mobile/copy.dart';

const _kOtherModel = '__other__';

/// Returns the picked model id, a custom id, or null when dismissed.
Future<String?> showModelPickerSheet({
  required BuildContext context,
  required List<String> models,
  required String selected,
}) async {
  final l10n = AppLocalizations.of(context);
  return showModalBottomSheet<String>(
    context: context,
    showDragHandle: true,
    isScrollControlled: true,
    builder: (ctx) {
      final height = MediaQuery.sizeOf(ctx).height * 0.55;
      return SafeArea(
        child: SizedBox(
          height: height,
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Padding(
                padding: const EdgeInsets.fromLTRB(16, 4, 16, 8),
                child: Text(
                  l10n.agentModel,
                  style: Theme.of(ctx).textTheme.titleMedium,
                ),
              ),
              Expanded(
                child: ListView(
                  children: [
                    for (final id in models)
                      ListTile(
                        title: Text(id),
                        selected: id == selected,
                        onTap: () => Navigator.pop(ctx, id),
                      ),
                    ListTile(
                      title: Text(l10n.agentModelOther),
                      onTap: () => Navigator.pop(ctx, _kOtherModel),
                    ),
                  ],
                ),
              ),
            ],
          ),
        ),
      );
    },
  );
}

bool isOtherModelSentinel(String? picked) => picked == _kOtherModel;

/// Free-form model id entry. Returns the trimmed id or null.
Future<String?> showOtherModelDialog({
  required BuildContext context,
  required String hint,
}) async {
  final l10n = AppLocalizations.of(context);
  final controller = TextEditingController();
  final raw = await showDialog<String>(
    context: context,
    builder: (ctx) {
      return AlertDialog(
        title: Text(l10n.agentModelOther),
        content: TextField(
          controller: controller,
          autofocus: true,
          decoration: InputDecoration(hintText: hint),
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
  return raw;
}
