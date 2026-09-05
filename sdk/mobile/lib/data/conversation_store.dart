/// Local thread + message cache. SQLite on disk; UI persistence only.
/// Talk still goes through kim-client.
library;

import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:isolate';

import 'package:path/path.dart' as p;
import 'package:shared_preferences/shared_preferences.dart';
import 'package:sqlite3/sqlite3.dart';

import '../core/image_extra.dart';
import '../models/models.dart';
import 'message_identity.dart';

enum UnreadPolicy { keep, ifInserted }

class MessageCursor {
  const MessageCursor({required this.at, required this.key});

  final int at;
  final String key;
}

class ApplyResult {
  const ApplyResult({
    required this.message,
    required this.inserted,
    required this.unreadDelta,
    required this.thread,
  });

  final KimChatMsg message;
  final bool inserted;
  final int unreadDelta;
  final KimThread thread;
}

const _msgCols =
    'key, dest, sender, body, at, sys, failed, kind, width, height, '
    'message_id, batch_id, status, local_path';

class ConversationStore {
  ConversationStore._(this._db) : _port = null;

  ConversationStore._remote(this._port) : _db = null;

  final Database? _db;
  final SendPort? _port;
  final Map<String, List<KimThread>> _threadCache = {};
  final Map<String, List<KimChatMsg>> _msgCache = {};
  var _seq = 0;

  bool get isolateBacked => _port != null;

  static const maxMessages = 400;
  static const _dbName = 'kim-cache.db';
  static const _prefsFlag = 'prefs_imported';
  static const _threadPrefix = 'kim.threads.';
  static const _msgPrefix = 'kim.msgs.';

  Database get _engine {
    final db = _db;
    if (db == null) {
      throw StateError('isolate store: use async apply/ensure');
    }
    return db;
  }

  static ConversationStore memory() {
    return ConversationStore._(_openAndMigrate(sqlite3.openInMemory()));
  }

  static Future<ConversationStore> open({
    required Directory support,
    SharedPreferences? prefs,
    bool isolate = true,
  }) async {
    await support.create(recursive: true);
    final file = File(p.join(support.path, _dbName));
    if (!isolate) {
      final store = ConversationStore._(
        _openAndMigrate(sqlite3.open(file.path)),
      );
      if (prefs != null) {
        await store._importPrefs(prefs);
      }
      return store;
    }
    try {
      final store = await _spawnIsolate(file.path);
      if (prefs != null) {
        await store._importPrefs(prefs);
      }
      return store;
    } catch (_) {
      final store = ConversationStore._(
        _openAndMigrate(sqlite3.open(file.path)),
      );
      if (prefs != null) {
        await store._importPrefs(prefs);
      }
      return store;
    }
  }

  static Database _openAndMigrate(Database db) {
    db.execute('PRAGMA foreign_keys = ON');
    db.select('PRAGMA busy_timeout = 3000');
    db.select('PRAGMA journal_mode = WAL');
    db.execute('''
      CREATE TABLE IF NOT EXISTS meta (
        key TEXT PRIMARY KEY NOT NULL,
        value TEXT NOT NULL
      )
    ''');
    db.execute('''
      CREATE TABLE IF NOT EXISTS threads (
        account TEXT NOT NULL,
        id TEXT NOT NULL,
        kind TEXT NOT NULL,
        title TEXT NOT NULL,
        last_body TEXT NOT NULL DEFAULT '',
        last_at INTEGER NOT NULL DEFAULT 0,
        unread INTEGER NOT NULL DEFAULT 0,
        PRIMARY KEY (account, id)
      )
    ''');
    db.execute('''
      CREATE TABLE IF NOT EXISTS messages (
        account TEXT NOT NULL,
        dest TEXT NOT NULL,
        key TEXT NOT NULL,
        sender TEXT NOT NULL,
        body TEXT NOT NULL,
        at INTEGER NOT NULL DEFAULT 0,
        sys INTEGER NOT NULL DEFAULT 0,
        failed INTEGER NOT NULL DEFAULT 0,
        kind TEXT NOT NULL DEFAULT 'text',
        width INTEGER NOT NULL DEFAULT 0,
        height INTEGER NOT NULL DEFAULT 0,
        PRIMARY KEY (account, dest, key)
      )
    ''');
    db.execute(
      'CREATE INDEX IF NOT EXISTS messages_by_thread ON messages (account, dest, at)',
    );
    _ensureColumn(db, 'threads', 'avatar', "TEXT NOT NULL DEFAULT ''");
    _ensureColumn(db, 'messages', 'message_id', 'INTEGER NOT NULL DEFAULT 0');
    _ensureColumn(db, 'messages', 'batch_id', "TEXT NOT NULL DEFAULT ''");
    _ensureColumn(db, 'messages', 'status', "TEXT NOT NULL DEFAULT 'sent'");
    _ensureColumn(db, 'messages', 'local_path', "TEXT NOT NULL DEFAULT ''");
    _mergeDuplicateMessageIds(db);
    db.execute(
      'CREATE INDEX IF NOT EXISTS messages_by_id ON messages (account, dest, message_id)',
    );
    db.execute(
      'CREATE UNIQUE INDEX IF NOT EXISTS messages_mid_unique '
      'ON messages (account, dest, message_id) WHERE message_id != 0',
    );
    db.execute(
      'CREATE INDEX IF NOT EXISTS messages_by_thread_key '
      'ON messages (account, dest, at DESC, key DESC)',
    );
    db.execute(
      'CREATE INDEX IF NOT EXISTS messages_pending '
      'ON messages (account, status, at, key)',
    );
    return db;
  }

