import 'dart:async';

import 'package:kim_mobile/core/media.dart';
import 'package:kim_mobile/data/conversation_store.dart';
import 'package:kim_mobile/kim_bridge.dart';
import 'package:kim_mobile/models/models.dart';

class FakeKim implements KimAuthPort, KimClientPort {
  FakeKim({this.session, this.error, this.connectError});

  KimAuthSession? session;
  Object? error;
  Object? connectError;
  Object? talkError;
  Duration? loginDelay;
  int logins = 0;
  int registers = 0;
  int logouts = 0;
  int connects = 0;
  int talks = 0;
  int imageTalks = 0;
  int acks = 0;
  int reads = 0;
  int typingCalls = 0;
  String lastTypingDest = '';
  bool lastTypingActive = false;
  String lastReadDest = '';
  int lastReadMessageId = 0;
  int talkSendTime = 1;
  int confirms = 0;
  int radioUps = 0;
  int foregrounds = 0;
  int friendRequests = 0;
  int lastConfirm = 0;
  final eventsController = StreamController<KimEvent>.broadcast();
  List<KimPerson> friends = const [];
  List<KimPerson> incoming = const [];
  String lastUserAgent = '';
  String lastOrigin = '';
  String lastAccount = '';
  String lastPassword = '';
  String lastTalkDest = '';
  int lastTalkKind = 0;
  String lastTalkBody = '';
  String lastImageUrl = '';
  String lastImageExtra = '';
  String lastClientId = '';
  final List<String> clientIds = [];
  KimLinkState _link = const KimLinkState();
  Completer<void>? sendHold;

  KimAuthSession _ok() {
    return session ??
        const KimAuthSession(token: 'tok.jwt', exp: 1, account: 'alice');
  }

