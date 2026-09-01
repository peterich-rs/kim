library;

enum ThreadKind { user, group }

class KimPerson {
  const KimPerson({
    required this.account,
    required this.nickname,
    this.avatar = '',
  });

  final String account;
  final String nickname;
  final String avatar;

  String get title => nickname.isEmpty ? account : nickname;
}

enum ConnStatus { connecting, online, reconnecting, offline }

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
  group,
  token,
  closed,
  link,
  inbox,
  syncProgress,
  syncDone,
  syncFailed,
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

  static KimThread? fromJson(Object? raw) {
    if (raw is! Map) {
      return null;
    }
    final id = raw['id'];
    if (id is! String || id.isEmpty) {
      return null;
    }
    final kindRaw = raw['kind'];
    final kind = kindRaw == 'group' ? ThreadKind.group : ThreadKind.user;
    return KimThread(
      id: id,
      kind: kind,
      title: raw['title'] is String && (raw['title'] as String).isNotEmpty
          ? raw['title'] as String
          : id,
      lastBody: raw['lastBody'] is String ? raw['lastBody'] as String : '',
      lastAt: raw['lastAt'] is int ? raw['lastAt'] as int : 0,
      unread: raw['unread'] is int ? raw['unread'] as int : 0,
      avatar: raw['avatar'] is String ? raw['avatar'] as String : '',
    );
  }
}

enum KimMsgKind { text, image, video }

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

  bool get isImage => kind == KimMsgKind.image;

  bool get isVideo => kind == KimMsgKind.video;

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
  };

  static KimChatMsg? fromJson(Object? raw) {
    if (raw is! Map) {
      return null;
    }
    final key = raw['key'];
    final dest = raw['dest'];
    final sender = raw['sender'];
    final body = raw['body'];
    if (key is! String ||
        dest is! String ||
        sender is! String ||
        body is! String) {
      return null;
    }
    final statusRaw = raw['status'];
    final failed = raw['failed'] == true;
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
      at: raw['at'] is int ? raw['at'] as int : 0,
      sys: raw['sys'] == true,
      failed: status == KimSendStatus.failed,
      kind: raw['kind'] == 'video'
          ? KimMsgKind.video
          : raw['kind'] == 'image'
          ? KimMsgKind.image
          : KimMsgKind.text,
      width: raw['width'] is int ? raw['width'] as int : 0,
      height: raw['height'] is int ? raw['height'] as int : 0,
      messageId: raw['messageId'] is int ? raw['messageId'] as int : 0,
      batchId: raw['batchId'] is String ? raw['batchId'] as String : null,
      status: status,
    );
  }
}
