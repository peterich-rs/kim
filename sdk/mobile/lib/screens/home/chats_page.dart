library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';
import 'package:skeletonizer/skeletonizer.dart';

import '../../copy.dart';
import '../../core/haptics.dart';
import '../../models/models.dart';
import '../../state/gateway.dart';
import '../../state/inbox.dart';
import '../../state/session.dart';
import '../../widgets/conversation_tile.dart';
import '../../widgets/empty_state.dart';
import '../../widgets/kim_header.dart';
import '../../widgets/new_chat_sheet.dart';
import '../../widgets/status_chip.dart';

class ChatsPage extends ConsumerWidget {
  const ChatsPage({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final session = ref.watch(sessionProvider);
    final inbox = ref.watch(inboxProvider);
    final visible = inbox.visible;
    final connecting =
        session.status == ConnStatus.connecting && inbox.threads.isEmpty;
    final theme = Theme.of(context);

    return Scaffold(
      body: CustomScrollView(
        slivers: [
          KimSliverHeader(
            title: Copy.conversations,
            actions: [
              IconButton(
                key: const Key('compose-chat'),
                tooltip: Copy.newChat,
                onPressed: () => openNewChatSheet(context),
                icon: const Icon(LucideIcons.edit3),
              ),
            ],
          ),
          SliverToBoxAdapter(
            child: Padding(
              padding: const EdgeInsets.fromLTRB(16, 4, 16, 8),
              child: SearchBar(
                hintText: Copy.searchChats,
                leading: Icon(
                  LucideIcons.search,
                  size: 18,
                  color: theme.colorScheme.onSurfaceVariant,
                ),
                elevation: const WidgetStatePropertyAll(0),
                onChanged: (v) => ref.read(inboxProvider.notifier).setQuery(v),
              ),
            ),
          ),
          SliverToBoxAdapter(
            child: ConnectionBanner(
              status: session.status,
              error: session.connectError,
              onRetry: () => ref.invalidate(gatewayProvider),
            ),
          ),
          if (connecting)
            SliverFillRemaining(
              child: Skeletonizer(
                child: ListView.builder(
                  physics: const NeverScrollableScrollPhysics(),
                  itemCount: 7,
                  itemBuilder: (context, i) => ConversationTile(
                    thread: KimThread(
                      id: 'skel-$i',
                      kind: ThreadKind.user,
                      title: 'skeleton',
                      lastBody: 'placeholder message body',
                      lastAt: DateTime.now().millisecondsSinceEpoch,
                    ),
                    onOpen: () {},
                    onDelete: () {},
                  ),
                ),
              ),
            )
          else if (visible.isEmpty)
            SliverFillRemaining(
              child: EmptyState(
                icon: LucideIcons.messageCircle,
                title: inbox.threads.isEmpty
                    ? Copy.noConversations
                    : Copy.noMatch,
                subtitle: inbox.threads.isEmpty
                    ? Copy.noConversationsHint
                    : Copy.searchChats,
                action: inbox.threads.isEmpty
                    ? FilledButton.tonal(
                        onPressed: () => openNewChatSheet(context),
                        child: const Text(Copy.newChat),
                      )
                    : null,
              ),
            )
          else
            SliverPadding(
              padding: const EdgeInsets.only(bottom: 24),
              sliver: SliverList.separated(
                itemCount: visible.length,
                separatorBuilder: (context, _) =>
                    Divider(indent: 80, color: theme.dividerColor),
                itemBuilder: (context, i) {
                  final thread = visible[i];
                  return ConversationTile(
                    index: i,
                    thread: thread,
                    onOpen: () {
                      KimHaptics.selection();
                      ref
                          .read(inboxProvider.notifier)
                          .ensureThread(
                            id: thread.id,
                            kind: thread.kind,
                            title: thread.title,
                          );
                      context.push('/chat/${thread.id}', extra: thread);
                    },
                    onDelete: () => ref
                        .read(inboxProvider.notifier)
                        .deleteThread(thread.id),
                  );
                },
              ),
            ),
        ],
      ),
    );
  }
}