  Future<KimAuthSession> _run() async {
    final delay = loginDelay;
    if (delay != null) {
      await Future<void>.delayed(delay);
    }
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
    lastAccount = account;
    lastPassword = password;
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
    lastAccount = account;
    lastPassword = password;
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
  Future<void> notifyForeground() async {
    foregrounds += 1;
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
    lastTalkKind = kind == ThreadKind.group ? 1 : 0;
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
    final hold = sendHold;
    if (hold != null) {
      await hold.future;
    }
    return KimTalkResult(messageId: 1, sendTime: talkSendTime);
  }

  List<KimHistoryMsg> historyRows = const [];
  int historyCalls = 0;
  int lastHistoryBeforeId = 0;

  @override
  Future<List<KimHistoryMsg>> history(
    String dest,
    ThreadKind kind, {
    int beforeId = 0,
    int limit = 50,
  }) async {
    historyCalls += 1;
    lastHistoryBeforeId = beforeId;
    if (beforeId <= 0) {
      return historyRows.take(limit).toList();
    }
    return historyRows
        .where((m) => m.messageId < beforeId)
        .take(limit)
        .toList();
  }

  @override
  Future<List<KimThread>> inboxList({int limit = 200}) async {
    return const [];
  }

  @override
  Future<void> ack(int messageId) async {
    acks += 1;
  }

  @override
  Future<void> markRead(String dest, ThreadKind kind, int messageId) async {
    reads += 1;
    lastReadDest = dest;
    lastReadMessageId = messageId;
  }

  void emitFriend({
    required String from,
    String nickname = '',
    bool accepted = false,
  }) {
    eventsController.add(
      KimEvent(
        kind: accepted ? KimEventKind.friendAccepted : KimEventKind.friend,
        dest: from,
        sender: from,
        nickname: nickname,
      ),
    );
  }

  void emitAuthExpired({String error = 'unauthorized'}) {
    eventsController.add(
      KimEvent(kind: KimEventKind.authExpired, error: error),
    );
  }

  void emitKick({String channelId = 'ch-1'}) {
    eventsController.add(KimEvent(kind: KimEventKind.kick, dest: channelId));
  }

  void emitSyncPage({required int pageId, required List<KimEvent> talks}) {
    eventsController.add(
      KimEvent(
        kind: KimEventKind.syncPage,
        pageId: pageId,
        talks: talks,
        pagePending: true,
      ),
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
  Future<List<Map<String, dynamic>>> roomEnter(
    String dest, {
    int kind = 0,
  }) async {
    return [
      {'account': dest, 'status': 2, 'lastSeen': 0},
    ];
  }

  @override
  Future<void> roomLeave(String dest, {int kind = 0}) async {}

  @override
  Future<void> sendTyping(
    String dest, {
    int kind = 0,
    bool active = true,
  }) async {
    lastTypingDest = dest;
    lastTypingActive = active;
    typingCalls += 1;
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

  int botCreates = 0;
  String lastBotCreateId = '';
  String lastBotCreateNickname = '';
  Object? botCreateError;
  final botReplies = <({String dest, String body, int inReplyTo})>[];
  var botReplyInFlight = 0;
  var botReplyMaxInFlight = 0;
  int botPendings = 0;
  List<KimBotPendingItem> pendingItems = const [];
  Duration? botReplyDelay;

  @override
  Future<KimPerson> botCreate({
    required String clientProfileId,
    required String nickname,
    String avatar = '',
    String bio = '',
  }) async {
    botCreates += 1;
    lastBotCreateId = clientProfileId;
    lastBotCreateNickname = nickname;
    final fail = botCreateError;
    if (fail != null) {
      throw fail;
    }
    final person = KimPerson(
      account: 'b_$clientProfileId',
      nickname: nickname,
      kind: ProfileKind.bot,
    );
    if (!friends.any((p) => p.account == person.account)) {
      friends = [...friends, person];
    }
    return person;
  }

  @override
  Future<void> botDelete(String dest) async {}

  @override
  Future<KimPerson> botUpdate({
    required String dest,
    required String nickname,
    String avatar = '',
    String bio = '',
  }) async {
    return KimPerson(
      account: dest,
      nickname: nickname,
      avatar: avatar,
      kind: ProfileKind.bot,
    );
  }

  @override
  Future<KimTalkResult> botReply({
    required String dest,
    required String body,
    required int inReplyTo,
    required String clientId,
  }) async {
    botReplyInFlight += 1;
    if (botReplyInFlight > botReplyMaxInFlight) {
      botReplyMaxInFlight = botReplyInFlight;
    }
    final delay = botReplyDelay;
    if (delay != null) {
      await Future<void>.delayed(delay);
    }
    botReplies.add((dest: dest, body: body, inReplyTo: inReplyTo));
    botReplyInFlight -= 1;
    return KimTalkResult(messageId: 100 + botReplies.length, sendTime: 1);
  }

  @override
  Future<List<KimBotPendingItem>> botPending(
    String dest, {
    int limit = 20,
  }) async {
    botPendings += 1;
    return pendingItems.take(limit).toList();
  }

  int attachStores = 0;
  String lastAttachPath = '';

  @override
  Future<void> attachStore(String dbPath) async {
    attachStores += 1;
    lastAttachPath = dbPath;
  }

  @override
  bool get rustStoreAttached => false;

  @override
  Future<void> persistTalks(
    Iterable<KimChatMsg> msgs, {
    required UnreadPolicy policy,
  }) async {}

  @override
  Future<void> persistInboxThreads(List<KimThread> threads) async {}

  int enqueues = 0;
  int retries = 0;
  int deletes = 0;
  int cancels = 0;
  String lastEnqueueDest = '';
  String lastEnqueueBody = '';
  int lastEnqueueKind = 0;
  String lastEnqueueMime = '';
  final enqueueIds = <String>[];

  @override
  Future<KimCommandReceipt> enqueueMessage({
    required String dest,
    required ThreadKind kind,
    required KimOutgoingContent content,
    required String clientId,
    String localPath = '',
    String mime = '',
    int width = 0,
    int height = 0,
    int byteSize = 0,
  }) async {
    enqueues += 1;
    lastEnqueueDest = dest;
    lastEnqueueKind = kind == ThreadKind.group ? 1 : 0;
    lastEnqueueMime = mime;
    lastClientId = clientId;
    enqueueIds.add(clientId);
    lastEnqueueBody = switch (content) {
      KimTextContent(:final text) => text,
      KimImageContent(:final url) => url,
      KimVideoContent(:final url) => url,
    };
    return KimCommandReceipt(
      requestId: 'req-$enqueues',
      clientId: clientId,
      dest: dest,
      acceptedAt: 1,
      sendStatus: 'pending',
    );
  }

  @override
  Future<void> cancelSend(String clientId) async {
    cancels += 1;
    lastClientId = clientId;
  }

  @override
  Future<KimCommandReceipt> retrySend(String clientId) async {
    retries += 1;
    lastClientId = clientId;
    return KimCommandReceipt(
      requestId: 'retry-$retries',
      clientId: clientId,
      dest: lastEnqueueDest,
      acceptedAt: 1,
      sendStatus: 'pending',
    );
  }

  @override
  Future<void> deleteThread(String dest) async {
    deletes += 1;
    lastTalkDest = dest;
  }
}

class FakeKimMedia implements KimMediaPort {
  FakeKimMedia({this.url = 'https://media.kim.ainexc.com/alice/a.jpg'});

  final String url;
  int uploads = 0;
  List<int> lastBytes = const [];
  String lastType = '';
  Completer<void>? uploadHold;

  @override
  Future<UploadedObject> uploadImage({
    required String token,
    required List<int> bytes,
    required String contentType,
  }) async {
    uploads += 1;
    lastBytes = bytes;
    lastType = contentType;
    final hold = uploadHold;
    if (hold != null) {
      await hold.future;
    }
    return UploadedObject(
      key: 'alice/a.jpg',
      url: url,
      contentType: contentType,
      bytes: bytes.length,
    );
  }
}