  static void _mergeDuplicateMessageIds(Database db) {
    final groups = db.select(
      'SELECT account, dest, message_id FROM messages '
      'WHERE message_id != 0 '
      'GROUP BY account, dest, message_id HAVING COUNT(*) > 1',
    );
    for (final g in groups) {
      final account = g['account'] as String;
      final dest = g['dest'] as String;
      final mid = g['message_id'] as int;
      final rows = db.select(
        'SELECT $_msgCols FROM messages '
        'WHERE account = ? AND dest = ? AND message_id = ?',
        [account, dest, mid],
      );
      if (rows.length < 2) {
        continue;
      }
      final msgs = [for (final row in rows) _msg(row)];
      var survivor = msgs.first;
      for (final m in msgs.skip(1)) {
        if (preferKey(m.key, survivor.key) == m.key) {
          survivor = m;
        }
      }
      for (final m in msgs) {
        if (m.key == survivor.key) {
          continue;
        }
        db.execute(
          'DELETE FROM messages WHERE account = ? AND dest = ? AND key = ?',
          [account, dest, m.key],
        );
      }
    }
  }

  static void _ensureColumn(
    Database db,
    String table,
    String column,
    String spec,
  ) {
    final rows = db.select('PRAGMA table_info($table)');
    for (final row in rows) {
      if (row['name'] == column) {
        return;
      }
    }
    db.execute('ALTER TABLE $table ADD COLUMN $column $spec');
  }

  List<KimThread> loadThreads(String account) {
    if (account.isEmpty) {
      return const [];
    }
    if (_port != null) {
      return List<KimThread>.from(_threadCache[account] ?? const []);
    }
    return _loadThreadsDb(_engine, account);
  }

  static List<KimThread> _loadThreadsDb(Database db, String account) {
    final rows = db.select(
      'SELECT id, kind, title, last_body, last_at, unread, avatar '
      'FROM threads WHERE account = ? ORDER BY last_at DESC, title COLLATE NOCASE',
      [account],
    );
    return [for (final row in rows) _thread(row)];
  }

  Future<List<KimThread>> warmThreads(String account) async {
    if (account.isEmpty) {
      return const [];
    }
    if (_port != null) {
      final raw = await _call({'op': 'threads', 'account': account});
      final threads = [
        for (final row in raw as List<dynamic>) KimThread.fromJson(row)!,
      ];
      _threadCache[account] = threads;
      return threads;
    }
    return loadThreads(account);
  }

  Future<List<KimChatMsg>> ensureMessages(
    String account,
    String dest, {
    int limit = 50,
  }) async {
    if (_port != null) {
      final raw = await _call({
        'op': 'page',
        'account': account,
        'dest': dest,
        'limit': limit,
      });
      final rows = _msgsFromJson(raw);
      _mergeMsgCache(account, dest, rows);
      return loadMessagesPage(account, dest, limit: limit);
    }
    return loadMessagesPage(account, dest, limit: limit);
  }

  Future<List<KimChatMsg>> loadPendingAsync(String account) async {
    if (_port != null) {
      return _msgsFromJson(await _call({'op': 'pending', 'account': account}));
    }
    return loadPending(account);
  }

  Future<List<KimChatMsg>> loadFailedAsync(String account) async {
    if (_port != null) {
      return _msgsFromJson(await _call({'op': 'failed', 'account': account}));
    }
    return loadFailed(account);
  }

  Future<void> saveThreads(String account, List<KimThread> threads) async {
    if (account.isEmpty) {
      return;
    }
    if (_port != null) {
      await _call({
        'op': 'saveThreads',
        'account': account,
        'threads': [for (final t in threads) t.toJson()],
      });
      _threadCache[account] = [
        for (final t in threads)
          t.copyWith(lastBody: previewSnippet(t.lastBody)),
      ]..sort((a, b) => b.lastAt.compareTo(a.lastAt));
      return;
    }
    _saveThreadsDb(account, threads);
  }

  void _saveThreadsDb(String account, List<KimThread> threads) {
    _engine.execute('BEGIN IMMEDIATE');
    try {
      _engine.execute('DELETE FROM threads WHERE account = ?', [account]);
      final insert = _engine.prepare(
        'INSERT INTO threads (account, id, kind, title, last_body, last_at, unread, avatar) '
        'VALUES (?, ?, ?, ?, ?, ?, ?, ?)',
      );
      try {
        for (final t in threads) {
          insert.execute([
            account,
            t.id,
            t.kind.name,
            t.title,
            previewSnippet(t.lastBody),
            t.lastAt,
            t.unread,
            t.avatar,
          ]);
        }
      } finally {
        insert.close();
      }
      _engine.execute('COMMIT');
    } catch (_) {
      _engine.execute('ROLLBACK');
      rethrow;
    }
  }

