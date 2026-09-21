library;

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:gap/gap.dart';
import 'package:go_router/go_router.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';
import 'package:toastification/toastification.dart';

import 'package:kim_mobile/features/agent/mention.dart';
import 'package:kim_mobile/copy.dart';
import 'package:kim_mobile/models/models.dart';
import 'package:kim_mobile/router/open_chat.dart';
import 'package:kim_mobile/features/contacts/contacts.dart';
import 'package:kim_mobile/features/contacts/peer_profile.dart';
import 'package:kim_mobile/features/chats/inbox.dart';
import 'package:kim_mobile/features/session/mutations.dart';
import 'package:kim_mobile/features/profile/profile.dart';
import 'package:kim_mobile/features/session/providers.dart';
import 'package:kim_mobile/features/session/session.dart';
import 'package:kim_mobile/design/kim_theme.dart';
import 'package:kim_mobile/design/kim_avatar.dart';
import 'package:kim_mobile/design/kim_group.dart';
import 'package:kim_mobile/design/kim_header.dart';

class PeerProfilePage extends ConsumerStatefulWidget {
  const PeerProfilePage({
    super.key,
    required this.account,
    this.seedTitle = '',
  });

  final String account;
  final String seedTitle;

  @override
  ConsumerState<PeerProfilePage> createState() => _PeerProfilePageState();
}

class _PeerProfilePageState extends ConsumerState<PeerProfilePage> {
  @override
  void initState() {
    super.initState();
    unawaited(_load());
  }

