library;

import 'package:kim_mobile/src/rust/api/types.dart' show ThreadPreview;

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
    this.bio = '',
    this.kind = ProfileKind.user,
  });

  final String account;
  final String nickname;
  final String avatar;
  final String bio;
  final int kind;

  String get title => nickname.isEmpty ? account : nickname;

  bool get isBot => kind == ProfileKind.bot;
}

class KimThread {
  const KimThread({
    required this.id,
    required this.kind,
    required this.title,
    this.lastBody = '',
    this.preview = const ThreadPreview.text(snippet: ''),
    this.lastAt = 0,
    this.unread = 0,
    this.avatar = '',
  });

  final String id;
  final ThreadKind kind;
  final String title;
  final String lastBody;
  final ThreadPreview preview;
  final int lastAt;
  final int unread;
  final String avatar;

  KimThread copyWith({
    String? id,
    ThreadKind? kind,
    String? title,
    String? lastBody,
    ThreadPreview? preview,
    int? lastAt,
    int? unread,
    String? avatar,
  }) {
    return KimThread(
      id: id ?? this.id,
      kind: kind ?? this.kind,
      title: title ?? this.title,
      lastBody: lastBody ?? this.lastBody,
      preview: preview ?? this.preview,
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

int _jsonInt(Object? value) {
  if (value is int) {
    return value;
  }
  if (value is num) {
    return value.toInt();
  }
  return 0;
}
