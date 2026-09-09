library;

import 'package:flutter/material.dart';
import 'package:go_router/go_router.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import '../../copy.dart';
import '../../core/layout.dart';
import '../../theme/kim_theme.dart';
import '../../widgets/empty_state.dart';
import '../chat/chat_page.dart';
import 'chats_page.dart';

class ChatsSplitView extends StatelessWidget {
  const ChatsSplitView({super.key, this.selectedId});

  final String? selectedId;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    final layout = kimLayoutSize(context);
    if (layout == KimLayoutSize.narrow) {
      return selectedId == null ? const ChatsPage() : ChatPage(id: selectedId!);
    }
    final compact = layout == KimLayoutSize.compact;
    return ColoredBox(
      color: KimTheme.canvasOf(context),
      child: Row(
        children: [
          AnimatedSize(
            duration: KimTheme.motionBase,
            curve: KimTheme.motionEmphasized,
            alignment: Alignment.topLeft,
            child: SizedBox(
              width: compact ? kKimCompactPaneWidth : kKimMasterPaneWidth,
              child: ChatsPage(selectedId: selectedId, compact: compact),
            ),
          ),
          VerticalDivider(
            width: 1,
            thickness: 1,
            color: KimTheme.hairlineOf(context),
          ),
          Expanded(
            child: selectedId == null
                ? EmptyState(
                    icon: LucideIcons.messageCircle,
                    title: l10n.emptyChatTitle,
                    subtitle: l10n.emptyChatSubtitle,
                  )
                : ChatPage(id: selectedId!),
          ),
        ],
      ),
    );
  }
}

class ChatRouteHost extends StatelessWidget {
  const ChatRouteHost({super.key, required this.id});

  final String id;

  @override
  Widget build(BuildContext context) => ChatsSplitView(selectedId: id);
}

class KimErrorPage extends StatelessWidget {
  const KimErrorPage({super.key, this.error});

  final Object? error;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    return Scaffold(
      body: EmptyState(
        icon: LucideIcons.circleAlert,
        title: l10n.routeNotFound,
        subtitle: error?.toString() ?? '',
        action: FilledButton.tonal(
          onPressed: () => context.go('/'),
          child: Text(l10n.goHome),
        ),
      ),
    );
  }
}