  List<KimChatMsg> loadMessages(String account, String dest) {
    if (account.isEmpty || dest.isEmpty) {
      return const [];
    }
    if (_port != null) {
      final rows = List<KimChatMsg>.from(
        _msgCache['$account|$dest'] ?? const [],
      );
      rows.sort((a, b) {
        final byAt = a.at.compareTo(b.at);
        if (byAt != 0) {
          return byAt;
        }
        return a.key.compareTo(b.key);
      });
      return rows;
    }
    final rows = _engine.select(
      'SELECT $_msgCols '
      'FROM messages WHERE account = ? AND dest = ? ORDER BY at ASC, key ASC',
      [account, dest],
    );
    return [for (final row in rows) _msg(row)];
  }

  Future<void> saveMessages(
    String account,
    String dest,
    List<KimChatMsg> messages,
  ) async {
    if (account.isEmpty || dest.isEmpty) {
      return;
    }
    final clipped = messages.length > maxMessages
        ? messages.sublist(messages.length - maxMessages)
        : messages;
    if (_port != null) {
      await _call({
        'op': 'saveMessages',
        'account': account,
        'dest': dest,
        'msgs': [for (final m in clipped) m.toJson()],
      });
      _msgCache['$account|$dest'] = List<KimChatMsg>.from(clipped);
      return;
    }
    _saveMessagesDb(account, dest, clipped);
  }

  void _saveMessagesDb(String account, String dest, List<KimChatMsg> clipped) {
    _engine.execute('BEGIN IMMEDIATE');
    try {
      _engine.execute('DELETE FROM messages WHERE account = ? AND dest = ?', [
        account,
        dest,
      ]);
      final insert = _engine.prepare(
        'INSERT INTO messages (account, dest, key, sender, body, at, sys, failed, kind, width, height, message_id, batch_id, status, local_path) '
        'VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)',
      );
      try {
        for (final m in clipped) {
          insert.execute(_msgValues(account, dest, m));
        }
      } finally {
        insert.close();
      }
      _engine.execute('COMMIT');
    } catch (_) {
      _engine.execute('ROLLBACK');
      rethrow;
    }
  }

  Future<void> deleteThread(String account, String dest) async {
    if (_port != null) {
      await _call({'op': 'deleteThread', 'account': account, 'dest': dest});
      final threads = _threadCache[account];
      if (threads != null) {
        _threadCache[account] = threads.where((t) => t.id != dest).toList();
      }
      _msgCache.remove('$account|$dest');
      return;
    }
    _deleteThreadDb(account, dest);
  }

  void _deleteThreadDb(String account, String dest) {
    _engine.execute('BEGIN IMMEDIATE');
    try {
      _engine.execute('DELETE FROM messages WHERE account = ? AND dest = ?', [
        account,
        dest,
      ]);
      _engine.execute('DELETE FROM threads WHERE account = ? AND id = ?', [
        account,
        dest,
      ]);
      _engine.execute('COMMIT');
    } catch (_) {
      _engine.execute('ROLLBACK');
      rethrow;
    }
  }

  void close() {
    if (_port != null) {
      unawaited(_call({'op': 'close'}));
      return;
    }
    _engine.close();
  }

  static KimThread _thread(Row row) {
    return KimThread(
      id: row['id'] as String,
      kind: row['kind'] == 'group' ? ThreadKind.group : ThreadKind.user,
      title: row['title'] as String,
      lastBody: previewSnippet(row['last_body'] as String),
      lastAt: row['last_at'] as int,
      unread: row['unread'] as int,
      avatar: row['avatar'] as String? ?? '',
    );
  }

  static KimChatMsg _msg(Row row) {
    final kindRaw = row['kind'] as String;
    final statusRaw = row['status'] as String? ?? 'sent';
    final failed = (row['failed'] as int) == 1 || statusRaw == 'failed';
    final status = statusRaw == 'sending'
        ? KimSendStatus.sending
        : failed
        ? KimSendStatus.failed
        : KimSendStatus.sent;
    final batch = row['batch_id'] as String? ?? '';
    final local = row['local_path'] as String? ?? '';
    return KimChatMsg(
      key: row['key'] as String,
      dest: row['dest'] as String,
      sender: row['sender'] as String,
      body: row['body'] as String,
      at: row['at'] as int,
      sys: (row['sys'] as int) == 1,
      failed: failed,
      kind: kindRaw == 'video'
          ? KimMsgKind.video
          : kindRaw == 'image'
          ? KimMsgKind.image
          : KimMsgKind.text,
      width: row['width'] as int,
      height: row['height'] as int,
      messageId: row['message_id'] as int? ?? 0,
      batchId: batch.isEmpty ? null : batch,
      status: status,
      localPath: local.isEmpty ? null : local,
    );
  }

  static List<Object?> _msgValues(String account, String dest, KimChatMsg m) {
    return [
      account,
      dest,
      m.key,
      m.sender,
      m.body,
      m.at,
      m.sys ? 1 : 0,
      m.isFailed ? 1 : 0,
      m.kind.name,
      m.width,
      m.height,
      m.messageId,
      m.batchId ?? '',
      m.status.name,
      m.localPath ?? '',
    ];
  }

  Future<void> upsertThread(String account, KimThread t) async {
    if (account.isEmpty || t.id.isEmpty) {
      return;
    }
    if (_port != null) {
      await _call({
        'op': 'upsertThread',
        'account': account,
        'thread': t.toJson(),
      });
      _cacheThread(account, t);
      return;
    }
    _upsertThreadDb(account, t);
  }

