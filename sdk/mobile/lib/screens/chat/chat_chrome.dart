library;

import 'dart:async';
import 'dart:ui';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:gap/gap.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';
import 'package:toastification/toastification.dart';
import 'package:wolt_modal_sheet/wolt_modal_sheet.dart';

import '../../core/format.dart';
import '../../core/haptics.dart';
import '../../widgets/kim_composer.dart';

import '../../copy.dart';
import '../../models/models.dart';
import '../../state/contacts.dart';
import '../../state/mutations.dart';
import '../../state/profile.dart';
import '../../theme/kim_theme.dart';
import '../../widgets/kim_avatar.dart';
import '../../widgets/status_chip.dart';

Future<void> showChatMessageSheet({
  required BuildContext context,
  required KimChatMsg message,
  required GlobalKey<KimComposerState> composer,
  required Future<void> Function(String key) onRetry,
  required void Function(String message) onCopied,
}) async {
  await KimHaptics.light();
  if (!context.mounted) {
    return;
  }
  final l10n = AppLocalizations.of(context);
  final text = message.sys ||
          message.isImage ||
          message.isVideo ||
          message.isAgentCard
      ? ''
      : message.body;
  final stamp = formatMessageStamp(message.at);
  await WoltModalSheet.show<void>(
    context: context,
    showDragHandle: true,
    pageListBuilder: (sheetContext) => [
      WoltModalSheetPage(
        hasSabGradient: false,
        navBarHeight: 28,
        child: Padding(
          padding: const EdgeInsets.fromLTRB(8, 0, 8, 28),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              if (stamp.isNotEmpty) ListTile(dense: true, title: Text(stamp)),
              if (text.isNotEmpty)
                ListTile(
                  leading: const Icon(LucideIcons.copy, size: 18),
                  title: Text(l10n.copy),
                  onTap: () async {
                    await Clipboard.setData(ClipboardData(text: text));
                    if (sheetContext.mounted) {
                      Navigator.of(sheetContext).pop();
                    }
                    onCopied(l10n.copied);
                  },
                ),
              if (text.isNotEmpty)
                ListTile(
                  leading: const Icon(LucideIcons.quote, size: 18),
                  title: Text(l10n.quote),
                  onTap: () {
                    Navigator.of(sheetContext).pop();
                    composer.currentState?.quote(text);
                  },
                ),
              if (message.isFailed)
                ListTile(
                  leading: Icon(
                    LucideIcons.refreshCw,
                    size: 18,
                    color: Theme.of(sheetContext).colorScheme.error,
                  ),
                  title: Text(l10n.retry),
                  onTap: () {
                    Navigator.of(sheetContext).pop();
                    unawaited(onRetry(message.key));
                  },
                ),
            ],
          ),
        ),
      ),
    ],
  );
}

class FrostedCircleButton extends StatelessWidget {
  const FrostedCircleButton({
    super.key,
    required this.child,
    this.tooltip,
    this.onTap,
  });

  final Widget child;
  final String? tooltip;
  final VoidCallback? onTap;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    final button = ClipOval(
      child: BackdropFilter(
        filter: ImageFilter.blur(sigmaX: 18, sigmaY: 18),
        child: Material(
          color: KimTheme.frostFillOf(context),
          shape: const CircleBorder(),
          child: InkWell(
            customBorder: const CircleBorder(),
            onTap: onTap,
            child: SizedBox(
              width: 40,
              height: 40,
              child: IconTheme.merge(
                data: IconThemeData(color: scheme.onSurface, size: 20),
                child: Center(child: child),
              ),
            ),
          ),
        ),
      ),
    );
    if (tooltip == null) {
      return button;
    }
    return Tooltip(message: tooltip!, child: button);
  }
}

class ChatTitleChrome extends StatelessWidget {
  const ChatTitleChrome({
    super.key,
    required this.title,
    required this.avatarUrl,
    required this.presence,
  });

