import 'dart:async';
import 'dart:convert';
import 'dart:typed_data';

import 'package:kim_mobile/core/media.dart';
import 'package:kim_mobile/bridge/kim_bridge.dart';
import 'package:kim_mobile/models/models.dart';
import 'package:kim_mobile/src/rust/api/handles.dart';
import 'package:kim_mobile/src/rust/api/types.dart'
    hide ProfileKind, Relation, ThreadKind;
import 'package:kim_mobile/src/rust/api/types.dart'
    as wire
    show ProfileKind, Relation, ThreadKind;

class FakeKim implements KimAuthPort, KimClientPort {
  /// Widget tests build a fresh [FakeKim] after [testRuntime] recorded the
  /// signed-in session. Empty constructor args inherit that session.
  static String inheritToken = '';
  static String inheritAccount = '';

  FakeKim({
    this.session,
    this.error,
    this.connectError,
    String token = '',
    String account = '',
  }) : storedToken = token.isNotEmpty ? token : inheritToken,
       signedInAccount = account.isNotEmpty ? account : inheritAccount;

  KimAuthSession? session;
  Object? error;
  Object? connectError;
  String? snapshotErrorOnConnect;
  Object? talkError;
  Object? watchThreadError;
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
  int visibilityGeneration = 0;
  bool visibilityForeground = false;
  String visibilityDest = '';
  ThreadKind visibilityKind = ThreadKind.user;
  int radioUps = 0;
  int foregrounds = 0;
  int friendRequests = 0;
  int friendRemoves = 0;
  String storedToken;
  String signedInAccount;
  String lastAccount = '';
  String lastPassword = '';
  String lastTalkDest = '';
  String lastClientId = '';
  final List<String> clientIds = [];
  Completer<void>? sendHold;
  bool autoPushEnqueueTimeline = true;