  void _upsertThreadDb(String account, KimThread t) {
    _engine.execute(
      'INSERT INTO threads (account, id, kind, title, last_body, last_at, unread, avatar) '
      'VALUES (?, ?, ?, ?, ?, ?, ?, ?) '
      'ON CONFLICT(account, id) DO UPDATE SET '
      'kind = excluded.kind, '
      'title = excluded.title, '
      'last_body = excluded.last_body, '
      'last_at = excluded.last_at, '
      'unread = excluded.unread, '
      'avatar = CASE WHEN excluded.avatar != \'\' THEN excluded.avatar ELSE threads.avatar END',
      [
        account,
        t.id,
        t.kind.name,
        t.title,
        previewSnippet(t.lastBody),
        t.lastAt,
        t.unread,
        t.avatar,
      ],
    );
  }

  Future<void> upsertMessages(
    String account,
    String dest,
    Iterable<KimChatMsg> msgs,
  ) async {
    if (account.isEmpty || dest.isEmpty) {
      return;
    }
    final list = msgs.toList();
    if (list.isEmpty) {
      return;
    }
    if (_port != null) {
      await applyMessages(account, list, policy: UnreadPolicy.keep);
      return;
    }
    _upsertMessagesDb(account, dest, list);
  }

  void _upsertMessagesDb(String account, String dest, List<KimChatMsg> list) {
    _engine.execute('BEGIN IMMEDIATE');
    try {
      final insert = _engine.prepare(
        'INSERT INTO messages (account, dest, key, sender, body, at, sys, failed, kind, width, height, message_id, batch_id, status, local_path) '
        'VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) '
        'ON CONFLICT(account, dest, key) DO UPDATE SET '
        'sender = excluded.sender, '
        'body = excluded.body, '
        'at = excluded.at, '
        'sys = excluded.sys, '
        'failed = excluded.failed, '
        'kind = excluded.kind, '
        'width = excluded.width, '
        'height = excluded.height, '
        'message_id = CASE WHEN excluded.message_id != 0 THEN excluded.message_id ELSE messages.message_id END, '
        'batch_id = CASE WHEN excluded.batch_id != \'\' THEN excluded.batch_id ELSE messages.batch_id END, '
        'status = excluded.status, '
        'local_path = CASE WHEN excluded.local_path != \'\' THEN excluded.local_path ELSE messages.local_path END',
      );
      try {
        for (final m in list) {
          insert.execute(_msgValues(account, dest, m));
        }
      } finally {
        insert.close();
      }
      _engine.execute('COMMIT');
    } catch (_) {
      _engine.execute('ROLLBACK');
      rethrow;
    }
  }

  Future<List<ApplyResult>> applyMessages(
    String account,
    Iterable<KimChatMsg> msgs, {
    UnreadPolicy policy = UnreadPolicy.ifInserted,
    String? viewingDest,
  }) async {
    if (_port != null) {
      final raw = await _call({
        'op': 'apply',
        'account': account,
        'policy': policy.name,
        'viewing': viewingDest ?? '',
        'msgs': [for (final m in msgs) m.toJson()],
      });
      final list = raw as List<dynamic>;
      final results = [
        for (final row in list) _applyResultFromJson(row as Map),
      ];
      _rememberResults(account, results);
      return results;
    }
    final results = _applyDb(
      account,
      msgs,
      policy: policy,
      viewingDest: viewingDest,
    );
    _rememberResults(account, results);
    return results;
  }

  List<ApplyResult> _applyDb(
    String account,
    Iterable<KimChatMsg> msgs, {
    required UnreadPolicy policy,
    String? viewingDest,
  }) {
    if (account.isEmpty) {
      return const [];
    }
    final list = msgs.toList();
    if (list.isEmpty) {
      return const [];
    }
    _engine.execute('BEGIN IMMEDIATE');
    try {
      final out = <ApplyResult>[];
      for (final incoming in list) {
        if (incoming.dest.isEmpty) {
          continue;
        }
        out.add(
          _applyOne(
            account,
            incoming,
            policy: policy,
            viewingDest: viewingDest,
          ),
        );
      }
      final dests = {for (final r in out) r.message.dest};
      for (final dest in dests) {
        _pruneThread(account, dest);
      }
      _engine.execute('COMMIT');
      return out;
    } catch (_) {
      _engine.execute('ROLLBACK');
      rethrow;
    }
  }

  ApplyResult _applyOne(
    String account,
    KimChatMsg incoming, {
    required UnreadPolicy policy,
    String? viewingDest,
  }) {
    final dest = incoming.dest;
    KimChatMsg? byMid;
    if (incoming.messageId != 0) {
      byMid = _findByMid(account, dest, incoming.messageId);
    }
    final byKey = _findByKey(account, dest, incoming.key);
    KimChatMsg? survivor;
    KimChatMsg? loser;
    if (byMid != null && byKey != null && byMid.key != byKey.key) {
      if (preferKey(byKey.key, byMid.key) == byKey.key) {
        survivor = byKey;
        loser = byMid;
      } else {
        survivor = byMid;
        loser = byKey;
      }
    } else {
      survivor = byMid ?? byKey;
    }
    final inserted = survivor == null;
    final merged = survivor == null
        ? incoming
        : _mergeStored(survivor, incoming);
    if (loser != null && loser.key != merged.key) {
      _engine.execute(
        'DELETE FROM messages WHERE account = ? AND dest = ? AND key = ?',
        [account, dest, loser.key],
      );
    }
    if (survivor != null && survivor.key != merged.key) {
      _engine.execute(
        'DELETE FROM messages WHERE account = ? AND dest = ? AND key = ?',
        [account, dest, survivor.key],
      );
    }
    _putMsg(account, dest, merged);
    var unreadDelta = 0;
    if (policy == UnreadPolicy.ifInserted &&
        inserted &&
        !merged.sys &&
        merged.sender != account &&
        merged.dest != viewingDest) {
      unreadDelta = 1;
    }
    final thread = _touchThread(
      account,
      merged,
      unreadDelta: unreadDelta,
      viewing: viewingDest == dest,
    );
    return ApplyResult(
      message: merged,
      inserted: inserted,
      unreadDelta: unreadDelta,
      thread: thread,
    );
  }

