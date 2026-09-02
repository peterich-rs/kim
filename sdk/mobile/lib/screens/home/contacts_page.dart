library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:gap/gap.dart';
import 'package:go_router/go_router.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';
import 'package:toastification/toastification.dart';

import '../../copy.dart';
import '../../core/haptics.dart';
import '../../models/models.dart';
import '../../state/contacts.dart';
import '../../state/inbox.dart';
import '../../state/mutations.dart';
import '../../widgets/empty_state.dart';
import '../../widgets/kim_avatar.dart';
import '../../widgets/kim_group.dart';
import '../../widgets/kim_header.dart';
import '../../widgets/kim_text_field.dart';

class ContactsPage extends ConsumerStatefulWidget {
  const ContactsPage({super.key});

  @override
  ConsumerState<ContactsPage> createState() => _ContactsPageState();
}

class _ContactsPageState extends ConsumerState<ContactsPage> {
  late final TextEditingController _query;
  var _searching = false;

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

  void _toast(String message, {bool success = false}) {
    if (!mounted) {
      return;
    }
    toastification.show(
      context: context,
      type: success ? ToastificationType.success : ToastificationType.error,
      style: ToastificationStyle.flatColored,
      title: Text(message),
      autoCloseDuration: const Duration(seconds: 3),
      alignment: Alignment.topCenter,
    );
  }

  Future<void> _search() async {
    setState(() => _searching = true);
    try {
      await searchPeopleMutation.run(ref, (tsx) {
        return tsx.get(contactsProvider.notifier).search(_query.text);
      });
    } catch (err) {
      _toast(socialError(err));
    } finally {
      if (mounted) {
        setState(() => _searching = false);
      }
    }
  }

  Future<void> _request(String dest) async {
    try {
      await friendRequestMutation(dest).run(ref, (tsx) {
        return tsx.get(contactsProvider.notifier).request(dest);
      });
      final social = ref.read(contactsProvider);
      _toast(
        social.isFriend(dest) ? Copy.friendAccepted : Copy.requestSent,
        success: true,
      );
      if (social.isFriend(dest) && mounted) {
        _open(dest, social.person(dest)?.title ?? dest);
      }
    } catch (err) {
      _toast(socialError(err));
    }
  }

  Future<void> _accept(KimPerson person) async {
    try {
      await friendAcceptMutation(person.account).run(ref, (tsx) {
        return tsx.get(contactsProvider.notifier).accept(person.account);
      });
      _toast(Copy.friendAccepted, success: true);
      if (mounted) {
        _open(person.account, person.title);
      }
    } catch (err) {
      _toast(socialError(err));
    }
  }

  void _open(String id, String title) {
    final thread = ref
        .read(threadsProvider.notifier)
        .ensureThread(id: id, kind: ThreadKind.user, title: title);
    KimHaptics.selection();
    context.push('/chat/${thread.id}', extra: thread);
  }