  final snapshotCtrl = StreamController<SessionSnapshot>.broadcast();
  final sessionUpdateCtrl = StreamController<SessionUpdate>.broadcast();
  final contactsCtrl = StreamController<ContactsSnapshot>.broadcast();
  final timelines = <String, StreamController<TimelineUpdate>>{};
  SessionSnapshot snapshot = const SessionSnapshot(
    link: LinkState.offline(),
    threads: [],
    unreadTotal: 0,
  );
  ContactsSnapshot contactsSnapshot = ContactsSnapshot(
    version: BigInt.zero,
    contacts: const [],
    syncError: null,
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

  void pushSnapshot(SessionSnapshot s) {
    snapshot = s;
    snapshotCtrl.add(s);
  }

  void pushEvent(SessionUpdate e) {
    sessionUpdateCtrl.add(e);
  }

  void pushContacts(ContactsSnapshot snapshot) {
    contactsSnapshot = snapshot;
    friends = [
      for (final person in snapshot.contacts)
        if (person.relation != wire.Relation.incoming)
          KimPerson(
            account: person.account,
            nickname: person.nickname,
            avatar: person.avatar,
            bio: person.bio,
            kind: person.kind == wire.ProfileKind.bot
                ? ProfileKind.bot
                : ProfileKind.user,
          ),
    ];
    incoming = [
      for (final person in snapshot.contacts)
        if (person.relation == wire.Relation.incoming)
          KimPerson(
            account: person.account,
            nickname: person.nickname,
            avatar: person.avatar,
            bio: person.bio,
            kind: person.kind == wire.ProfileKind.bot
                ? ProfileKind.bot
                : ProfileKind.user,
          ),
    ];
    contactsCtrl.add(snapshot);
  }

  void pushTimeline(String dest, TimelineUpdate u) {
    lastTimeline[dest] = u;
    timelines
        .putIfAbsent(dest, StreamController<TimelineUpdate>.broadcast)
        .add(u);
  }

  void fakeIncomingText({
    required String dest,
    required String body,
    int id = 1,
  }) {
    final thread = ThreadView(
      id: dest,
      kind: wire.ThreadKind.user,
      title: dest,
      avatar: '',
      lastBody: body,
      preview: ThreadPreview.text(snippet: body),
      lastAt: id,
      unread: 1,
    );
    final rest = snapshot.threads.where((t) => t.id != dest).toList();
    pushSnapshot(
      SessionSnapshot(
        link: snapshot.link,
        lastError: snapshot.lastError,
        threads: [thread, ...rest],
        unreadTotal: snapshot.unreadTotal + 1,
      ),
    );
    pushTimeline(
      dest,
      TimelineUpdate.delta(
        delta: TimelineDelta(
          dest: dest,
          fromVersion: BigInt.zero,
          toVersion: BigInt.one,
          upserts: [
            MessageView(
              key: 'm$id',
              dest: dest,
              sender: dest,
              body: body,
              at: id,
              sys: false,
              kind: MediaKind.text,
              width: 0,
              height: 0,
              messageId: id,
              sendStatus: SendStatus.sent,
            ),
          ],
          deletedKeys: const [],
        ),
      ),
    );
  }

  @override
  Future<KimAuthSession> login({
    required String account,
    required String password,
  }) async {
    logins += 1;
    lastAccount = account;
    lastPassword = password;
    return _run();
  }

  @override
  Future<KimAuthSession> register({
    required String account,
    required String password,
  }) async {
    registers += 1;
    lastAccount = account;
    lastPassword = password;
    return _run();
  }

  @override
  Future<void> startSession() async {
    connects += 1;
    if (snapshotErrorOnConnect != null) {
      pushSnapshot(
        SessionSnapshot(
          link: const LinkState.offline(),
          lastError: snapshotErrorOnConnect,
          threads: snapshot.threads,
          unreadTotal: snapshot.unreadTotal,
        ),
      );
      return;
    }
    if (connectError != null) {
      throw connectError!;
    }
    pushSnapshot(
      SessionSnapshot(
        link: const LinkState.online(),
        lastError: snapshot.lastError,
        threads: snapshot.threads,
        unreadTotal: snapshot.unreadTotal,
      ),
    );
  }

  @override
  Future<void> stopSession() async {
    pushSnapshot(
      SessionSnapshot(
        link: const LinkState.offline(),
        threads: snapshot.threads,
        unreadTotal: snapshot.unreadTotal,
      ),
    );
  }

  @override
  Stream<SessionSnapshot> watchSessionSnapshot() async* {
    yield snapshot;
    yield* snapshotCtrl.stream;
  }

  @override
  Stream<SessionUpdate> watchSessionEvents() => sessionUpdateCtrl.stream;

  @override
  Stream<ContactsSnapshot> watchContacts() async* {
    yield contactsSnapshot;
    yield* contactsCtrl.stream;
  }

  final lastTimeline = <String, TimelineUpdate>{};
  final olderTimeline = <String, List<MessageView>>{};

  /// Queues rows that [loadOlder] will expose through the next snapshot.
  void setOlderTimeline(String dest, List<MessageView> messages) {
    olderTimeline[dest] = List.of(messages);
  }

  @override
  Stream<TimelineUpdate> watchThread(String dest, {int limit = 50}) {
    final err = watchThreadError;
    if (err != null) {
      throw err;
    }
    return _watchThreadStream(dest);
  }

  Stream<TimelineUpdate> _watchThreadStream(String dest) async* {
    final last = lastTimeline[dest];
    if (last != null) {
      yield last;
    }
    yield* timelines
        .putIfAbsent(dest, StreamController<TimelineUpdate>.broadcast)
        .stream;
  }

  @override
  Future<void> loadOlder({required String dest}) async {
    final current = lastTimeline[dest];
    final queued = olderTimeline.remove(dest) ?? const <MessageView>[];
    if (current case TimelineUpdate_Snapshot(:final snapshot)) {
      final byKey = {
        for (final message in snapshot.messages) message.key: message,
      };
      for (final message in queued) {
        byKey[message.key] = message;
      }
      final messages = byKey.values.toList()
        ..sort((left, right) {
          final byAt = left.at.compareTo(right.at);
          return byAt != 0 ? byAt : left.key.compareTo(right.key);
        });
      pushTimeline(
        dest,
        TimelineUpdate.snapshot(
          snapshot: TimelineSnapshot(
            dest: snapshot.dest,
            version: snapshot.version + BigInt.one,
            messages: messages,
            pending: snapshot.pending,
            unread: snapshot.unread,
            lastReadMessageId: snapshot.lastReadMessageId,
            hasMore: false,
            loadingOlder: false,
            historyError: null,
          ),
        ),
      );
    }
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

  @override
  Future<void> markConversationRead(String dest, ThreadKind kind) async {
    await markRead(dest, kind, 0);
  }

  @override
  Future<void> setConversationVisibility({
    required int generation,
    required bool foreground,
    required String dest,
    required ThreadKind kind,
  }) async {
    visibilityGeneration = generation;
    visibilityForeground = foreground;
    visibilityDest = dest;
    visibilityKind = kind;
  }

  void emitKick({String channelId = 'ch-1'}) {
    pushEvent(SessionUpdate.kickout(channelId: channelId));
  }

  void emitAuthExpired({String error = 'unauthorized'}) {
    pushEvent(SessionUpdate.authExpired(reason: error));
  }

  void emitFriend({
    required String from,
    String nickname = '',
    bool accepted = false,
  }) {
    if (accepted) {
      pushEvent(SessionUpdate.friendAccepted(from: from, nickname: nickname));
    } else {
      pushEvent(SessionUpdate.friendRequest(from: from, nickname: nickname));
    }
  }

  @override
  Future<List<KimPerson>> friendList() async => friends;

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
    final contacts = [
      ...contactsSnapshot.contacts.where((p) => p.account != dest),
      Person(
        account: dest,
        nickname: dest,
        avatar: '',
        bio: '',
        relation: wire.Relation.outgoing,
        kind: wire.ProfileKind.user,
      ),
    ];
    pushContacts(
      ContactsSnapshot(
        version: contactsSnapshot.version + BigInt.one,
        contacts: contacts,
        syncError: contactsSnapshot.syncError,
      ),
    );
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
  Future<List<RoomMember>> roomEnter(String dest, ThreadKind kind) async {
    return [RoomMember(account: dest, status: 2, lastSeen: 0)];
  }

  @override
  Future<void> roomLeave(String dest, ThreadKind kind) async {}

  @override
  Future<void> sendTyping(
    String dest,
    ThreadKind kind, {
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
    required OutgoingContent content,
    String? clientId,
  }) async {
    enqueues += 1;
    lastEnqueueDest = dest;
    lastEnqueueKind = kind == ThreadKind.group ? 1 : 0;
    final id = clientId ?? 'fake-${enqueueIds.length + 1}';
    lastClientId = id;
    enqueueIds.add(id);
    lastEnqueueBody = switch (content) {
      OutgoingContent_Text(:final body) => body,
      OutgoingContent_Image(:final path) => path,
      OutgoingContent_Video(:final path) => path,
      OutgoingContent_Voice(:final path) => path,
    };
    final hold = sendHold;
    if (hold != null) {
      await hold.future;
    }
    final body = lastEnqueueBody;
    final failed = talkError != null;
    if (autoPushEnqueueTimeline) {
      pushTimeline(
        dest,
        TimelineUpdate.delta(
          delta: TimelineDelta(
            dest: dest,
            fromVersion: BigInt.zero,
            toVersion: BigInt.one,
            upserts: [
              MessageView(
                key: id,
                dest: dest,
                sender: 'alice',
                body: body,
                at: 1,
                sys: false,
                kind: MediaKind.text,
                width: 0,
                height: 0,
                messageId: 0,
                sendStatus: failed ? SendStatus.failed : SendStatus.pending,
              ),
            ],
            deletedKeys: const [],
          ),
        ),
      );
    }
    return KimCommandReceipt(
      requestId: 'req-$enqueues',
      clientId: id,
      dest: dest,
      acceptedAt: 1,
      sendStatus: SendStatus.pending,
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
      sendStatus: SendStatus.pending,
    );
  }

  @override
  Future<void> deleteThread(String dest) async {
    deletes += 1;
    lastTalkDest = dest;
  }

  Settings settings = const Settings(
    wsUrl: 'wss://kim.ainexc.com/',
    httpOrigin: 'https://kim.ainexc.com',
    env: 'prod',
    locale: '',
    account: '',
  );
  bool prefsImported = false;

  @override
  Future<bool> hasStoredToken() async => storedToken.trim().isNotEmpty;

  @override
  Future<String> storedAccount() async => signedInAccount;

  @override
  Future<void> storeAuth({
    required String token,
    required String account,
  }) async {
    storedToken = token;
    signedInAccount = account;
  }

  @override
  Future<void> clearAuth() async {
    storedToken = '';
    signedInAccount = '';
  }

  @override
  Future<void> authLogout() async {
    logouts += 1;
  }

  @override
  Future<void> authChangePassword({
    required String oldPassword,
    required String newPassword,
  }) async {}

  @override
  Future<Settings> settingsGet() async => settings;

  @override
  Future<Settings> settingsPatch({String? wsUrl, String? env}) async {
    final nextUrl = wsUrl ?? settings.wsUrl;
    settings = Settings(
      wsUrl: nextUrl,
      httpOrigin: nextUrl.contains('127.0.0.1')
          ? 'http://127.0.0.1:8080'
          : 'https://kim.ainexc.com',
      env: env ?? settings.env,
      locale: settings.locale,
      account: settings.account,
    );
    return settings;
  }

  @override
  Future<Settings> settingsPreset(SettingsPreset preset) async {
    final local = preset == SettingsPreset.local;
    settings = Settings(
      wsUrl: local ? 'ws://127.0.0.1:8001/ws' : 'wss://kim.ainexc.com/',
      httpOrigin: local ? 'http://127.0.0.1:8080' : 'https://kim.ainexc.com',
      env: local ? 'dev' : 'prod',
      locale: settings.locale,
      account: settings.account,
    );
    return settings;
  }

  @override
  Future<bool> settingsImported() async => prefsImported;

  List<AgentProfile> agentProfiles = const [];
  final sessionEvents = <SessionUpdate>[];

  @override
  Future<List<AgentProfile>> listAgentProfiles() async => agentProfiles;

  @override
  Future<void> upsertAgentProfile(String documentJson) async {
    final row = AgentProfile(
      profileId: 'local',
      nickname: '',
      serverAccount: '',
      documentJson: documentJson,
      placement: ProfilePlacement.local,
      updatedAt: 0,
    );
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

  List<ProviderAccount> providerAccounts = const [];
  final overlays = <String, DeviceOverlay>{};
  String flagsJson = '{}';

  @override
  Future<List<ProviderAccount>> listProviderAccounts() async =>
      providerAccounts;

  @override
  Future<void> upsertProviderAccount(ProviderAccount row) async {
    providerAccounts = [
      for (final a in providerAccounts)
        if (a.id != row.id) a,
      row,
    ];
  }

  @override
  Future<void> deleteProviderAccount(String id) async {
    providerAccounts = [
      for (final a in providerAccounts)
        if (a.id != id) a,
    ];
  }

  @override
  Future<DeviceOverlay?> getDeviceOverlay(String profileId) async =>
      overlays[profileId];

  @override
  Future<void> upsertDeviceOverlay(DeviceOverlay row) async {
    overlays[row.profileId] = row;
  }

  @override
  Future<AgentFlags> agentFlags() async {
    if (flagsJson.trim().isEmpty) {
      return const AgentFlags(multiProfile: false, serverIdentity: false);
    }
    final decoded = jsonDecode(flagsJson);
    if (decoded is! Map) {
      return const AgentFlags(multiProfile: false, serverIdentity: false);
    }
    return AgentFlags(
      multiProfile: decoded['multi_profile'] == true,
      serverIdentity: decoded['server_identity'] == true,
    );
  }

  @override
  Future<void> setAgentFlags(AgentFlags flags) async {
    flagsJson = jsonEncode({
      'multi_profile': flags.multiProfile,
      'server_identity': flags.serverIdentity,
    });
  }

  @override
  Future<void> syncAgentSpecs() async {}

  @override
  Future<List<MessageView>> searchMessages(String query, {String? dest}) async {
    return const [];
  }

  @override
  Future<LocalMedia> mediaFetch(String url) async {
    return const LocalMedia(localPath: '', byteSize: 0, width: 0, height: 0);
  }

  @override
  Future<LocalMedia> mediaUploadBytes({
    required Uint8List bytes,
    required String mime,
    int width = 0,
    int height = 0,
  }) async {
    return LocalMedia(
      localPath: 'mem',
      byteSize: bytes.length,
      width: width,
      height: height,
    );
  }

  @override
  Future<Metrics> metricsSnapshot() async {
    return Metrics(
      enqueueTotal: BigInt.zero,
      persistTalkTotal: BigInt.zero,
      epochDropTotal: BigInt.zero,
      storeWipeTotal: BigInt.zero,
    );
  }

  @override
  Future<void> storeAgentSecret({
    required String keyRef,
    required String secret,
  }) async {}

  @override
  Future<List<String>> catalogModelCache(String vendor) async {
    return vendorCache[vendor] ?? const [];
  }

  final vendorCache = <String, List<String>>{};

  @override
  Future<List<String>> fetchModels({
    required String vendorId,
    required String baseUrl,
    required String keyRef,
  }) async => const [];

  @override
  Future<CapabilityPreview> previewProfile(String profileId) async {
    return const CapabilityPreview(tools: [], warnings: []);
  }

  final grants = <String, String>{};

  @override
  Future<void> workspaceGrantRegister({
    required String profileId,
    required String path,
    String? bookmark,
  }) async {
    grants[profileId] = path;
    if (bookmark != null) {
      grantBookmarks[profileId] = bookmark;
    }
  }

  final grantBookmarks = <String, String>{};

  @override
  Future<String?> workspaceGrant(String profileId) async => grants[profileId];

  @override
  Future<String> workspaceGrantBookmark(String profileId) async =>
      grantBookmarks[profileId] ?? '';

  @override
  Future<String> ensureAgentSandbox(String profileId) async {
    return '/tmp/kim-agent/$profileId';
  }

  @override
  Future<void> respondAgentPermission({
    required String callId,
    required bool allow,
  }) async {}

  @override
  Stream<AgentPermissionEvent> watchAgentPermission() => const Stream.empty();

  @override
  Stream<AgentUiStatus> watchAgentUi() => const Stream.empty();

  @override
  Future<void> refreshContacts() async {
    final friendIds = {for (final p in friends) p.account};
    final rows = [
      for (final p in friends)
        Person(
          account: p.account,
          nickname: p.nickname,
          avatar: p.avatar,
          bio: p.bio,
          relation: wire.Relation.friend,
          kind: p.kind == ProfileKind.bot
              ? wire.ProfileKind.bot
              : wire.ProfileKind.user,
        ),
      for (final p in incoming)
        Person(
          account: p.account,
          nickname: p.nickname,
          avatar: p.avatar,
          bio: p.bio,
          relation: wire.Relation.incoming,
          kind: p.kind == ProfileKind.bot
              ? wire.ProfileKind.bot
              : wire.ProfileKind.user,
        ),
      for (final p in contactsSnapshot.contacts)
        if (p.relation == wire.Relation.outgoing &&
            !friendIds.contains(p.account))
          p,
    ];
    pushContacts(
      ContactsSnapshot(
        version: contactsSnapshot.version + BigInt.one,
        contacts: rows,
        syncError: contactsSnapshot.syncError,
      ),
    );
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