  KimChatMsg? _findByMid(String account, String dest, int mid) {
    final rows = _engine.select(
      'SELECT $_msgCols FROM messages '
      'WHERE account = ? AND dest = ? AND message_id = ? AND message_id != 0 LIMIT 1',
      [account, dest, mid],
    );
    if (rows.isEmpty) {
      return null;
    }
    return _msg(rows.first);
  }

  KimChatMsg? _findByKey(String account, String dest, String key) {
    final rows = _engine.select(
      'SELECT $_msgCols FROM messages '
      'WHERE account = ? AND dest = ? AND key = ? LIMIT 1',
      [account, dest, key],
    );
    if (rows.isEmpty) {
      return null;
    }
    return _msg(rows.first);
  }

  void _putMsg(String account, String dest, KimChatMsg m) {
    _engine.execute(
      'INSERT INTO messages (account, dest, key, sender, body, at, sys, failed, kind, width, height, message_id, batch_id, status, local_path) '
      'VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) '
      'ON CONFLICT(account, dest, key) DO UPDATE SET '
      'sender = excluded.sender, '
      'body = excluded.body, '
      'at = excluded.at, '
      'sys = excluded.sys, '
      'failed = excluded.failed, '
      'kind = excluded.kind, '
      'width = excluded.width, '
      'height = excluded.height, '
      'message_id = CASE WHEN excluded.message_id != 0 THEN excluded.message_id ELSE messages.message_id END, '
      'batch_id = CASE WHEN excluded.batch_id != \'\' THEN excluded.batch_id ELSE messages.batch_id END, '
      'status = excluded.status, '
      'local_path = CASE WHEN excluded.local_path != \'\' THEN excluded.local_path ELSE messages.local_path END',
      _msgValues(account, dest, m),
    );
  }

  static KimChatMsg _mergeStored(KimChatMsg prev, KimChatMsg next) {
    final body = isRemoteUrl(prev.body) && !isRemoteUrl(next.body)
        ? prev.body
        : next.body;
    return KimChatMsg(
      key: prev.key,
      dest: prev.dest,
      sender: next.sender.isEmpty ? prev.sender : next.sender,
      body: body,
      at: next.at == 0 ? prev.at : next.at,
      sys: next.sys,
      failed: next.failed,
      kind: next.kind,
      width: next.width == 0 ? prev.width : next.width,
      height: next.height == 0 ? prev.height : next.height,
      messageId: next.messageId == 0 ? prev.messageId : next.messageId,
      batchId: (next.batchId == null || next.batchId!.isEmpty)
          ? prev.batchId
          : next.batchId,
      status: next.status,
      localPath: (next.localPath == null || next.localPath!.isEmpty)
          ? prev.localPath
          : next.localPath,
    );
  }

  KimThread _touchThread(
    String account,
    KimChatMsg msg, {
    required int unreadDelta,
    required bool viewing,
  }) {
    final existing = _findThread(account, msg.dest);
    final lastAt = existing == null
        ? msg.at
        : (msg.at >= existing.lastAt ? msg.at : existing.lastAt);
    final lastBody = msg.sys
        ? (existing?.lastBody ?? '')
        : (existing == null || msg.at >= existing.lastAt
              ? previewBody(msg)
              : existing.lastBody);
    final unread = viewing ? 0 : (existing?.unread ?? 0) + unreadDelta;
    final thread = KimThread(
      id: msg.dest,
      kind: existing?.kind ?? ThreadKind.user,
      title: existing?.title ?? msg.dest,
      lastBody: lastBody,
      lastAt: lastAt,
      unread: unread < 0 ? 0 : unread,
      avatar: existing?.avatar ?? '',
    );
    _engine.execute(
      'INSERT INTO threads (account, id, kind, title, last_body, last_at, unread, avatar) '
      'VALUES (?, ?, ?, ?, ?, ?, ?, ?) '
      'ON CONFLICT(account, id) DO UPDATE SET '
      'last_body = excluded.last_body, '
      'last_at = excluded.last_at, '
      'unread = excluded.unread, '
      'avatar = CASE WHEN excluded.avatar != \'\' THEN excluded.avatar ELSE threads.avatar END',
      [
        account,
        thread.id,
        thread.kind.name,
        thread.title,
        thread.lastBody,
        thread.lastAt,
        thread.unread,
        thread.avatar,
      ],
    );
    return thread;
  }