  @override
  Widget build(BuildContext context) {
    final social = ref.watch(contactsProvider);
    final theme = Theme.of(context);
    final hits = social.hits;
    final queried = social.query.isNotEmpty;

    return Scaffold(
      body: RefreshIndicator(
        onRefresh: () => ref.read(contactsProvider.notifier).refresh(),
        child: CustomScrollView(
          slivers: [
            const KimSliverHeader(title: Copy.contacts),
            SliverToBoxAdapter(
              child: Padding(
                padding: const EdgeInsets.fromLTRB(16, 0, 16, 8),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
                    KimTextField(
                      controller: _query,
                      label: Copy.addByAccount,
                      hintText: Copy.searchPeople,
                      maxLength: 32,
                      prefixIcon: LucideIcons.search,
                      autocorrect: false,
                      enableSuggestions: false,
                      textInputAction: TextInputAction.search,
                      onEditingComplete: _search,
                    ),
                    const Gap(8),
                    FilledButton.tonal(
                      onPressed: _searching ? null : _search,
                      child: const Text(Copy.addFriend),
                    ),
                  ],
                ),
              ),
            ),
            if (queried) ...[
              _sectionLabel(theme, Copy.searchPeople),
              if (hits.isEmpty)
                const SliverToBoxAdapter(
                  child: Padding(
                    padding: EdgeInsets.symmetric(vertical: 24),
                    child: Center(child: Text(Copy.searchEmpty)),
                  ),
                )
              else
                _groupSliver([
                  for (var i = 0; i < hits.length; i++) ...[
                    _HitTile(
                      person: hits[i],
                      friend: social.isFriend(hits[i].account),
                      pending: social.isOutgoing(hits[i].account),
                      onChat: () => _open(hits[i].account, hits[i].title),
                      onAdd: () => _request(hits[i].account),
                    ),
                    if (i != hits.length - 1) const Divider(indent: 72),
                  ],
                ]),
            ],
            if (social.incoming.isNotEmpty) ...[
              _sectionLabel(
                theme,
                '${Copy.incoming} (${social.incomingCount})',
              ),
              _groupSliver([
                for (var i = 0; i < social.incoming.length; i++) ...[
                  ListTile(
                    leading: KimAvatar(
                      name: social.incoming[i].title,
                      url: social.incoming[i].avatar,
                    ),
                    title: Text(social.incoming[i].title),
                    subtitle: Text('@${social.incoming[i].account}'),
                    trailing: Row(
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        TextButton(
                          onPressed: () => ref
                              .read(contactsProvider.notifier)
                              .reject(social.incoming[i].account),
                          child: const Text(Copy.reject),
                        ),
                        FilledButton(
                          onPressed: () => _accept(social.incoming[i]),
                          child: const Text(Copy.accept),
                        ),
                      ],
                    ),
                  ),
                  if (i != social.incoming.length - 1)
                    const Divider(indent: 72),
                ],
              ]),
            ],
            _sectionLabel(theme, Copy.recentContacts),
            if (social.friends.isEmpty)
              const SliverToBoxAdapter(
                child: Padding(
                  padding: EdgeInsets.only(top: 24),
                  child: EmptyState(
                    icon: LucideIcons.users,
                    title: Copy.noFriends,
                    subtitle: Copy.noFriendsHint,
                  ),
                ),
              )
            else
              _groupSliver([
                for (var i = 0; i < social.friends.length; i++) ...[
                  ListTile(
                    leading: KimAvatar(
                      name: social.friends[i].title,
                      url: social.friends[i].avatar,
                    ),
                    title: Text(social.friends[i].title),
                    subtitle: Text('@${social.friends[i].account}'),
                    trailing: const Text(Copy.chatAction),
                    onTap: () => _open(
                      social.friends[i].account,
                      social.friends[i].title,
                    ),
                  ),
                  if (i != social.friends.length - 1) const Divider(indent: 72),
                ],
              ]),
            const SliverToBoxAdapter(child: Gap(24)),
          ],
        ),
      ),
    );
  }
}

SliverToBoxAdapter _sectionLabel(ThemeData theme, String text) {
  return SliverToBoxAdapter(
    child: Padding(
      padding: const EdgeInsets.fromLTRB(20, 20, 16, 8),
      child: Text(text, style: theme.textTheme.titleSmall),
    ),
  );
}

SliverToBoxAdapter _groupSliver(List<Widget> children) {
  return SliverToBoxAdapter(
    child: Padding(
      padding: const EdgeInsets.fromLTRB(16, 0, 16, 8),
      child: KimGroupCard(children: children),
    ),
  );
}

class _HitTile extends StatelessWidget {
  const _HitTile({
    required this.person,
    required this.friend,
    required this.pending,
    required this.onChat,
    required this.onAdd,
  });

  final KimPerson person;
  final bool friend;
  final bool pending;
  final VoidCallback onChat;
  final VoidCallback onAdd;

  @override
  Widget build(BuildContext context) {
    return ListTile(
      leading: KimAvatar(name: person.title, url: person.avatar),
      title: Text(person.title),
      subtitle: Text('@${person.account}'),
      trailing: friend
          ? TextButton(onPressed: onChat, child: const Text(Copy.chatAction))
          : pending
          ? const Text(Copy.requested)
          : FilledButton.tonal(
              onPressed: onAdd,
              child: const Text(Copy.addFriend),
            ),
    );
  }
}
