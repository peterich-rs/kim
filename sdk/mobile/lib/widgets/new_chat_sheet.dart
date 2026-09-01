library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:gap/gap.dart';
import 'package:go_router/go_router.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';
import 'package:wolt_modal_sheet/wolt_modal_sheet.dart';

import '../copy.dart';
import '../core/haptics.dart';
import '../models/models.dart';
import '../state/contacts.dart';
import '../state/inbox.dart';
import '../widgets/empty_state.dart';
import '../widgets/kim_avatar.dart';

Future<void> openNewChatSheet(BuildContext context) {
  return WoltModalSheet.show<void>(
    context: context,
    showDragHandle: true,
    pageListBuilder: (context) => [
      WoltModalSheetPage(
        hasSabGradient: false,
        navBarHeight: 56,
        pageTitle: const Padding(
          padding: EdgeInsets.fromLTRB(24, 8, 24, 0),
          child: Text(Copy.newChat),
        ),
        child: const Padding(
          padding: EdgeInsets.fromLTRB(8, 0, 8, 28),
          child: _NewChatBody(),
        ),
      ),
    ],
  );
}

class _NewChatBody extends ConsumerWidget {
  const _NewChatBody();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final friends = ref.watch(contactsProvider).friends;
    if (friends.isEmpty) {
      return EmptyState(
        icon: LucideIcons.users,
        title: Copy.noFriends,
        subtitle: Copy.noFriendsHint,
        action: FilledButton.tonal(
          onPressed: () {
            Navigator.of(context).pop();
            context.go('/contacts');
          },
          child: const Text(Copy.addFriend),
        ),
      );
    }
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        for (final p in friends)
          ListTile(
            leading: KimAvatar(
              name: p.title,
              url: p.avatar,
              size: KimAvatarSize.sm,
            ),
            title: Text(p.title),
            subtitle: Text('@${p.account}'),
            onTap: () {
              final thread = ref
                  .read(threadsProvider.notifier)
                  .ensureThread(
                    id: p.account,
                    kind: ThreadKind.user,
                    title: p.title,
                  );
              KimHaptics.selection();
              Navigator.of(context).pop();
              context.push('/chat/${thread.id}', extra: thread);
            },
          ),
        const Gap(8),
      ],
    );
  }
}