  KimThread? _findThread(String account, String id) {
    final rows = _engine.select(
      'SELECT id, kind, title, last_body, last_at, unread, avatar '
      'FROM threads WHERE account = ? AND id = ?',
      [account, id],
    );
    if (rows.isEmpty) {
      return null;
    }
    return _thread(rows.first);
  }

  void _pruneThread(String account, String dest) {
    _engine.execute(
      'DELETE FROM messages WHERE account = ? AND dest = ? AND key NOT IN ('
      'SELECT key FROM messages WHERE account = ? AND dest = ? '
      'ORDER BY at DESC, key DESC LIMIT ?)',
      [account, dest, account, dest, maxMessages],
    );
  }

  Future<void> markThreadRead(String account, String dest) async {
    if (account.isEmpty || dest.isEmpty) {
      return;
    }
    if (_port != null) {
      await _call({'op': 'markRead', 'account': account, 'dest': dest});
      final cached = _threadCache[account];
      if (cached != null) {
        _threadCache[account] = [
          for (final t in cached)
            if (t.id == dest) t.copyWith(unread: 0) else t,
        ];
      }
      return;
    }
    _engine.execute(
      'UPDATE threads SET unread = 0 WHERE account = ? AND id = ?',
      [account, dest],
    );
  }

  List<KimChatMsg> loadMessagesPage(
    String account,
    String dest, {
    int? beforeAt,
    String? beforeKey,
    int limit = 50,
  }) {
    if (account.isEmpty || dest.isEmpty) {
      return const [];
    }
    if (_port != null) {
      return _pageFromCache(
        account,
        dest,
        beforeAt: beforeAt,
        beforeKey: beforeKey,
        limit: limit,
      );
    }
    return _loadPageDb(
      _engine,
      account,
      dest,
      beforeAt: beforeAt,
      beforeKey: beforeKey,
      limit: limit,
    );
  }

  static List<KimChatMsg> _loadPageDb(
    Database db,
    String account,
    String dest, {
    int? beforeAt,
    String? beforeKey,
    int limit = 50,
  }) {
    final ResultSet rows;
    if (beforeAt == null) {
      rows = db.select(
        'SELECT $_msgCols FROM messages WHERE account = ? AND dest = ? '
        'ORDER BY at DESC, key DESC LIMIT ?',
        [account, dest, limit],
      );
    } else if (beforeKey != null && beforeKey.isNotEmpty) {
      rows = db.select(
        'SELECT $_msgCols FROM messages WHERE account = ? AND dest = ? '
        'AND (at < ? OR (at = ? AND key < ?)) '
        'ORDER BY at DESC, key DESC LIMIT ?',
        [account, dest, beforeAt, beforeAt, beforeKey, limit],
      );
    } else {
      rows = db.select(
        'SELECT $_msgCols FROM messages WHERE account = ? AND dest = ? AND at < ? '
        'ORDER BY at DESC, key DESC LIMIT ?',
        [account, dest, beforeAt, limit],
      );
    }
    return [for (final row in rows) _msg(row)];
  }

  List<KimChatMsg> loadPending(String account) {
    return _loadByStatus(account, 'sending');
  }

  List<KimChatMsg> loadFailed(String account) {
    return _loadByStatus(account, 'failed');
  }

  List<KimChatMsg> _loadByStatus(String account, String status) {
    if (account.isEmpty) {
      return const [];
    }
    if (_port != null) {
      final out = <KimChatMsg>[];
      for (final rows in _msgCache.values) {
        for (final m in rows) {
          if (m.status.name == status) {
            out.add(m);
          }
        }
      }
      out.sort((a, b) {
        final byAt = a.at.compareTo(b.at);
        if (byAt != 0) {
          return byAt;
        }
        return a.key.compareTo(b.key);
      });
      return out;
    }
    final rows = _engine.select(
      'SELECT $_msgCols '
      'FROM messages WHERE account = ? AND status = ? '
      'ORDER BY at ASC, key ASC',
      [account, status],
    );
    return [for (final row in rows) _msg(row)];
  }

  Future<void> _importPrefs(SharedPreferences prefs) async {
    if (_port != null) {
      final payload = <String, String>{};
      for (final key in prefs.getKeys()) {
        if (key.startsWith(_threadPrefix) || key.startsWith(_msgPrefix)) {
          payload[key] = prefs.getString(key) ?? '';
        }
      }
      if (payload.isEmpty) {
        return;
      }
      await _call({'op': 'importPrefs', 'data': payload});
      return;
    }
    final done = _engine.select('SELECT value FROM meta WHERE key = ?', [
      _prefsFlag,
    ]);
    if (done.isNotEmpty) {
      return;
    }
    for (final key in prefs.getKeys()) {
      if (key.startsWith(_threadPrefix)) {
        final account = key.substring(_threadPrefix.length);
        if (account.isEmpty) {
          continue;
        }
        await saveThreads(account, _decodeThreads(prefs.getString(key)));
      }
    }
    for (final key in prefs.getKeys()) {
      if (!key.startsWith(_msgPrefix)) {
        continue;
      }
      final rest = key.substring(_msgPrefix.length);
      final dot = rest.indexOf('.');
      if (dot <= 0 || dot == rest.length - 1) {
        continue;
      }
      final account = rest.substring(0, dot);
      final dest = rest.substring(dot + 1);
      await saveMessages(account, dest, _decodeMessages(prefs.getString(key)));
    }
    _engine.execute('INSERT OR REPLACE INTO meta (key, value) VALUES (?, ?)', [
      _prefsFlag,
      '1',
    ]);
  }

