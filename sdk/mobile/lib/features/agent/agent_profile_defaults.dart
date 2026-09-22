part of 'agent_profiles.dart';


const _kProfiles = 'agent.profiles';
const _kActive = 'agent.active_profile_id';
const _kGooseKey = 'agent.api_key.goose';
const _kMulti = 'agent.multi_profile';
const _kMultiMigrated = 'agent.multi_profile_migrated_on';
const _kServerIdentity = 'agent.server_identity';
const _kIdentityMigrated = 'agent.identity_migrated_on';

/// Byte-identical to Rust `DEFAULT_IDENTITY_PROMPT`. Empty prompt injects this.
const kDefaultSystemPrompt =
    'You are a local desktop agent inside the KIM messenger. '
    'You run on the user\'s machine (not a cloud bot). Reply in the user\'s language. Be concise. '
    'Only use tools that appear in your tool list; never claim tools you were not given.';

/// Pre-B-KD 4 identity that enumerated every IM tool. Load as empty.
const kLegacyToolLaundryIdentity =
    'You are 助手, a local desktop agent inside the KIM messenger. '
    'You run on the user\'s machine (not a cloud bot). Reply in the user\'s language. '
    'Be concise. You can see the current conversation because the host pasted it into this session. '
    'You have search_contacts, search_messages, get_conversation_context, list_profiles, '
    'send_message, and read_clipboard. send_message and clipboard require user confirmation. '
    'You do not have filesystem or shell access. Do not claim you have tools you were not given.';

bool isLegacyToolLaundryIdentity(String prompt) {
  final t = prompt.trim();
  if (t.isEmpty) {
    return false;
  }
  if (t == kLegacyToolLaundryIdentity) {
    return true;
  }
  return t.contains('You are 助手') &&
      t.contains('search_contacts') &&
      t.contains('search_messages') &&
      t.contains('get_conversation_context') &&
      t.contains('list_profiles') &&
      t.contains('send_message') &&
      t.contains('read_clipboard') &&
      t.contains('You do not have filesystem or shell access');
}

String migrateIdentityPrompt(String prompt) {
  return isLegacyToolLaundryIdentity(prompt) ? '' : prompt;
}

/// Default capabilities for a new persona (B-KD create defaults).
const kCreateDefaultCapabilities = <CapabilityRef>[
  CapabilityRef(kind: CapabilityKinds.imSendMessage),
  CapabilityRef(kind: CapabilityKinds.imReadClipboard),
];

/// Projection of [kCreateDefaultCapabilities] for one-release ToolSet compat.
final kCreateDefaultTools = projectToolSet(kCreateDefaultCapabilities);

/// Aligns with Chat `BOT_MAX_PER_OWNER`.
const kMaxBotsPerOwner = 20;

class AgentProfileCapExceeded implements Exception {
  @override
  String toString() => Copy.agentCapReached;
}

/// Profiles that have or will have a cloud bot when [serverIdentity] is on,
/// including disabled rows.
int cloudIdentitySlots(
  Iterable<AgentProfile> profiles, {
  required bool serverIdentity,
}) {
  var n = 0;
  for (final p in profiles) {
    if (p.serverAccount.isNotEmpty || serverIdentity) {
      n++;
    }
  }
  return n;
}

bool isBotAlreadyGone(Object err) => botAlreadyGone(err);

String agentRegisterError(Object err) => agentRegisterFailureCopy(err);