  Future<void> _load() async {
    final account = widget.account;
    final social = ref.read(contactsProvider);
    final cached = social.person(account);
    if (cached != null) {
      ref.read(peerProfileProvider(account).notifier).showCached(cached);
    }
    try {
      final person = await ref.read(clientPortProvider).profile(dest: account);
      if (!mounted) {
        return;
      }
      ref.read(peerProfileProvider(account).notifier).showLoaded(person);
    } catch (err) {
      if (!mounted) {
        return;
      }
      ref
          .read(peerProfileProvider(account).notifier)
          .showFailed(
            error: socialError(err),
            fallback: KimPerson(
              account: account,
              nickname: widget.seedTitle.isEmpty ? account : widget.seedTitle,
            ),
          );
    }
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

  Future<void> _sendMessage(KimPerson person) async {
    openKimChat(context, ref, id: person.account, title: person.title);
  }

  Future<void> _addFriend(KimPerson person) async {
    try {
      await friendRequestMutation(person.account).run(ref, (tsx) {
        return tsx.get(contactsProvider.notifier).request(person.account);
      });
      final social = ref.read(contactsProvider);
      _toast(
        social.isFriend(person.account)
            ? Copy.friendAccepted
            : Copy.requestSent,
        success: true,
      );
      await _load();
    } catch (err) {
      _toast(socialError(err));
    }
  }

  Future<void> _deletePeer(KimPerson person) async {
    final l10n = AppLocalizations.of(context);
    final ok = await showDialog<bool>(
      context: context,
      builder: (ctx) {
        final scheme = Theme.of(ctx).colorScheme;
        return AlertDialog(
          title: Text(l10n.deletePeerTitle),
          content: Text(l10n.deletePeerBody),
          actions: [
            TextButton(
              onPressed: () => Navigator.of(ctx).pop(false),
              child: Text(l10n.cancel),
            ),
            TextButton(
              onPressed: () => Navigator.of(ctx).pop(true),
              style: TextButton.styleFrom(foregroundColor: scheme.error),
              child: Text(l10n.delete),
            ),
          ],
        );
      },
    );
    if (ok != true || !mounted) {
      return;
    }
    try {
      await friendRemoveMutation(person.account).run(ref, (tsx) async {
        await tsx
            .get(contactsProvider.notifier)
            .removePeer(person.account, isBot: person.isBot);
        await tsx.get(threadsProvider.notifier).deleteThread(person.account);
      });
      if (!mounted) {
        return;
      }
      _toast(l10n.peerDeleted, success: true);
      if (context.canPop()) {
        context.pop();
      } else {
        context.go('/contacts');
      }
    } catch (err) {
      _toast(socialError(err));
    }
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final scheme = theme.colorScheme;
    final l10n = AppLocalizations.of(context);
    final view = ref.watch(peerProfileProvider(widget.account));
    final social = ref.watch(contactsProvider);
    final me = ref.watch(sessionProvider).account;
    final person = view.person;
    final account = widget.account;
    final isSelf = account == me;
    final friend = social.isFriend(account);
    final outgoing = social.isOutgoing(account);
    final onFriendList = social.friends.any((p) => p.account == account);
    // Local-only agent rows (no b_ account) are not removable via chat.bot.delete.
    final localOnlyAgent = isAgentDest(account) && !isServerBotAccount(account);
    final showDelete = !isSelf && onFriendList && !localOnlyAgent;

    return Scaffold(
      body: CustomScrollView(
        slivers: [
          KimSliverHeader(title: l10n.profile),
          if (view.loading && person == null)
            const SliverFillRemaining(
              hasScrollBody: false,
              child: Center(child: CircularProgressIndicator()),
            )
          else if (person == null)
            SliverFillRemaining(
              hasScrollBody: false,
              child: Center(child: Text(view.error ?? l10n.userNotFound)),
            )
          else ...[
            SliverToBoxAdapter(
              child: Padding(
                padding: const EdgeInsets.fromLTRB(16, 0, 16, 16),
                child: DecoratedBox(
                  decoration: BoxDecoration(
                    color: KimTheme.raisedOf(context),
                    borderRadius: BorderRadius.circular(KimTheme.radiusCard),
                    border: Border.all(color: KimTheme.hairlineOf(context)),
                  ),
                  child: Padding(
                    padding: const EdgeInsets.fromLTRB(18, 22, 18, 22),
                    child: Row(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        KimAvatar(
                          name: person.title,
                          url: person.avatar.isEmpty
                              ? avatarFor(
                                  ref.watch(profileProvider),
                                  social,
                                  person.account,
                                )
                              : person.avatar,
                          size: KimAvatarSize.lg,
                          shape: KimAvatarShape.squircle,
                        ),
                        const Gap(16),
                        Expanded(
                          child: Column(
                            crossAxisAlignment: CrossAxisAlignment.start,
                            children: [
                              Row(
                                children: [
                                  Flexible(
                                    child: Text(
                                      person.title,
                                      style: theme.textTheme.headlineSmall
                                          ?.copyWith(
                                            fontWeight: FontWeight.w700,
                                          ),
                                    ),
                                  ),
                                  if (person.isBot) ...[
                                    const Gap(8),
                                    Container(
                                      padding: const EdgeInsets.symmetric(
                                        horizontal: 8,
                                        vertical: 2,
                                      ),
                                      decoration: BoxDecoration(
                                        color: scheme.secondaryContainer,
                                        borderRadius: BorderRadius.circular(
                                          999,
                                        ),
                                      ),
                                      child: Text(
                                        l10n.botBadge,
                                        style: theme.textTheme.labelSmall
                                            ?.copyWith(
                                              color:
                                                  scheme.onSecondaryContainer,
                                              fontWeight: FontWeight.w600,
                                            ),
                                      ),
                                    ),
                                  ],
                                ],
                              ),
                              const Gap(6),
                              Text(
                                '@${person.account}',
                                style: theme.textTheme.bodyMedium?.copyWith(
                                  color: scheme.onSurfaceVariant,
                                ),
                              ),
                              if (person.bio.isNotEmpty) ...[
                                const Gap(10),
                                Text(
                                  person.bio,
                                  style: theme.textTheme.bodyMedium,
                                ),
                              ],
                            ],
                          ),
                        ),
                      ],
                    ),
                  ),
                ),
              ),
            ),
            SliverPadding(
              padding: const EdgeInsets.symmetric(horizontal: 16),
              sliver: SliverList.list(
                children: [
                  KimGroupCard(
                    children: [
                      if (!isSelf)
                        ListTile(
                          leading: const Icon(LucideIcons.messageCircle),
                          title: Text(l10n.chatAction),
                          onTap: () => unawaited(_sendMessage(person)),
                        ),
                      if (!isSelf && !friend && !person.isBot) ...[
                        const Divider(indent: 56),
                        ListTile(
                          leading: const Icon(LucideIcons.userPlus),
                          title: Text(
                            outgoing ? l10n.requested : l10n.addFriend,
                          ),
                          onTap: outgoing
                              ? null
                              : () => unawaited(_addFriend(person)),
                        ),
                      ],
                      if (showDelete) ...[
                        const Divider(indent: 56),
                        ListTile(
                          leading: Icon(
                            LucideIcons.trash2,
                            color: scheme.error,
                          ),
                          title: Text(
                            l10n.deletePeer,
                            style: TextStyle(color: scheme.error),
                          ),
                          onTap: () => unawaited(_deletePeer(person)),
                        ),
                      ],
                    ],
                  ),
                  if (view.error != null) ...[
                    const Gap(12),
                    Text(
                      view.error!,
                      style: theme.textTheme.bodySmall?.copyWith(
                        color: scheme.error,
                      ),
                    ),
                  ],
                  const Gap(24),
                ],
              ),
            ),
          ],
        ],
      ),
    );
  }
}
