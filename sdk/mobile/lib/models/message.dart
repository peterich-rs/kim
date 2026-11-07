library;

int _jsonInt(Object? value) {
  if (value is int) {
    return value;
  }
  if (value is num) {
    return value.toInt();
  }
  return 0;
}

enum KimMsgKind { text, image, video, voice, agentCard }

KimMsgKind kimMsgKindFromName(String? raw) {
  if (raw == null || raw.isEmpty) {
    return KimMsgKind.text;
  }
  for (final kind in KimMsgKind.values) {
    if (kind.name == raw) {
      return kind;
    }
  }
  return KimMsgKind.text;
}

class KimAgentCard {
  const KimAgentCard({
    required this.callId,
    required this.name,
    required this.actionRequired,
    required this.pending,
    required this.preview,
    required this.ok,
  });

  final String callId;
  final String name;
  final bool actionRequired;
  final bool pending;
  final String preview;
  final bool ok;
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
    this.card,
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
  final KimAgentCard? card;

  bool get isText => kind == KimMsgKind.text;

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

  bool get isVoice => kind == KimMsgKind.voice;

  bool get isAgentCard => kind == KimMsgKind.agentCard && card != null;

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
      kind: kimMsgKindFromName(
        json['kind'] is String ? json['kind'] as String : null,
      ),
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
