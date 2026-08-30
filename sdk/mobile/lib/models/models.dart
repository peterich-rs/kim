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

enum KimEventKind { talk, kick, friend, group, token, closed }

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
}

class KimThread {
  const KimThread({
    required this.id,
    required this.kind,
    required this.title,
    this.lastBody = '',
    this.lastAt = 0,
    this.unread = 0,
  });

  final String id;
  final ThreadKind kind;
  final String title;
  final String lastBody;
  final int lastAt;
  final int unread;

  KimThread copyWith({
    String? id,
    ThreadKind? kind,
    String? title,
    String? lastBody,
    int? lastAt,
    int? unread,
  }) {
    return KimThread(
      id: id ?? this.id,
      kind: kind ?? this.kind,
      title: title ?? this.title,
      lastBody: lastBody ?? this.lastBody,
      lastAt: lastAt ?? this.lastAt,
      unread: unread ?? this.unread,
    );
  }

  Map<String, Object?> toJson() => {
    'id': id,
    'kind': kind.name,
    'title': title,
    'lastBody': lastBody,
    'lastAt': lastAt,
    'unread': unread,
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
    );
  }
}

enum KimMsgKind { text, image, video }

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

  bool get isImage => kind == KimMsgKind.image;

  bool get isVideo => kind == KimMsgKind.video;

  KimChatMsg copyWith({bool? failed}) {
    return KimChatMsg(
      key: key,
      dest: dest,
      sender: sender,
      body: body,
      at: at,
      sys: sys,
      failed: failed ?? this.failed,
      kind: kind,
      width: width,
      height: height,
    );
  }

  Map<String, Object?> toJson() => {
    'key': key,
    'dest': dest,
    'sender': sender,
    'body': body,
    'at': at,
    'sys': sys,
    'failed': failed,
    'kind': kind.name,
    'width': width,
    'height': height,
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
    return KimChatMsg(
      key: key,
      dest: dest,
      sender: sender,
      body: body,
      at: raw['at'] is int ? raw['at'] as int : 0,
      sys: raw['sys'] == true,
      failed: raw['failed'] == true,
      kind: raw['kind'] == 'video'
          ? KimMsgKind.video
          : raw['kind'] == 'image'
          ? KimMsgKind.image
          : KimMsgKind.text,
      width: raw['width'] is int ? raw['width'] as int : 0,
      height: raw['height'] is int ? raw['height'] as int : 0,
    );
  }
}
