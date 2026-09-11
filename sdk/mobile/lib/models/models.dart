library;

enum ThreadKind { user, group }

/// `UserProfile.kind`: 1 human, 2 bot. Missing/0 on the wire is human.
class ProfileKind {
  static const user = 1;
  static const bot = 2;
}

class KimPerson {
  const KimPerson({
    required this.account,
    required this.nickname,
    this.avatar = '',
    this.kind = ProfileKind.user,
  });

  final String account;
  final String nickname;
  final String avatar;
  final int kind;

  String get title => nickname.isEmpty ? account : nickname;

  bool get isBot => kind == ProfileKind.bot;
}

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

  static ConnStatus statusFromLabel(String raw) {
    switch (raw) {
      case 'Connecting':
        return ConnStatus.connecting;
      case 'Online':
        return ConnStatus.online;
      case 'Reconnecting':
        return ConnStatus.reconnecting;
      default:
        return ConnStatus.offline;
    }
  }
}

enum KimEventKind {
  talk,
  kick,
  friend,
  friendAccepted,
  profileUpdated,
  presence,
  typing,
  receiptRead,
  group,
  token,
  closed,
  link,
  inbox,
  syncProgress,
  syncDone,
  syncFailed,
  authExpired,
  syncPage,
}

class KimEvent {
  const KimEvent({
    required this.kind,
    this.dest = '',
    this.sender = '',
    this.body = '',
    this.extra = '',
    this.messageId = 0,
    this.sendTime = 0,
    this.token = '',
    this.exp = 0,
    this.state = '',
    this.attempt = 0,
    this.inbox = const [],
    this.pulled = 0,
    this.pagePending = false,
    this.error = '',
    this.msgType = 0,
    this.nickname = '',
    this.talks = const [],
    this.pageId = 0,
  });

  final KimEventKind kind;
  final String dest;
  final String sender;
  final String body;
  final String extra;
  final int messageId;
  final int sendTime;
  final String token;
  final int exp;
  final String state;
  final int attempt;
  final List<KimThread> inbox;
  final int pulled;
  final bool pagePending;
  final String error;
  final int msgType;
  final String nickname;
  final List<KimEvent> talks;
  final int pageId;
}

class KimTalkResult {
  const KimTalkResult({required this.messageId, required this.sendTime});

  final int messageId;
  final int sendTime;
}

class KimHistoryMsg {
  const KimHistoryMsg({
    required this.messageId,
    required this.msgType,
    required this.body,
    required this.extra,
    required this.sender,
    required this.sendTime,
    required this.direction,
  });

  final int messageId;
  final int msgType;
  final String body;
  final String extra;
  final String sender;
  final int sendTime;
  final int direction;
}

sealed class KimOutgoingContent {
  const KimOutgoingContent();

  const factory KimOutgoingContent.text(String text) = KimTextContent;

  const factory KimOutgoingContent.image({
    required String url,
    required int width,
    required int height,
  }) = KimImageContent;

  const factory KimOutgoingContent.video({required String url}) =
      KimVideoContent;
}

class KimTextContent extends KimOutgoingContent {
  const KimTextContent(this.text);

  final String text;
}

class KimImageContent extends KimOutgoingContent {
  const KimImageContent({
    required this.url,
    required this.width,
    required this.height,
  });

  final String url;
  final int width;
  final int height;
}

class KimVideoContent extends KimOutgoingContent {
  const KimVideoContent({required this.url});

  final String url;
}

class KimThread {
  const KimThread({
    required this.id,
    required this.kind,
    required this.title,
    this.lastBody = '',
    this.lastAt = 0,
    this.unread = 0,
    this.avatar = '',
  });

  final String id;
  final ThreadKind kind;
  final String title;
  final String lastBody;
  final int lastAt;
  final int unread;
  final String avatar;

  KimThread copyWith({
    String? id,
    ThreadKind? kind,
    String? title,
    String? lastBody,
    int? lastAt,
    int? unread,
    String? avatar,
  }) {
    return KimThread(
      id: id ?? this.id,
      kind: kind ?? this.kind,
      title: title ?? this.title,
      lastBody: lastBody ?? this.lastBody,
      lastAt: lastAt ?? this.lastAt,
      unread: unread ?? this.unread,
      avatar: avatar ?? this.avatar,
    );
  }

  Map<String, Object?> toJson() => {
    'id': id,
    'kind': kind.name,
    'title': title,
    'lastBody': lastBody,
    'lastAt': lastAt,
    'unread': unread,
    'avatar': avatar,
  };

  factory KimThread.fromJson(Map<String, Object?> json) {
    final id = json['id'];
    if (id is! String || id.isEmpty) {
      throw const FormatException('KimThread.id');
    }
    final title = json['title'];
    return KimThread(
      id: id,
      kind: json['kind'] == 'group' ? ThreadKind.group : ThreadKind.user,
      title: title is String && title.isNotEmpty ? title : id,
      lastBody: json['lastBody'] is String ? json['lastBody'] as String : '',
      lastAt: _jsonInt(json['lastAt']),
      unread: _jsonInt(json['unread']),
      avatar: json['avatar'] is String ? json['avatar'] as String : '',
    );
  }

