library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';
import 'package:skeletonizer/skeletonizer.dart';

import '../../copy.dart';
import '../../core/haptics.dart';
import '../../core/layout.dart';
import '../../models/models.dart';
import '../../router/open_chat.dart';
import '../../state/chats_search.dart';
import '../../state/presence.dart';
import '../../state/contacts.dart';
import '../../state/link.dart';
import '../../state/inbox.dart';
import '../../state/profile.dart';
import '../../state/session.dart';
import '../../widgets/conversation_tile.dart';
import '../../widgets/empty_state.dart';
import '../../widgets/kim_avatar.dart';
import '../../widgets/kim_dock.dart';
import '../../widgets/new_chat_sheet.dart';
import '../../widgets/status_chip.dart';

class ChatsPage extends ConsumerStatefulWidget {
  const ChatsPage({super.key, this.selectedId, this.compact = false});

  final String? selectedId;
  final bool compact;

  @override
  ConsumerState<ChatsPage> createState() => _ChatsPageState();
}

class _ChatsPageState extends ConsumerState<ChatsPage> {
  var _searchOpen = false;
  final _searchFocus = FocusNode();

  @override
  void dispose() {
    _searchFocus.dispose();
    super.dispose();
  }

  void _toggleSearch() {
    setState(() => _searchOpen = !_searchOpen);
    if (_searchOpen) {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted) {
          _searchFocus.requestFocus();
        }
      });
    } else {
      ref.read(threadsProvider.notifier).setQuery('');
      _searchFocus.unfocus();
    }
  }

  @override
  Widget build(BuildContext context) {
    ref.listen(chatsSearchTickProvider, (prev, next) {
      if (widget.compact) {
        return;
      }
      if (prev != next && !_searchOpen) {
        _toggleSearch();
      }
    });
    final session = ref.watch(sessionProvider);
    final inbox = ref.watch(threadsProvider);
    final me = ref.watch(profileProvider);
    final social = ref.watch(contactsProvider);
    final visible = inbox.visible;
    final connecting =
        session.status == ConnStatus.connecting && inbox.threads.isEmpty;
    final theme = Theme.of(context);
    final scheme = theme.colorScheme;
    final chrome = scheme.surfaceContainerHigh;

    if (widget.compact) {
      return _compactScaffold(
        connecting: connecting,
        visible: visible,
        me: me,
        social: social,
        chrome: chrome,
      );
    }

    return Scaffold(
      body: CustomScrollView(
        slivers: [
          SliverAppBar(
            pinned: true,
            toolbarHeight: 56,
            titleSpacing: 16,
            title: Align(
              alignment: Alignment.centerLeft,
              child: GestureDetector(
                onTap: () => context.go('/me'),
                child: KimAvatar(
                  name: me.title.isEmpty ? Copy.me : me.title,
                  url: me.avatar,
                  size: KimAvatarSize.sm,
                  shape: KimAvatarShape.circle,
                ),
              ),
            ),
            actions: [
              _HeaderCircleButton(
                key: const Key('chats-search'),
                tooltip: Copy.searchChats,
                color: chrome,
                icon: LucideIcons.search,
                onTap: _toggleSearch,
              ),
              const SizedBox(width: 8),
              _HeaderCircleButton(
                key: const Key('compose-chat'),
                tooltip: Copy.newChat,
                color: chrome,
                icon: LucideIcons.plus,
                onTap: () => openNewChatSheet(context),
              ),
              const SizedBox(width: 12),
            ],
          ),
          if (_searchOpen)
            SliverToBoxAdapter(
              child: Padding(
                padding: const EdgeInsets.fromLTRB(16, 4, 16, 8),
                child: SearchBar(
                  focusNode: _searchFocus,
                  hintText: Copy.searchChats,
                  leading: Icon(
                    LucideIcons.search,
                    size: 18,
                    color: scheme.onSurfaceVariant,
                  ),
                  trailing: [
                    IconButton(
                      tooltip: Copy.cancel,
                      onPressed: _toggleSearch,
                      icon: const Icon(LucideIcons.x, size: 18),
                    ),
                  ],
                  onChanged: (v) =>
                      ref.read(threadsProvider.notifier).setQuery(v),
                ),
              ),
            ),
          SliverToBoxAdapter(
            child: ConnectionBanner(
              status: session.status,
              error: session.connectError,
              onRetry: () => ref.read(linkProvider.notifier).retry(),
            ),
          ),
          if (connecting)
            SliverFillRemaining(
              child: Skeletonizer(
                child: ListView.builder(
                  physics: const NeverScrollableScrollPhysics(),
                  padding: EdgeInsets.only(bottom: _listBottomPad(context)),
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
              child: Padding(
                padding: EdgeInsets.only(bottom: _listBottomPad(context)),
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
                          child: Text(Copy.newChat),
                        )
                      : null,
                ),
              ),
            )
          else
            SliverPadding(
              padding: EdgeInsets.only(bottom: _listBottomPad(context)),
              sliver: SliverList(
                delegate: SliverChildBuilderDelegate((context, i) {
                  final thread = visible[i];
                  final title = threadDisplayTitle(thread, social);
                  return ConversationTile(
                    thread: thread,
                    selected: thread.id == widget.selectedId,
                    avatarUrl: avatarFor(me, social, thread.id),
                    displayTitle: title,
                    presence: thread.kind == ThreadKind.user
                        ? ref.watch(peerPresenceProvider(thread.id))
                        : PeerPresenceStatus.unknown,
                    onOpen: () {
                      KimHaptics.selection();
                      openKimChat(
                        context,
                        ref,
                        id: thread.id,
                        kind: thread.kind,
                        title: title,
                      );
                    },
                    onDelete: () => ref
                        .read(threadsProvider.notifier)
                        .deleteThread(thread.id),
                  );
                }, childCount: visible.length),
              ),
            ),
        ],
      ),
    );
  }

  double _listBottomPad(BuildContext context) {
    if (widget.compact || kimLayoutSize(context) != KimLayoutSize.narrow) {
      return 24;
    }
    return KimDock.overlapOf(context);
  }

  Widget _compactScaffold({
    required bool connecting,
    required List<KimThread> visible,
    required ProfileState me,
    required ContactsState social,
    required Color chrome,
  }) {
    return Scaffold(
      body: Column(
        children: [
          SafeArea(
            bottom: false,
            child: Padding(
              padding: const EdgeInsets.fromLTRB(0, 8, 0, 4),
              child: Center(
                child: _HeaderCircleButton(
                  key: const Key('compose-chat'),
                  tooltip: Copy.newChat,
                  color: chrome,
                  icon: LucideIcons.plus,
                  onTap: () => openNewChatSheet(context),
                ),
              ),
            ),
          ),
          Expanded(child: _compactList(connecting, visible, me, social)),
        ],
      ),
    );
  }

  Widget _compactList(
    bool connecting,
    List<KimThread> visible,
    ProfileState me,
    ContactsState social,
  ) {
    if (connecting) {
      return Skeletonizer(
        child: ListView.builder(
          padding: const EdgeInsets.only(bottom: 16),
          itemCount: 7,
          itemBuilder: (context, i) => ConversationRailAvatar(
            thread: KimThread(
              id: 'skel-$i',
              kind: ThreadKind.user,
              title: 'skeleton',
            ),
            onOpen: () {},
          ),
        ),
      );
    }
    if (visible.isEmpty) {
      return const SizedBox.shrink();
    }
    return ListView.builder(
      padding: const EdgeInsets.only(bottom: 16),
      itemCount: visible.length,
      itemBuilder: (context, i) {
        final thread = visible[i];
        final title = threadDisplayTitle(thread, social);
        return ConversationRailAvatar(
          key: Key('rail-${thread.id}'),
          thread: thread,
          selected: thread.id == widget.selectedId,
          avatarUrl: avatarFor(me, social, thread.id),
          displayTitle: title,
          presence: thread.kind == ThreadKind.user
              ? ref.watch(peerPresenceProvider(thread.id))
              : PeerPresenceStatus.unknown,
          onOpen: () {
            KimHaptics.selection();
            openKimChat(
              context,
              ref,
              id: thread.id,
              kind: thread.kind,
              title: title,
            );
          },
        );
      },
    );
  }
}

class _HeaderCircleButton extends StatelessWidget {
  const _HeaderCircleButton({
    super.key,
    required this.color,
    required this.icon,
    required this.onTap,
    this.tooltip,
  });

  final Color color;
  final IconData icon;
  final VoidCallback onTap;
  final String? tooltip;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    final button = Material(
      color: color,
      shape: const CircleBorder(),
      clipBehavior: Clip.antiAlias,
      child: InkWell(
        customBorder: const CircleBorder(),
        onTap: onTap,
        child: SizedBox(
          width: 40,
          height: 40,
          child: Icon(icon, size: 20, color: scheme.onSurface),
        ),
      ),
    );
    if (tooltip == null) {
      return button;
    }
    return Tooltip(message: tooltip!, child: button);
  }
}
