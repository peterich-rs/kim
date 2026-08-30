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
  int acks = 0;
  int friendRequests = 0;
  final eventsController = StreamController<KimEvent>.broadcast();
  List<KimPerson> friends = const [];
  List<KimPerson> incoming = const [];
  String lastUserAgent = '';
  String lastOrigin = '';
  String lastTalkDest = '';
  String lastTalkBody = '';

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
  Future<String> connect(
    String url,
    String token, {
    required String userAgent,
  }) async {
    connects += 1;
    if (connectError != null) {
      throw connectError!;
    }
    return 'connected $url';
  }

  @override
  Future<String> loginWs() async => 'channel_id=1 account=alice';

  @override
  Future<String> ping() async => 'pong';

  @override
  Future<String> talk(String dest, String body) async {
    talks += 1;
    lastTalkDest = dest;
    lastTalkBody = body;
    if (talkError != null) {
      throw talkError!;
    }
    return 'message_id=1 send_time=1';
  }

  @override
  Future<void> ack(int messageId) async {
    acks += 1;
  }

  @override
  Stream<KimEvent> events() => eventsController.stream;

  void emitTalk({
    required String dest,
    required String sender,
    required String body,
    int sendTime = 0,
  }) {
    eventsController.add(
      KimEvent(
        kind: KimEventKind.talk,
        dest: dest,
        sender: sender,
        body: body,
        messageId: DateTime.now().microsecondsSinceEpoch,
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

  @override
  Future<String> disconnect() async => 'disconnected';
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