  static KimThread? tryFromJson(Object? raw) {
    if (raw is! Map) {
      return null;
    }
    try {
      return KimThread.fromJson(Map<String, Object?>.from(raw));
    } on FormatException {
      return null;
    }
  }
}

enum KimMsgKind { text, image, video, agentCard }

KimMsgKind kimMsgKindFromName(String? raw) {
  switch (raw) {
    case 'video':
      return KimMsgKind.video;
    case 'image':
      return KimMsgKind.image;
    case 'agentCard':
      return KimMsgKind.agentCard;
    default:
      return KimMsgKind.text;
  }
}

enum KimSendStatus { sending, sent, failed }

class KimChatMsg {
  const KimChatMsg({
    required this.key,
    required this.dest,
    required this.sender,
    required this.body,
    required this.at,
    this.sys = false,
    this.failed = false,
    this.kind = KimMsgKind.text,
    this.width = 0,
    this.height = 0,
    this.messageId = 0,
    this.batchId,
    this.status = KimSendStatus.sent,
    this.localPath,
  });

  final String key;
  final String dest;
  final String sender;
  final String body;
  final int at;
  final bool sys;
  final bool failed;
  final KimMsgKind kind;
  final int width;
  final int height;
  final int messageId;
  final String? batchId;
  final KimSendStatus status;
  final String? localPath;

  bool get isImage => kind == KimMsgKind.image;

  /// Prefer a local file for display after the body has been replaced by a URL.
  String get displaySrc {
    final local = localPath;
    if (local != null && local.isNotEmpty) {
      return local;
    }
    return body;
  }

  bool get isVideo => kind == KimMsgKind.video;

  bool get isAgentCard => kind == KimMsgKind.agentCard;

  bool get isFailed => failed || status == KimSendStatus.failed;

  bool get isSending => status == KimSendStatus.sending;

  KimChatMsg copyWith({
    String? body,
    bool? failed,
    KimMsgKind? kind,
    int? width,
    int? height,
    int? messageId,
    String? batchId,
    KimSendStatus? status,
    int? at,
    String? localPath,
  }) {
    final nextStatus =
        status ??
        (failed == null
            ? this.status
            : (failed ? KimSendStatus.failed : KimSendStatus.sent));
    final nextFailed = failed ?? (nextStatus == KimSendStatus.failed);
    return KimChatMsg(
      key: key,
      dest: dest,
      sender: sender,
      body: body ?? this.body,
      at: at ?? this.at,
      sys: sys,
      failed: nextFailed,
      kind: kind ?? this.kind,
      width: width ?? this.width,
      height: height ?? this.height,
      messageId: messageId ?? this.messageId,
      batchId: batchId ?? this.batchId,
      status: nextStatus,
      localPath: localPath ?? this.localPath,
    );
  }

  Map<String, Object?> toJson() => {
    'key': key,
    'dest': dest,
    'sender': sender,
    'body': body,
    'at': at,
    'sys': sys,
    'failed': isFailed,
    'kind': kind.name,
    'width': width,
    'height': height,
    'messageId': messageId,
    'batchId': batchId,
    'status': status.name,
    'localPath': localPath,
  };

  factory KimChatMsg.fromJson(Map<String, Object?> json) {
    final key = json['key'];
    final dest = json['dest'];
    final sender = json['sender'];
    final body = json['body'];
    if (key is! String ||
        dest is! String ||
        sender is! String ||
        body is! String) {
      throw const FormatException('KimChatMsg');
    }
    final statusRaw = json['status'];
    final failed = json['failed'] == true;
    final status = statusRaw == 'sending'
        ? KimSendStatus.sending
        : statusRaw == 'failed' || failed
        ? KimSendStatus.failed
        : KimSendStatus.sent;
    return KimChatMsg(
      key: key,
      dest: dest,
      sender: sender,
      body: body,
      at: _jsonInt(json['at']),
      sys: json['sys'] == true,
      failed: status == KimSendStatus.failed,
      kind: kimMsgKindFromName(json['kind'] is String ? json['kind'] as String : null),
      width: _jsonInt(json['width']),
      height: _jsonInt(json['height']),
      messageId: _jsonInt(json['messageId']),
      batchId: json['batchId'] is String ? json['batchId'] as String : null,
      status: status,
      localPath: json['localPath'] is String
          ? json['localPath'] as String
          : null,
    );
  }

  static KimChatMsg? tryFromJson(Object? raw) {
    if (raw is! Map) {
      return null;
    }
    try {
      return KimChatMsg.fromJson(Map<String, Object?>.from(raw));
    } on FormatException {
      return null;
    }
  }
}

int _jsonInt(Object? value) {
  if (value is int) {
    return value;
  }
  if (value is num) {
    return value.toInt();
  }
  return 0;
}