  void _cacheThread(String account, KimThread thread) {
    final next = thread.copyWith(lastBody: previewSnippet(thread.lastBody));
    final prev = _threadCache[account] ?? const <KimThread>[];
    _threadCache[account] = [next, ...prev.where((t) => t.id != next.id)]
      ..sort((a, b) => b.lastAt.compareTo(a.lastAt));
  }

  void _rememberResults(String account, List<ApplyResult> results) {
    if (results.isEmpty) {
      return;
    }
    for (final r in results) {
      _mergeMsgCache(account, r.message.dest, [r.message]);
      _cacheThread(account, r.thread);
    }
  }

  void _mergeMsgCache(String account, String dest, List<KimChatMsg> incoming) {
    final k = '$account|$dest';
    final prev = _msgCache[k] ?? [];
    final byKey = {for (final m in prev) m.key: m};
    for (final m in incoming) {
      KimChatMsg? match;
      if (m.messageId != 0) {
        for (final existing in byKey.values) {
          if (existing.messageId == m.messageId) {
            match = existing;
            break;
          }
        }
      }
      match ??= byKey[m.key];
      if (match != null && match.key != m.key) {
        byKey.remove(match.key);
      }
      final kept = match == null ? m : _mergeStored(match, m);
      byKey[kept.key] = kept;
    }
    _msgCache[k] = byKey.values.toList();
  }

  List<KimChatMsg> _pageFromCache(
    String account,
    String dest, {
    int? beforeAt,
    String? beforeKey,
    int limit = 50,
  }) {
    final rows = List<KimChatMsg>.from(_msgCache['$account|$dest'] ?? const []);
    rows.sort((a, b) {
      final byAt = b.at.compareTo(a.at);
      if (byAt != 0) {
        return byAt;
      }
      return b.key.compareTo(a.key);
    });
    Iterable<KimChatMsg> filtered = rows;
    if (beforeAt != null) {
      filtered = rows.where((m) {
        if (m.at < beforeAt) {
          return true;
        }
        if (beforeKey != null && beforeKey.isNotEmpty) {
          return m.at == beforeAt && m.key.compareTo(beforeKey) < 0;
        }
        return false;
      });
    }
    return filtered.take(limit).toList();
  }

  static List<KimChatMsg> _msgsFromJson(Object? raw) {
    if (raw is! List) {
      return const [];
    }
    return [
      for (final row in raw)
        if (KimChatMsg.fromJson(row) != null) KimChatMsg.fromJson(row)!,
    ];
  }

  static ApplyResult _applyResultFromJson(Map row) {
    return ApplyResult(
      message: KimChatMsg.fromJson(row['message'])!,
      inserted: row['inserted'] == true,
      unreadDelta: row['unreadDelta'] is int ? row['unreadDelta'] as int : 0,
      thread: KimThread.fromJson(row['thread'])!,
    );
  }

  static Map<String, Object?> _applyResultToJson(ApplyResult r) {
    return {
      'message': r.message.toJson(),
      'inserted': r.inserted,
      'unreadDelta': r.unreadDelta,
      'thread': r.thread.toJson(),
    };
  }

  static Future<ConversationStore> _spawnIsolate(String path) async {
    final ready = ReceivePort();
    await Isolate.spawn(_conversationIsolateMain, ready.sendPort);
    final port = await ready.first as SendPort;
    final store = ConversationStore._remote(port);
    await store._call({'op': 'open', 'path': path});
    return store;
  }

  Future<Object?> _call(Map<String, Object?> cmd) async {
    final port = _port;
    if (port == null) {
      throw StateError('not isolate-backed');
    }
    final reply = ReceivePort();
    final id = ++_seq;
    port.send({...cmd, 'id': id, 'reply': reply.sendPort});
    final raw = await reply.first;
    reply.close();
    if (raw is Map && raw['err'] != null) {
      throw StateError('${raw['err']}');
    }
    if (raw is Map) {
      return raw['ok'];
    }
    return raw;
  }

