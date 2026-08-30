library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:gap/gap.dart';
import 'package:go_router/go_router.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import '../../copy.dart';
import '../../core/haptics.dart';
import '../../core/validation.dart';
import '../../models/models.dart';
import '../../state/inbox.dart';
import '../../state/session.dart';
import '../../widgets/empty_state.dart';
import '../../widgets/kim_avatar.dart';
import '../../widgets/kim_text_field.dart';

class ContactsPage extends ConsumerStatefulWidget {
  const ContactsPage({super.key});

  @override
  ConsumerState<ContactsPage> createState() => _ContactsPageState();
}

class _ContactsPageState extends ConsumerState<ContactsPage> {
  late final TextEditingController _query;
  String? _error;

  @override
  void initState() {
    super.initState();
    _query = TextEditingController();
  }

  @override
  void dispose() {
    _query.dispose();
    super.dispose();
  }

  void _start() {
    final dest = _query.text.trim();
    final me = ref.read(sessionProvider).account;
    final accountErr = validateAccount(dest);
    setState(() {
      if (accountErr != null) {
        _error = accountErr;
      } else if (dest == me) {
        _error = Copy.cannotChatSelf;
      } else {
        _error = null;
      }
    });
    if (_error != null) {
      return;
    }
    final thread = ref.read(inboxProvider.notifier).ensureThread(
      id: dest,
      kind: ThreadKind.user,
      title: dest,
    );
    KimHaptics.selection();
    context.push('/chat/${thread.id}', extra: thread);
  }

  @override
  Widget build(BuildContext context) {
    final threads = ref.watch(inboxProvider).threads;
    final theme = Theme.of(context);

    return Scaffold(
      body: CustomScrollView(
        slivers: [
          const SliverAppBar.large(title: Text(Copy.contacts)),
          SliverToBoxAdapter(
            child: Padding(
              padding: const EdgeInsets.fromLTRB(16, 0, 16, 8),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  Text(
                    Copy.addByAccount,
                    style: theme.textTheme.titleSmall?.copyWith(
                      color: theme.colorScheme.onSurfaceVariant,
                    ),
                  ),
                  KimTextField(
                    controller: _query,
                    label: Copy.peerAccount,
                    hintText: Copy.peerPlaceholder,
                    errorText: _error,
                    maxLength: 32,
                    prefixIcon: LucideIcons.search,
                    autocorrect: false,
                    enableSuggestions: false,
                    textInputAction: TextInputAction.done,
                    onEditingComplete: _start,
                  ),
                  const Gap(12),
                  FilledButton.tonal(
                    onPressed: _start,
                    child: const Text(Copy.startChat),
                  ),
                ],
              ),
            ),
          ),
          if (threads.isEmpty)
            const SliverFillRemaining(
              hasScrollBody: false,
              child: EmptyState(
                icon: LucideIcons.users,
                title: Copy.noFriends,
                subtitle: Copy.noFriendsHint,
              ),
            )
          else ...[
            SliverToBoxAdapter(
              child: Padding(
                padding: const EdgeInsets.fromLTRB(16, 20, 16, 8),
                child: Text(
                  Copy.recentContacts,
                  style: theme.textTheme.titleSmall?.copyWith(
                    color: theme.colorScheme.onSurfaceVariant,
                  ),
                ),
              ),
            ),
            SliverList.separated(
              itemCount: threads.length,
              separatorBuilder: (context, index) => const Divider(indent: 80),
              itemBuilder: (context, i) {
                final t = threads[i];
                return ListTile(
                  contentPadding: const EdgeInsets.symmetric(
                    horizontal: 16,
                    vertical: 4,
                  ),
                  leading: KimAvatar(name: t.title, size: KimAvatarSize.sm),
                  title: Text(t.title),
                  subtitle: Text(
                    t.kind == ThreadKind.group
                        ? Copy.privateChat
                        : Copy.privateChat,
                  ),
                  onTap: () {
                    KimHaptics.selection();
                    context.push('/chat/${t.id}', extra: t);
                  },
                );
              },
            ),
          ],
        ],
      ),
    );
  }
}
