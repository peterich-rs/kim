import 'dart:async';

import 'package:kim_mobile/core/media.dart';
import 'package:kim_mobile/bridge/kim_bridge.dart';
import 'package:kim_mobile/models/models.dart';
import 'package:kim_mobile/src/rust/api/types.dart';

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
  int reads = 0;
  int typingCalls = 0;
  String lastTypingDest = '';
  bool lastTypingActive = false;
  String lastReadDest = '';
  int lastReadMessageId = 0;
  int radioUps = 0;
  int foregrounds = 0;
  int friendRequests = 0;
  int friendRemoves = 0;
  String lastUserAgent = '';
  String lastOrigin = '';
  String lastAccount = '';
  String lastPassword = '';
  String lastTalkDest = '';
  String lastClientId = '';
  final List<String> clientIds = [];
  Completer<void>? sendHold;

  final snapshotCtrl = StreamController<SessionSnapshotDto>.broadcast();
  final sessionUpdateCtrl = StreamController<SessionUpdateDto>.broadcast();
  final timelines = <String, StreamController<TimelineUpdateDto>>{};
  SessionSnapshotDto snapshot = const SessionSnapshotDto(
    link: LinkStateDto.offline(),
    threads: [],
    unreadTotal: 0,
  );

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

  void pushSnapshot(SessionSnapshotDto s) {
    snapshot = s;
    snapshotCtrl.add(s);
  }

  void pushEvent(SessionUpdateDto e) {
    sessionUpdateCtrl.add(e);
  }

  void pushTimeline(String dest, TimelineUpdateDto u) {
    lastTimeline[dest] = u;
    timelines
        .putIfAbsent(dest, StreamController<TimelineUpdateDto>.broadcast)
        .add(u);
  }

  void fakeIncomingText({
    required String dest,
    required String body,
    int id = 1,
  }) {
    final thread = ThreadViewDto(
      id: dest,
      kind: 0,
      title: dest,
      avatar: '',
      lastBody: body,
      lastAt: id,
      unread: 1,
    );
    final rest = snapshot.threads.where((t) => t.id != dest).toList();
    pushSnapshot(
      SessionSnapshotDto(
        link: snapshot.link,
        lastError: snapshot.lastError,
        threads: [thread, ...rest],
        unreadTotal: snapshot.unreadTotal + 1,
      ),
    );
    pushTimeline(
      dest,
      TimelineUpdateDto.delta(
        delta: TimelineDeltaDto(
          dest: dest,
          fromVersion: BigInt.zero,
          toVersion: BigInt.one,
          upserts: [
            MessageViewDto(
              key: 'm$id',
              dest: dest,
              sender: dest,
              body: body,
              at: id,
              sys: false,
              kind: 1,
              width: 0,
              height: 0,
              messageId: id,
              sendStatus: SendStatusDto.sent,
            ),
          ],
          deletedKeys: const [],
        ),
      ),
    );
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
    pushSnapshot(
      SessionSnapshotDto(
        link: const LinkStateDto.online(),
        lastError: snapshot.lastError,
        threads: snapshot.threads,
        unreadTotal: snapshot.unreadTotal,
      ),
    );
  }

  @override
  Future<void> stopSession() async {
    pushSnapshot(
      SessionSnapshotDto(
        link: const LinkStateDto.offline(),
        threads: snapshot.threads,
        unreadTotal: snapshot.unreadTotal,
      ),
    );
  }

  @override
  Stream<SessionSnapshotDto> watchSessionSnapshot() async* {
    yield snapshot;
    yield* snapshotCtrl.stream;
  }

  @override
  Stream<SessionUpdateDto> watchSessionEvents() => sessionUpdateCtrl.stream;

  final lastTimeline = <String, TimelineUpdateDto>{};

  @override
  Stream<TimelineUpdateDto> watchThread(String dest, {int limit = 50}) async* {
    final last = lastTimeline[dest];
    if (last != null) {
      yield last;
    }
    yield* timelines
        .putIfAbsent(dest, StreamController<TimelineUpdateDto>.broadcast)
        .stream;
  }

  @override
  Future<MessagePageDto> loadOlder({
    required String dest,
    required int beforeAt,
    required String beforeKey,
    int beforeId = 0,
    int limit = 50,
  }) async {
    return MessagePageDto(dest: dest, messages: const [], hasMore: false);
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
  Future<void> markRead(String dest, ThreadKind kind, int messageId) async {
    reads += 1;
    lastReadDest = dest;
    lastReadMessageId = messageId;
  }

  void emitKick({String channelId = 'ch-1'}) {
    pushEvent(SessionUpdateDto.kickout(channelId: channelId));
  }

  void emitAuthExpired({String error = 'unauthorized'}) {
    pushEvent(SessionUpdateDto.authExpired(reason: error));
  }

  void emitFriend({
    required String from,
    String nickname = '',
    bool accepted = false,
  }) {
    if (accepted) {
      pushEvent(
        SessionUpdateDto.friendAccepted(from: from, nickname: nickname),
      );
    } else {
      pushEvent(SessionUpdateDto.friendRequest(from: from, nickname: nickname));
    }
  }

  @override
  Future<List<KimPerson>> friendList() async => friends;

  @override
  Future<List<KimPerson>> friendIncoming() async => incoming;

  List<KimPerson> friends = const [];
  List<KimPerson> incoming = const [];

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

  @override
  Future<void> friendRemove(String dest) async {
    friendRemoves += 1;
    friends = friends.where((p) => p.account != dest).toList();
  }

  KimPerson me = const KimPerson(account: 'alice', nickname: 'alice');
  String lastAvatar = '';

  @override
  Future<KimPerson> profile({String dest = ''}) async {
    if (dest.isEmpty || dest == me.account) {
      return me;
    }
    for (final p in friends) {
      if (p.account == dest) {
        return p;
      }
    }
    for (final p in incoming) {
      if (p.account == dest) {
        return p;
      }
    }
    return KimPerson(account: dest, nickname: dest);
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
    me = KimPerson(
      account: me.account,
      nickname: nickname,
      avatar: avatar,
      bio: bio,
    );
    return me;
  }

  int botCreates = 0;
  int botUpdates = 0;
  int botDeletes = 0;
  String lastBotCreateId = '';
  String lastBotCreateNickname = '';
  String lastBotCreateModel = '';
  String lastBotCreateThinking = '';
  int? lastBotCreateContextTokens;
  String lastBotUpdateDest = '';
  String lastBotUpdateModel = '';
  String lastBotUpdateThinking = '';
  int? lastBotUpdateContextTokens;
  Object? botCreateError;
  String lastBotDeleteDest = '';
  Object? botDeleteError;
  final botReplies = <({String dest, String body, int inReplyTo})>[];
  var botReplyInFlight = 0;
  var botReplyMaxInFlight = 0;
  int botPendings = 0;
  List<KimBotPendingItem> pendingItems = const [];
  Duration? botReplyDelay;
  int botTypings = 0;
  bool? lastBotTypingActive;
  String lastBotTypingDest = '';

  @override
  Future<KimPerson> botCreate({
    required String clientProfileId,
    required String nickname,
    String avatar = '',
    String bio = '',
    String model = '',
    String thinkingEffort = '',
    int? contextTokens,
    String visibility = '',
  }) async {
    botCreates += 1;
    lastBotCreateId = clientProfileId;
    lastBotCreateNickname = nickname;
    lastBotCreateModel = model;
    lastBotCreateThinking = thinkingEffort;
    lastBotCreateContextTokens = contextTokens;
    final fail = botCreateError;
    if (fail != null) {
      throw fail;
    }
    final person = KimPerson(
      account: 'b_$clientProfileId',
      nickname: nickname,
      bio: bio,
      kind: ProfileKind.bot,
    );
    if (!friends.any((p) => p.account == person.account)) {
      friends = [...friends, person];
    }
    return person;
  }

  @override
  Future<void> botDelete(String dest) async {
    botDeletes += 1;
    lastBotDeleteDest = dest;
    final fail = botDeleteError;
    if (fail != null) {
      throw fail;
    }
    friends = friends.where((p) => p.account != dest).toList();
  }

  @override
  Future<KimPerson> botUpdate({
    required String dest,
    required String nickname,
    String avatar = '',
    String bio = '',
    String model = '',
    String thinkingEffort = '',
    int? contextTokens,
    String visibility = '',
  }) async {
    botUpdates += 1;
    lastBotUpdateDest = dest;
    lastBotUpdateModel = model;
    lastBotUpdateThinking = thinkingEffort;
    lastBotUpdateContextTokens = contextTokens;
    return KimPerson(
      account: dest,
      nickname: nickname,
      avatar: avatar,
      bio: bio,
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

  @override
  Future<void> botTyping(
    String dest, {
    int kind = 0,
    bool active = true,
  }) async {
    botTypings += 1;
    lastBotTypingDest = dest;
    lastBotTypingActive = active;
  }

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
    final hold = sendHold;
    if (hold != null) {
      await hold.future;
    }
    final body = lastEnqueueBody;
    final failed = talkError != null;
    pushTimeline(
      dest,
      TimelineUpdateDto.delta(
        delta: TimelineDeltaDto(
          dest: dest,
          fromVersion: BigInt.zero,
          toVersion: BigInt.one,
          upserts: [
            MessageViewDto(
              key: clientId,
              dest: dest,
              sender: 'alice',
              body: body,
              at: 1,
              sys: false,
              kind: 1,
              width: width,
              height: height,
              messageId: 0,
              sendStatus: failed ? SendStatusDto.failed : SendStatusDto.pending,
              localPath: localPath.isEmpty ? null : localPath,
            ),
          ],
          deletedKeys: const [],
        ),
      ),
    );
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

  final tokenPersistCtrl = StreamController<TokenPersistDto>.broadcast();
  SettingsDto settings = const SettingsDto(
    wsUrl: 'wss://kim.ainexc.com/',
    httpOrigin: 'https://kim.ainexc.com',
    env: 'prod',
    locale: '',
    account: '',
  );

  @override
  Stream<TokenPersistDto> watchTokenPersist() => tokenPersistCtrl.stream;

  @override
  Future<SettingsDto> settingsGet() async => settings;

  @override
  Future<SettingsDto> settingsPatch({
    String? wsUrl,
    String? httpOrigin,
    String? env,
  }) async {
    settings = SettingsDto(
      wsUrl: wsUrl ?? settings.wsUrl,
      httpOrigin: httpOrigin ?? settings.httpOrigin,
      env: env ?? settings.env,
      locale: settings.locale,
      account: settings.account,
    );
    return settings;
  }

  @override
  Future<SettingsDto> importDeviceSettings({
    required String wsUrl,
    required String httpOrigin,
    String env = 'prod',
    String locale = '',
  }) async {
    if (settings.wsUrl.isEmpty || settings.wsUrl == 'wss://kim.ainexc.com/') {
      settings = SettingsDto(
        wsUrl: wsUrl,
        httpOrigin: httpOrigin,
        env: env,
        locale: locale,
        account: settings.account,
      );
    }
    return settings;
  }

  final agentRunCtrl = StreamController<AgentRunRequestDto>.broadcast();
  final submittedAgentRuns = <AgentRunResultDto>[];
  List<AgentProfileDto> agentProfiles = const [];

  @override
  Stream<AgentRunRequestDto> watchAgentRun() => agentRunCtrl.stream;

  @override
  Future<void> submitAgentRun(AgentRunResultDto result) async {
    submittedAgentRuns.add(result);
  }

  @override
  Future<List<AgentProfileDto>> listAgentProfiles() async => agentProfiles;

  @override
  Future<void> upsertAgentProfile(AgentProfileDto row) async {
    agentProfiles = [
      for (final p in agentProfiles)
        if (p.profileId != row.profileId) p,
      row,
    ];
  }

  @override
  Future<void> deleteAgentProfile(String profileId) async {
    agentProfiles = [
      for (final p in agentProfiles)
        if (p.profileId != profileId) p,
    ];
  }

  @override
  Future<void> importAgentProfiles(List<AgentProfileDto> rows) async {
    if (agentProfiles.isEmpty) {
      agentProfiles = rows;
    }
  }

  @override
  Future<CommandAckDto> command(UiCommandDto cmd) async {
    return const CommandAckDto(
      requestId: '',
      clientId: '',
      dest: '',
      acceptedAt: 0,
      sendStatus: SendStatusDto.sent,
    );
  }

  @override
  Future<List<MessageViewDto>> searchMessages(
    String query, {
    String? dest,
  }) async {
    return const [];
  }

  @override
  Future<LocalMediaDto> mediaFetch(String url) async {
    return const LocalMediaDto(localPath: '', byteSize: 0, width: 0, height: 0);
  }

  @override
  Future<LocalMediaDto> mediaUpload({
    required String path,
    required String mime,
    int width = 0,
    int height = 0,
    int byteSize = 0,
  }) async {
    return LocalMediaDto(
      localPath: path,
      byteSize: byteSize,
      width: width,
      height: height,
    );
  }

  @override
  Future<MetricsDto> metricsSnapshot() async {
    return MetricsDto(
      enqueueTotal: BigInt.zero,
      persistTalkTotal: BigInt.zero,
      epochDropTotal: BigInt.zero,
      storeWipeTotal: BigInt.zero,
    );
  }

  @override
  Future<List<PersonDto>> refreshContacts() async {
    final rows = [
      for (final p in friends)
        PersonDto(
          account: p.account,
          nickname: p.nickname,
          avatar: p.avatar,
          bio: p.bio,
          relation: 'friend',
          kind: p.kind,
        ),
      for (final p in incoming)
        PersonDto(
          account: p.account,
          nickname: p.nickname,
          avatar: p.avatar,
          bio: p.bio,
          relation: 'incoming',
          kind: p.kind,
        ),
    ];
    pushEvent(SessionUpdateDto.contactsChanged(contacts: rows));
    return rows;
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