  Object? _dispatch(Map cmd) {
    final op = cmd['op'] as String? ?? '';
    switch (op) {
      case 'open':
        return 'ok';
      case 'apply':
        final account = cmd['account'] as String? ?? '';
        final viewing = cmd['viewing'] as String? ?? '';
        final policy = cmd['policy'] == UnreadPolicy.keep.name
            ? UnreadPolicy.keep
            : UnreadPolicy.ifInserted;
        final msgs = _msgsFromJson(cmd['msgs']);
        final results = _applyDb(
          account,
          msgs,
          policy: policy,
          viewingDest: viewing.isEmpty ? null : viewing,
        );
        return [for (final r in results) _applyResultToJson(r)];
      case 'threads':
        final account = cmd['account'] as String? ?? '';
        return [for (final t in _loadThreadsDb(_engine, account)) t.toJson()];
      case 'page':
        final account = cmd['account'] as String? ?? '';
        final dest = cmd['dest'] as String? ?? '';
        final limit = cmd['limit'] is int ? cmd['limit'] as int : 50;
        final beforeAt = cmd['beforeAt'] as int?;
        final beforeKey = cmd['beforeKey'] as String?;
        return [
          for (final m in _loadPageDb(
            _engine,
            account,
            dest,
            beforeAt: beforeAt,
            beforeKey: beforeKey,
            limit: limit,
          ))
            m.toJson(),
        ];
      case 'pending':
        return [
          for (final m in loadPending(cmd['account'] as String? ?? ''))
            m.toJson(),
        ];
      case 'failed':
        return [
          for (final m in loadFailed(cmd['account'] as String? ?? ''))
            m.toJson(),
        ];
      case 'upsertThread':
        final account = cmd['account'] as String? ?? '';
        final thread = KimThread.fromJson(cmd['thread']);
        if (thread != null) {
          _upsertThreadDb(account, thread);
        }
        return 'ok';
      case 'saveThreads':
        _saveThreadsDb(cmd['account'] as String? ?? '', [
          for (final row in cmd['threads'] as List<dynamic>? ?? const [])
            if (KimThread.fromJson(row) != null) KimThread.fromJson(row)!,
        ]);
        return 'ok';
      case 'saveMessages':
        _saveMessagesDb(
          cmd['account'] as String? ?? '',
          cmd['dest'] as String? ?? '',
          _msgsFromJson(cmd['msgs']),
        );
        return 'ok';
      case 'deleteThread':
        _deleteThreadDb(
          cmd['account'] as String? ?? '',
          cmd['dest'] as String? ?? '',
        );
        return 'ok';
      case 'markRead':
        _engine.execute(
          'UPDATE threads SET unread = 0 WHERE account = ? AND id = ?',
          [cmd['account'], cmd['dest']],
        );
        return 'ok';
      case 'importPrefs':
        final data = cmd['data'];
        if (data is Map) {
          _importPrefsMap({
            for (final e in data.entries) '${e.key}': '${e.value}',
          });
        }
        return 'ok';
      case 'close':
        _engine.close();
        return 'ok';
      default:
        throw StateError('unknown op $op');
    }
  }

  void _importPrefsMap(Map<String, String> data) {
    final done = _engine.select('SELECT value FROM meta WHERE key = ?', [
      _prefsFlag,
    ]);
    if (done.isNotEmpty) {
      return;
    }
    for (final entry in data.entries) {
      if (entry.key.startsWith(_threadPrefix)) {
        final account = entry.key.substring(_threadPrefix.length);
        if (account.isEmpty) {
          continue;
        }
        final threads = _decodeThreads(entry.value);
        _engine.execute('DELETE FROM threads WHERE account = ?', [account]);
        for (final t in threads) {
          _engine.execute(
            'INSERT INTO threads (account, id, kind, title, last_body, last_at, unread, avatar) '
            'VALUES (?, ?, ?, ?, ?, ?, ?, ?)',
            [
              account,
              t.id,
              t.kind.name,
              t.title,
              previewSnippet(t.lastBody),
              t.lastAt,
              t.unread,
              t.avatar,
            ],
          );
        }
      }
    }
    for (final entry in data.entries) {
      if (!entry.key.startsWith(_msgPrefix)) {
        continue;
      }
      final rest = entry.key.substring(_msgPrefix.length);
      final dot = rest.indexOf('.');
      if (dot <= 0 || dot == rest.length - 1) {
        continue;
      }
      final account = rest.substring(0, dot);
      final dest = rest.substring(dot + 1);
      for (final m in _decodeMessages(entry.value)) {
        _putMsg(account, dest, m);
      }
    }
    _engine.execute('INSERT OR REPLACE INTO meta (key, value) VALUES (?, ?)', [
      _prefsFlag,
      '1',
    ]);
  }
}

void _conversationIsolateMain(SendPort ready) {
  final inbox = ReceivePort();
  ready.send(inbox.sendPort);
  ConversationStore? store;
  inbox.listen((raw) {
    if (raw is! Map) {
      return;
    }
    final reply = raw['reply'];
    if (reply is! SendPort) {
      return;
    }
    try {
      final op = raw['op'] as String? ?? '';
      if (op == 'open' || store == null) {
        final path = raw['path'] as String? ?? '';
        store = ConversationStore._(
          ConversationStore._openAndMigrate(
            path.isEmpty ? sqlite3.openInMemory() : sqlite3.open(path),
          ),
        );
      }
      final result = store!._dispatch(Map<String, Object?>.from(raw));
      reply.send({'ok': result});
    } catch (err) {
      reply.send({'err': err.toString()});
    }
  });
}

List<KimThread> _decodeThreads(String? raw) {
  if (raw == null || raw.isEmpty) {
    return const [];
  }
  try {
    final parsed = jsonDecode(raw);
    if (parsed is! List) {
      return const [];
    }
    final out = <KimThread>[];
    for (final row in parsed) {
      final t = KimThread.fromJson(row);
      if (t != null) {
        out.add(t);
      }
    }
    out.sort((a, b) => b.lastAt.compareTo(a.lastAt));
    return out;
  } catch (_) {
    return const [];
  }
}

List<KimChatMsg> _decodeMessages(String? raw) {
  if (raw == null || raw.isEmpty) {
    return const [];
  }
  try {
    final parsed = jsonDecode(raw);
    if (parsed is! List) {
      return const [];
    }
    final out = <KimChatMsg>[];
    for (final row in parsed) {
      final m = KimChatMsg.fromJson(row);
      if (m != null) {
        out.add(m);
      }
    }
    return out;
  } catch (_) {
    return const [];
  }
}
