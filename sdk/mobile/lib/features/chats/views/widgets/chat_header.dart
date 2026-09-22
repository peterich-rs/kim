library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:gap/gap.dart';
import 'package:go_router/go_router.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import 'package:kim_mobile/copy.dart';
import 'package:kim_mobile/core/layout.dart';
import 'package:kim_mobile/design/kim_avatar.dart';
import 'package:kim_mobile/design/status_chip.dart';
import 'package:kim_mobile/features/chats/providers/chat_chrome.dart';
import 'package:kim_mobile/features/chats/providers/chat_view.dart';
import 'package:kim_mobile/features/session/link.dart';
import 'package:kim_mobile/features/session/presence.dart';
import 'package:kim_mobile/features/session/session.dart';
import 'package:kim_mobile/models/models.dart';
import 'package:kim_mobile/router/open_peer.dart';
import 'package:kim_mobile/router/app_routes.dart';

class ChatHeader extends ConsumerWidget {
  const ChatHeader({super.key, required this.dest});

  final String dest;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = AppLocalizations.of(context);
    final title = ref.watch(
      chatChromeProvider(dest).select((c) => c.liveTitle),
    );
    final avatarUrl = ref.watch(
      chatChromeProvider(dest).select((c) => c.avatarUrl),
    );
    final kind = ref.watch(chatChromeProvider(dest).select((c) => c.kind));
    final wide = kimIsWide(context);
    final presence = ref.watch(peerPresenceProvider(dest));
    final status = ref.watch(sessionProvider.select((s) => s.status));
    final error = ref.watch(sessionProvider.select((s) => s.connectError));
    return SafeArea(
      bottom: false,
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          Padding(
            padding: const EdgeInsets.fromLTRB(12, 4, 12, 8),
            child: Row(
              children: [
                if (!wide)
                  FrostedCircleButton(
                    key: const Key('chat-back'),
                    onTap: () {
                      if (context.canPop()) {
                        context.pop();
                      } else {
                        context.go(AppRoutes.home);
                      }
                    },
                    child: const Icon(LucideIcons.chevronLeft, size: 22),
                  ),
                if (!wide) const Gap(8),
                ChatTitleChrome(
                  title: title,
                  avatarUrl: avatarUrl,
                  avatar: KimAvatar(
                    name: title,
                    url: avatarUrl,
                    size: KimAvatarSize.sm,
                    shape: KimAvatarShape.squircle,
                  ),
                  presence: presence,
                  onTap: kind == ThreadKind.user
                      ? () => openKimPeerProfile(
                          context,
                          ref,
                          id: dest,
                          title: title,
                        )
                      : null,
                ),
                const Spacer(),
                FrostedCircleButton(
                  key: const Key('chat-more'),
                  tooltip: l10n.more,
                  child: const Icon(LucideIcons.ellipsis, size: 20),
                ),
              ],
            ),
          ),
          ConnectionBanner(
            status: status,
            error: error,
            onRetry: () => ref.read(linkProvider.notifier).retry(),
          ),
        ],
      ),
    );
  }
}
