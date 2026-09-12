library;

import 'package:flutter/material.dart';
import 'package:flutter_slidable/flutter_slidable.dart';
import 'package:gap/gap.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import '../copy.dart';
import '../core/format.dart';
import '../core/image_extra.dart';
import '../models/models.dart';
import '../theme/kim_theme.dart';
import 'kim_avatar.dart';
import 'status_chip.dart';

class ConversationTile extends StatelessWidget {
  const ConversationTile({
    super.key,
    required this.thread,
    required this.onOpen,
    required this.onDelete,
    this.avatarUrl = '',
    this.presence = PeerPresenceStatus.unknown,
    this.selected = false,
    this.displayTitle = '',
  });

  final KimThread thread;
  final VoidCallback onOpen;
  final VoidCallback onDelete;
  final String avatarUrl;
  final PeerPresenceStatus presence;
  final bool selected;
  final String displayTitle;

  String get _title => displayTitle.isEmpty ? thread.title : displayTitle;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final scheme = theme.colorScheme;
    final snippet = previewSnippet(thread.lastBody);
    final preview = snippet.isEmpty ? Copy.noMessages : snippet;
    final time = formatListTime(thread.lastAt);

    return Slidable(
      key: ValueKey(thread.id),
      endActionPane: ActionPane(
        motion: const DrawerMotion(),
        extentRatio: 0.28,
        children: [
          SlidableAction(
            onPressed: (_) => onDelete(),
            backgroundColor: scheme.error,
            foregroundColor: scheme.onError,
            icon: LucideIcons.trash2,
            label: Copy.delete,
          ),
        ],
      ),
      child: InkWell(
        onTap: onOpen,
        child: ColoredBox(
          color: selected
              ? scheme.primaryContainer.withValues(alpha: 0.45)
              : Colors.transparent,
          child: Padding(
            padding: const EdgeInsets.fromLTRB(16, 10, 16, 10),
            child: Row(
              crossAxisAlignment: CrossAxisAlignment.center,
              children: [
                Stack(
                  clipBehavior: Clip.none,
                  children: [
                    KimAvatar(
                      name: _title,
                      url: avatarUrl,
                      size: KimAvatarSize.md,
                      shape: KimAvatarShape.squircle,
                    ),
                    if (presence != PeerPresenceStatus.unknown)
                      Positioned(
                        right: -1,
                        bottom: -1,
                        child: PeerPresenceDot(status: presence, size: 10),
                      ),
                  ],
                ),
                const Gap(14),
                Expanded(
                  child: Column(
                    mainAxisAlignment: MainAxisAlignment.center,
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Row(
                        children: [
                          Expanded(
                            child: Text(
                              _title,
                              maxLines: 1,
                              overflow: TextOverflow.ellipsis,
                              style: theme.textTheme.titleMedium?.copyWith(
                                fontWeight: thread.unread > 0
                                    ? FontWeight.w700
                                    : FontWeight.w600,
                                fontSize: KimTheme.fontTitle,
                                color: scheme.onSurface,
                              ),
                            ),
                          ),
                          if (time.isNotEmpty)
                            Text(
                              time,
                              style: theme.textTheme.labelSmall?.copyWith(
                                color: thread.unread > 0
                                    ? scheme.primary
                                    : scheme.onSurfaceVariant,
                                fontSize: KimTheme.fontMeta,
                              ),
                            ),
                        ],
                      ),
                      const Gap(4),
                      Row(
                        children: [
                          Expanded(
                            child: Text(
                              preview,
                              maxLines: 1,
                              overflow: TextOverflow.ellipsis,
                              style: theme.textTheme.bodyMedium?.copyWith(
                                color: thread.unread > 0
                                    ? scheme.onSurface.withValues(alpha: 0.82)
                                    : scheme.onSurfaceVariant,
                                fontSize: KimTheme.fontBody,
                                fontWeight: thread.unread > 0
                                    ? FontWeight.w500
                                    : FontWeight.w400,
                              ),
                            ),
                          ),
                          if (thread.unread > 0) ...[
                            const Gap(8),
                            Container(
                              constraints: const BoxConstraints(
                                minWidth: 20,
                                minHeight: 20,
                              ),
                              padding: const EdgeInsets.symmetric(
                                horizontal: 7,
                                vertical: 2,
                              ),
                              decoration: BoxDecoration(
                                color: scheme.primary,
                                borderRadius: BorderRadius.circular(10),
                              ),
                              child: Text(
                                thread.unread > 99 ? '99+' : '${thread.unread}',
                                textAlign: TextAlign.center,
                                style: TextStyle(
                                  color: scheme.onPrimary,
                                  fontSize: 11,
                                  fontWeight: FontWeight.w700,
                                  height: 1.2,
                                ),
                              ),
                            ),
                          ],
                        ],
                      ),
                    ],
                  ),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

/// Avatar-only conversation row for the compact master pane.
class ConversationRailAvatar extends StatelessWidget {
  const ConversationRailAvatar({
    super.key,
    required this.thread,
    required this.onOpen,
    this.avatarUrl = '',
    this.presence = PeerPresenceStatus.unknown,
    this.selected = false,
    this.displayTitle = '',
  });

  final KimThread thread;
  final VoidCallback onOpen;
  final String avatarUrl;
  final PeerPresenceStatus presence;
  final bool selected;
  final String displayTitle;

  String get _title => displayTitle.isEmpty ? thread.title : displayTitle;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    return Tooltip(
      message: _title,
      child: Semantics(
        button: true,
        selected: selected,
        label: _title,
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
          child: Material(
            color: selected
                ? scheme.primaryContainer.withValues(alpha: 0.45)
                : Colors.transparent,
            borderRadius: BorderRadius.circular(16),
            clipBehavior: Clip.antiAlias,
            child: InkWell(
              onTap: onOpen,
              customBorder: RoundedRectangleBorder(
                borderRadius: BorderRadius.circular(16),
              ),
              child: Padding(
                padding: const EdgeInsets.all(8),
                child: Stack(
                  clipBehavior: Clip.none,
                  children: [
                    KimAvatar(
                      name: _title,
                      url: avatarUrl,
                      size: KimAvatarSize.md,
                      shape: KimAvatarShape.squircle,
                    ),
                    if (presence != PeerPresenceStatus.unknown)
                      Positioned(
                        right: -1,
                        bottom: -1,
                        child: PeerPresenceDot(status: presence, size: 10),
                      ),
                    if (thread.unread > 0)
                      Positioned(
                        right: -4,
                        top: -4,
                        child: Container(
                          constraints: const BoxConstraints(
                            minWidth: 16,
                            minHeight: 16,
                          ),
                          padding: const EdgeInsets.symmetric(horizontal: 4),
                          decoration: BoxDecoration(
                            color: scheme.primary,
                            borderRadius: BorderRadius.circular(8),
                          ),
                          alignment: Alignment.center,
                          child: Text(
                            thread.unread > 99 ? '99+' : '${thread.unread}',
                            textAlign: TextAlign.center,
                            style: TextStyle(
                              color: scheme.onPrimary,
                              fontSize: 9,
                              fontWeight: FontWeight.w700,
                              height: 1.1,
                            ),
                          ),
                        ),
                      ),
                  ],
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}
