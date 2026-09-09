library;

import 'package:flutter/material.dart';

/// Window width where the shell uses a labeled rail and a full conversation list.
const double kKimWideBreakpoint = 900;

/// Window width where the split collapses to an avatar rail instead of stacking.
///
/// Below this, the shell uses bottom navigation and a single pane (list or chat).
const double kKimCompactBreakpoint = 600;

/// Readable max width for login / settings forms on desktop.
const double kKimFormMaxWidth = 480;

/// Conversation list pane in the wide split.
const double kKimMasterPaneWidth = 340;

/// Avatar-only conversation rail in the compact split.
const double kKimCompactPaneWidth = 80;

enum KimLayoutSize { narrow, compact, wide }

KimLayoutSize kimLayoutSizeForWidth(double width) => switch (width) {
  >= kKimWideBreakpoint => KimLayoutSize.wide,
  >= kKimCompactBreakpoint => KimLayoutSize.compact,
  _ => KimLayoutSize.narrow,
};

KimLayoutSize kimLayoutSize(BuildContext context) =>
    kimLayoutSizeForWidth(MediaQuery.sizeOf(context).width);

/// True when the shell keeps a side pane (full list or avatar rail) beside chat.
bool kimIsWide(BuildContext context) =>
    kimLayoutSize(context) != KimLayoutSize.narrow;

/// Center a form so it does not stretch across a desktop window.
Widget kimConstrainedForm({required Widget child}) {
  return Align(
    alignment: Alignment.topCenter,
    child: ConstrainedBox(
      constraints: const BoxConstraints(maxWidth: kKimFormMaxWidth),
      child: child,
    ),
  );
}
