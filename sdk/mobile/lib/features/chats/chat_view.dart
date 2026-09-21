library;

import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:kim_mobile/features/agent/agent_profiles.dart';
import 'package:kim_mobile/features/agent/host_support.dart';
import 'package:kim_mobile/features/agent/mention.dart';
import 'package:kim_mobile/features/chats/inbox.dart';
import 'package:kim_mobile/features/contacts/contacts.dart';
import 'package:kim_mobile/features/profile/profile.dart';
import 'package:kim_mobile/models/models.dart';

/// Derived chrome for one conversation. Widgets [select] fields so a title
/// change does not rebuild the message list.
class ChatChrome {
  const ChatChrome({
    required this.liveTitle,
    required this.avatarUrl,
    required this.readOnly,
    required this.gated,
    required this.userThread,
    required this.showTyping,
    required this.agentThread,
    required this.agentChat,
    required this.kind,
    required this.incoming,
    required this.outgoing,
  });

  final String liveTitle;
  final String avatarUrl;
  final bool readOnly;
  final bool gated;
  final bool userThread;
  final bool showTyping;
  final bool agentThread;
  final bool agentChat;
  final ThreadKind kind;
  final bool incoming;
  final bool outgoing;
}

ChatChrome deriveChatChrome({
  required String dest,
  required ThreadKind kind,
  required ContactsState social,
  required ProfileState me,
  required List<AgentProfile> profiles,
  required bool profilesReady,
  required bool hostSupported,
}) {
  final agentChat = isAgentDest(dest);
  final orphan = profilesReady && profileForChatDest(dest, profiles) == null;
  final knownBot = agentChat || isServerBotAccount(dest);
  final desktopOrphan = hostSupported && orphan && knownBot;
  final phoneOrphan =
      !hostSupported &&
      social.ready &&
      isServerBotAccount(dest) &&
      !social.friends.any((p) => p.account == dest);
  final readOnly = desktopOrphan || phoneOrphan;
  final userThread = kind == ThreadKind.user && !agentChat;
  final showTyping =
      kind == ThreadKind.user || agentChat || isServerBotAccount(dest);
  final liveTitle = kind == ThreadKind.user
      ? (social.person(dest)?.title ?? dest)
      : dest;
  final gated =
      !agentChat &&
      !isServerBotAccount(dest) &&
      kind == ThreadKind.user &&
      social.ready &&
      !social.isFriend(dest);
  return ChatChrome(
    liveTitle: liveTitle,
    avatarUrl: avatarFor(me, social, dest),
    readOnly: readOnly,
    gated: gated,
    userThread: userThread,
    showTyping: showTyping,
    agentThread: knownBot,
    agentChat: agentChat,
    kind: kind,
    incoming: social.isIncoming(dest),
    outgoing: social.isOutgoing(dest),
  );
}

final chatChromeProvider = Provider.autoDispose.family<ChatChrome, String>((
  ref,
  dest,
) {
  final social = ref.watch(contactsProvider);
  final me = ref.watch(profileProvider);
  final profiles = ref.watch(agentProfilesProvider);
  final ready = ref.watch(agentProfilesProvider.notifier).profilesReady;
  final kind = ref.watch(threadsProvider).thread(dest)?.kind ?? ThreadKind.user;
  return deriveChatChrome(
    dest: dest,
    kind: kind,
    social: social,
    me: me,
    profiles: profiles,
    profilesReady: ready,
    hostSupported: agentHostSupported,
  );
});
