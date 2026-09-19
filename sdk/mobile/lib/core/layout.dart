library;

import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:go_router/go_router.dart';

/// Window width where the shell uses a labeled rail and a full conversation list.
const double kKimWideBreakpoint = 900;

/// Window width where the split collapses to an avatar rail instead of stacking.
///
/// Below this, the shell uses bottom navigation and a single pane (list or chat).
const double kKimCompactBreakpoint = 600;

/// Readable max width for login / settings forms on desktop.
const double kKimFormMaxWidth = 420;

/// Settings / editor pages on a wide pane.
const double kKimPageMaxWidth = 640;

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

/// Desktop pointer can drag-select message text. Phones keep long-press copy.
bool kimSelectsMessageText([TargetPlatform? platform]) {
  return switch (platform ?? defaultTargetPlatform) {
    TargetPlatform.macOS ||
    TargetPlatform.windows ||
    TargetPlatform.linux => true,
    TargetPlatform.iOS ||
    TargetPlatform.android ||
    TargetPlatform.fuchsia => false,
  };
}

/// Horizontal inset that keeps [maxWidth] content centered in [width].
double kimBodyInset(
  double width, {
  double maxWidth = kKimPageMaxWidth,
  double minInset = 16,
}) {
  if (width <= maxWidth + minInset * 2) {
    return minInset;
  }
  return (width - maxWidth) / 2;
}

/// Overlay routes sit on top of the tab shell and must always be leavable.
bool kimIsOverlayPath(String path) {
  return path.startsWith('/agent') ||
      path.startsWith('/password') ||
      path.startsWith('/dev') ||
      path.startsWith('/peer');
}

/// Whether a page header should show a back control.
bool kimShowsBack(BuildContext context) {
  final router = GoRouter.maybeOf(context);
  if (router != null) {
    if (router.canPop()) {
      return true;
    }
    return kimIsOverlayPath(GoRouterState.of(context).uri.path);
  }
  return Navigator.of(context).canPop();
}

/// Pop the overlay, or return to the conversation tab if the stack was wiped.
void kimLeaveOverlay(BuildContext context) {
  final router = GoRouter.maybeOf(context);
  if (router != null) {
    if (router.canPop()) {
      router.pop();
      return;
    }
    router.go('/');
    return;
  }
  Navigator.of(context).maybePop();
}

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

/// Pads a sliver so its content caps at [maxWidth] on a wide pane.
class KimBodySliver extends StatelessWidget {
  const KimBodySliver({
    super.key,
    required this.sliver,
    this.maxWidth = kKimPageMaxWidth,
    this.bottom = 24,
  });

  final Widget sliver;
  final double maxWidth;
  final double bottom;

  @override
  Widget build(BuildContext context) {
    return SliverLayoutBuilder(
      builder: (context, constraints) {
        final inset = kimBodyInset(
          constraints.crossAxisExtent,
          maxWidth: maxWidth,
        );
        return SliverPadding(
          padding: EdgeInsets.fromLTRB(inset, 0, inset, bottom),
          sliver: sliver,
        );
      },
    );
  }
}