  final String title;
  final String avatarUrl;
  final PeerPresenceStatus presence;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return ClipRRect(
      borderRadius: BorderRadius.circular(22),
      child: BackdropFilter(
        filter: ImageFilter.blur(sigmaX: 18, sigmaY: 18),
        child: Container(
          padding: const EdgeInsets.fromLTRB(6, 4, 12, 4),
          color: KimTheme.frostFillOf(context),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              Stack(
                clipBehavior: Clip.none,
                children: [
                  KimAvatar(
                    name: title,
                    url: avatarUrl,
                    size: KimAvatarSize.sm,
                    shape: KimAvatarShape.squircle,
                  ),
                  if (presence != PeerPresenceStatus.unknown)
                    Positioned(
                      right: -1,
                      bottom: -1,
                      child: Container(
                        width: 12,
                        height: 12,
                        decoration: BoxDecoration(
                          color: KimTheme.chatCanvasOf(context),
                          shape: BoxShape.circle,
                        ),
                        alignment: Alignment.center,
                        child: PeerPresenceDot(status: presence, size: 8),
                      ),
                    ),
                ],
              ),
              const Gap(8),
              ConstrainedBox(
                constraints: const BoxConstraints(maxWidth: 180),
                child: Text(
                  title,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: theme.textTheme.titleMedium?.copyWith(
                    fontSize: KimTheme.fontTitle,
                    fontWeight: FontWeight.w600,
                  ),
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class FriendGate extends ConsumerWidget {
  const FriendGate({
    super.key,
    required this.dest,
    required this.title,
    required this.incoming,
    required this.outgoing,
  });

  final String dest;
  final String title;
  final bool incoming;
  final bool outgoing;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final theme = Theme.of(context);
    final l10n = AppLocalizations.of(context);
    final avatarUrl = avatarFor(
      ref.watch(profileProvider),
      ref.watch(contactsProvider),
      dest,
    );
    Future<void> run(Future<void> Function() action, String ok) async {
      try {
        await action();
        if (context.mounted) {
          toastification.show(
            context: context,
            type: ToastificationType.success,
            style: ToastificationStyle.flatColored,
            title: Text(ok),
            autoCloseDuration: const Duration(seconds: 2),
            alignment: Alignment.topCenter,
          );
        }
      } catch (err) {
        if (context.mounted) {
          toastification.show(
            context: context,
            type: ToastificationType.error,
            style: ToastificationStyle.flatColored,
            title: Text(socialError(err)),
            autoCloseDuration: const Duration(seconds: 3),
            alignment: Alignment.topCenter,
          );
        }
      }
    }

    return SafeArea(
      top: false,
      child: Padding(
        padding: const EdgeInsets.fromLTRB(16, 8, 16, 16),
        child: ClipRRect(
          borderRadius: BorderRadius.circular(KimTheme.radiusCard),
          child: BackdropFilter(
            filter: ImageFilter.blur(sigmaX: 18, sigmaY: 18),
            child: DecoratedBox(
              decoration: BoxDecoration(
                color: KimTheme.frostFillOf(context),
                borderRadius: BorderRadius.circular(KimTheme.radiusCard),
                border: Border.all(color: KimTheme.hairlineOf(context)),
              ),
              child: Padding(
                padding: const EdgeInsets.fromLTRB(16, 16, 16, 16),
                child: Column(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    KimAvatar(name: title, url: avatarUrl),
                    const Gap(10),
                    Text(l10n.notFriends, style: theme.textTheme.titleSmall),
                    const Gap(4),
                    Text(
                      outgoing
                          ? l10n.waitingAccept
                          : incoming
                          ? l10n.friendRequestToast
                          : l10n.addFriendToChat,
                      textAlign: TextAlign.center,
                      style: theme.textTheme.bodySmall,
                    ),
                    const Gap(12),
                    if (outgoing)
                      Text(l10n.requested, style: theme.textTheme.labelMedium)
                    else if (incoming)
                      FilledButton(
                        onPressed: () => run(
                          () => friendAcceptMutation.run(ref, (tsx) {
                            return tsx
                                .get(contactsProvider.notifier)
                                .accept(dest);
                          }),
                          l10n.friendAccepted,
                        ),
                        child: Text(l10n.accept),
                      )
                    else
                      FilledButton.tonal(
                        onPressed: () => run(
                          () => friendRequestMutation.run(ref, (tsx) {
                            return tsx
                                .get(contactsProvider.notifier)
                                .request(dest);
                          }),
                          l10n.requestSent,
                        ),
                        child: Text(l10n.addFriend),
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
