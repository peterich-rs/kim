import 'dart:async';

import 'package:kim_mobile/core/media.dart';
import 'package:kim_mobile/kim_bridge.dart';
import 'package:kim_mobile/models/models.dart';

class FakeKim implements KimAuthPort, KimClientPort {
  FakeKim({this.session, this.error, this.connectError});

  KimAuthSession? session;
  Object? error;
  Object? connectError;
  Object? talkError;
  int logins = 0;
  int registers = 0;
  int logouts = 0;
  int connects = 0;
  int talks = 0;
  int imageTalks = 0;
  int acks = 0;
  int confirms = 0;
  int radioUps = 0;
  int friendRequests = 0;
  int lastConfirm = 0;
  final eventsController = StreamController<KimEvent>.broadcast();
  List<KimPerson> friends = const [];
  List<KimPerson> incoming = const [];
  String lastUserAgent = '';
  String lastOrigin = '';
  String lastTalkDest = '';
  String lastTalkBody = '';
  String lastImageUrl = '';
  String lastImageExtra = '';
  String lastClientId = '';
  final List<String> clientIds = [];
  KimLinkState _link = const KimLinkState();

  KimAuthSession _ok() {
    return session ??
        const KimAuthSession(token: 'tok.jwt', exp: 1, account: 'alice');
  }

  Future<KimAuthSession> _run() async {
    if (error != null) {
      throw error!;
    }
    return _ok();
  }

  @override
  Future<KimAuthSession> login({
    required String origin,
    required String userAgent,
    required String account,
    required String password,
  }) async {
    logins += 1;
    lastOrigin = origin;
    lastUserAgent = userAgent;
    return _run();
  }

  @override
  Future<KimAuthSession> register({
    required String origin,
    required String userAgent,
    required String account,
    required String password,
  }) async {
    registers += 1;
    lastOrigin = origin;
    lastUserAgent = userAgent;
    return _run();
  }

  @override
  Future<void> logout({
    required String origin,
    required String userAgent,
    required String token,
  }) async {
    logouts += 1;
    lastUserAgent = userAgent;
  }

  @override
  Future<void> changePassword({
    required String origin,
    required String userAgent,
    required String token,
    required String oldPassword,
    required String newPassword,
  }) async {}

  @override
  String httpOriginFromWs(String wsUrl) => 'http://127.0.0.1:8080';

  @override
  Future<void> startSession(
    String url,
    String token, {
    required String userAgent,
  }) async {
    connects += 1;
    lastUserAgent = userAgent;
    if (connectError != null) {
      throw connectError!;
    }
    _link = const KimLinkState(status: ConnStatus.online);
    eventsController.add(
      const KimEvent(kind: KimEventKind.link, state: 'Online'),
    );
  }

  @override
  Future<void> stopSession() async {
    _link = const KimLinkState();
  }

  @override
  KimLinkState linkState() => _link;

  @override
  Stream<KimEvent> sessionEvents() => eventsController.stream;

  @override
  Future<void> syncConfirm(int cursor) async {
    confirms += 1;
    lastConfirm = cursor;
  }

  @override
  Future<void> notifyRadioUp() async {
    radioUps += 1;
  }

  @override
  Future<KimTalkResult> sendMessage(
    String dest,
    ThreadKind kind,
    KimOutgoingContent content, {
    required String clientId,
  }) async {
    lastClientId = clientId;
    clientIds.add(clientId);
    lastTalkDest = dest;
    switch (content) {
      case KimTextContent(:final text):
        talks += 1;
        lastTalkBody = text;
      case KimImageContent(:final url, :final width, :final height):
        imageTalks += 1;
        lastImageUrl = url;
        lastImageExtra = '{"w":$width,"h":$height}';
      case KimVideoContent(:final url):
        talks += 1;
        lastTalkBody = url;
    }
    if (talkError != null) {
      throw talkError!;
    }
    return const KimTalkResult(messageId: 1, sendTime: 1);
  }

  @override
  Future<List<KimHistoryMsg>> history(
    String dest,
    ThreadKind kind, {
    int beforeId = 0,
    int limit = 50,
  }) async {
    return const [];
  }

  @override
  Future<List<KimThread>> inboxList({int limit = 200}) async {
    return const [];
  }

  @override
  Future<void> ack(int messageId) async {
    acks += 1;
  }

  void emitAuthExpired({String error = 'unauthorized'}) {
    eventsController.add(
      KimEvent(kind: KimEventKind.authExpired, error: error),
    );
  }

  void emitTalk({
    required String dest,
    required String sender,
    required String body,
    String extra = '',
    int sendTime = 0,
    int messageId = 0,
  }) {
    eventsController.add(
      KimEvent(
        kind: KimEventKind.talk,
        dest: dest,
        sender: sender,
        body: body,
        extra: extra,
        messageId: messageId == 0
            ? DateTime.now().microsecondsSinceEpoch
            : messageId,
        sendTime: sendTime == 0
            ? DateTime.now().millisecondsSinceEpoch
            : sendTime,
      ),
    );
  }

  @override
  Future<List<KimPerson>> friendList() async => friends;

  @override
  Future<List<KimPerson>> friendIncoming() async => incoming;

  @override
  Future<List<KimPerson>> searchUsers(String query) async {
    return friends
        .where((p) => p.account.contains(query) || p.nickname.contains(query))
        .toList();
  }

  @override
  Future<void> friendRequest(String dest) async {
    friendRequests += 1;
  }

  @override
  Future<void> friendAccept(String dest) async {}

  @override
  Future<void> friendReject(String dest) async {}

  KimPerson me = const KimPerson(account: 'alice', nickname: 'alice');
  String lastAvatar = '';

  @override
  Future<KimPerson> profile({String dest = ''}) async {
    return me;
  }

  @override
  Future<KimPerson> updateProfile({
    required String nickname,
    required String avatar,
    String bio = '',
  }) async {
    lastAvatar = avatar;
    me = KimPerson(account: me.account, nickname: nickname, avatar: avatar);
    return me;
  }
}

class FakeKimMedia implements KimMediaPort {
  FakeKimMedia({this.url = 'https://media.kim.ainexc.com/alice/a.jpg'});

  final String url;
  int uploads = 0;
  List<int> lastBytes = const [];
  String lastType = '';

  @override
  Future<UploadedObject> uploadImage({
    required String token,
    required List<int> bytes,
    required String contentType,
  }) async {
    uploads += 1;
    lastBytes = bytes;
    lastType = contentType;
    return UploadedObject(
      key: 'alice/a.jpg',
      url: url,
      contentType: contentType,
      bytes: bytes.length,
    );
  }
}
