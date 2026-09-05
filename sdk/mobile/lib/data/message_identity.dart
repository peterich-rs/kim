/// Message row identity. `key` is stable; `messageId` is the server id.
library;

/// Incoming/history rows use a `m{messageId}` key so they never collide with
/// a local UUID clientId used as `key` for outgoing drafts.
String incomingMessageKey({
  required int messageId,
  int sendTime = 0,
  String sender = '',
}) {
  if (messageId != 0) {
    return 'm$messageId';
  }
  return 'talk-$sendTime-$sender';
}

/// UUID v4 (8-4-4-4-12) used as the outbox clientId / row key.
bool isClientKey(String key) {
  if (key.length != 36) {
    return false;
  }
  return key[8] == '-' && key[13] == '-' && key[18] == '-' && key[23] == '-';
}

/// Prefer the local UUID row when a history/push row shares a messageId.
String preferKey(String a, String b) {
  if (isClientKey(a) && !isClientKey(b)) {
    return a;
  }
  if (isClientKey(b) && !isClientKey(a)) {
    return b;
  }
  return a;
}
