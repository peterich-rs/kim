library;

enum ConnStatus { connecting, online, reconnecting, offline }

/// Peer presence from room enter / chat.presence. Unknown = no badge.

enum PeerPresenceStatus { unknown, offline, online, busy }

PeerPresenceStatus peerPresenceFromWire(int status) {
  switch (status) {
    case 1:
      return PeerPresenceStatus.offline;
    case 2:
      return PeerPresenceStatus.online;
    case 3:
      return PeerPresenceStatus.busy;
    default:
      return PeerPresenceStatus.unknown;
  }
}

class KimLinkState {
  const KimLinkState({
    this.status = ConnStatus.offline,
    this.attempt = 0,
    this.error,
  });

  final ConnStatus status;
  final int attempt;
  final String? error;

  static ConnStatus? parseStatus(String raw) {
    switch (raw) {
      case 'Connecting':
        return ConnStatus.connecting;
      case 'Online':
        return ConnStatus.online;
      case 'Reconnecting':
        return ConnStatus.reconnecting;
      case 'Offline':
        return ConnStatus.offline;
      default:
        return null;
    }
  }

  static ConnStatus statusFromLabel(String raw) {
    return parseStatus(raw) ?? ConnStatus.offline;
  }
}
