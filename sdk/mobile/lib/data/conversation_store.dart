/// Local thread + message cache. SQLite on disk; UI persistence only.
/// Talk still goes through kim-client.
library;

import 'dart:convert';
import 'dart:io';

import 'package:path/path.dart' as p;
import 'package:shared_preferences/shared_preferences.dart';
import 'package:sqlite3/sqlite3.dart';

import '../models/models.dart';

class ConversationStore {
  ConversationStore._(this._db);

  static const maxMessages = 400;
  static const _dbName = 'kim-cache.db';
  static const _prefsFlag = 'prefs_imported';
  static const _threadPrefix = 'kim.threads.';
  static const _msgPrefix = 'kim.msgs.';

  final Database _db;

  static ConversationStore memory() {
    return ConversationStore._(_openAndMigrate(sqlite3.openInMemory()));
  }

  static Future<ConversationStore> open({
    required Directory support,
    SharedPreferences? prefs,
  }) async {
    await support.create(recursive: true);
    final file = File(p.join(support.path, _dbName));
    final store = ConversationStore._(_openAndMigrate(sqlite3.open(file.path)));
    if (prefs != null) {
      await store._importPrefs(prefs);
    }
    return store;
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
    db.execute(
      'CREATE INDEX IF NOT EXISTS messages_by_id ON messages (account, dest, message_id)',
    );
    return db;
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
    final rows = _db.select(
      'SELECT id, kind, title, last_body, last_at, unread, avatar '
      'FROM threads WHERE account = ? ORDER BY last_at DESC, title COLLATE NOCASE',
      [account],
    );
    return [for (final row in rows) _thread(row)];
  }

  Future<void> saveThreads(String account, List<KimThread> threads) async {
    if (account.isEmpty) {
      return;
    }
    _db.execute('BEGIN IMMEDIATE');
    try {
      _db.execute('DELETE FROM threads WHERE account = ?', [account]);
      final insert = _db.prepare(
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
            t.lastBody,
            t.lastAt,
            t.unread,
            t.avatar,
          ]);
        }
      } finally {
        insert.close();
      }
      _db.execute('COMMIT');
    } catch (_) {
      _db.execute('ROLLBACK');
      rethrow;
    }
  }

  List<KimChatMsg> loadMessages(String account, String dest) {
    if (account.isEmpty || dest.isEmpty) {
      return const [];
    }
    final rows = _db.select(
      'SELECT key, dest, sender, body, at, sys, failed, kind, width, height, '
      'message_id, batch_id, status '
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
    _db.execute('BEGIN IMMEDIATE');
    try {
      _db.execute('DELETE FROM messages WHERE account = ? AND dest = ?', [
        account,
        dest,
      ]);
      final insert = _db.prepare(
        'INSERT INTO messages (account, dest, key, sender, body, at, sys, failed, kind, width, height, message_id, batch_id, status) '
        'VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)',
      );
      try {
        for (final m in clipped) {
          insert.execute(_msgValues(account, dest, m));
        }
      } finally {
        insert.close();
      }
      _db.execute('COMMIT');
    } catch (_) {
      _db.execute('ROLLBACK');
      rethrow;
    }
  }

  Future<void> deleteThread(String account, String dest) async {
    _db.execute('BEGIN IMMEDIATE');
    try {
      _db.execute('DELETE FROM messages WHERE account = ? AND dest = ?', [
        account,
        dest,
      ]);
      _db.execute('DELETE FROM threads WHERE account = ? AND id = ?', [
        account,
        dest,
      ]);
      _db.execute('COMMIT');
    } catch (_) {
      _db.execute('ROLLBACK');
      rethrow;
    }
  }

  void close() {
    _db.close();
  }

  KimThread _thread(Row row) {
    return KimThread(
      id: row['id'] as String,
      kind: row['kind'] == 'group' ? ThreadKind.group : ThreadKind.user,
      title: row['title'] as String,
      lastBody: row['last_body'] as String,
      lastAt: row['last_at'] as int,
      unread: row['unread'] as int,
      avatar: row['avatar'] as String? ?? '',
    );
  }

  KimChatMsg _msg(Row row) {
    final kindRaw = row['kind'] as String;
    final statusRaw = row['status'] as String? ?? 'sent';
    final failed = (row['failed'] as int) == 1 || statusRaw == 'failed';
    final status = statusRaw == 'sending'
        ? KimSendStatus.sending
        : failed
        ? KimSendStatus.failed
        : KimSendStatus.sent;
    final batch = row['batch_id'] as String? ?? '';
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
    );
  }

  List<Object?> _msgValues(String account, String dest, KimChatMsg m) {
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
    ];
  }

  Future<void> upsertThread(String account, KimThread t) async {
    if (account.isEmpty || t.id.isEmpty) {
      return;
    }
    _db.execute(
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
        t.lastBody,
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
    _db.execute('BEGIN IMMEDIATE');
    try {
      final insert = _db.prepare(
        'INSERT INTO messages (account, dest, key, sender, body, at, sys, failed, kind, width, height, message_id, batch_id, status) '
        'VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) '
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
        'status = excluded.status',
      );
      try {
        for (final m in list) {
          insert.execute(_msgValues(account, dest, m));
        }
      } finally {
        insert.close();
      }
      _db.execute('COMMIT');
    } catch (_) {
      _db.execute('ROLLBACK');
      rethrow;
    }
  }

  Future<void> markThreadRead(String account, String dest) async {
    if (account.isEmpty || dest.isEmpty) {
      return;
    }
    _db.execute('UPDATE threads SET unread = 0 WHERE account = ? AND id = ?', [
      account,
      dest,
    ]);
  }

  List<KimChatMsg> loadMessagesPage(
    String account,
    String dest, {
    int? beforeAt,
    int limit = 50,
  }) {
    if (account.isEmpty || dest.isEmpty) {
      return const [];
    }
    final rows = beforeAt == null
        ? _db.select(
            'SELECT key, dest, sender, body, at, sys, failed, kind, width, height, '
            'message_id, batch_id, status '
            'FROM messages WHERE account = ? AND dest = ? '
            'ORDER BY at DESC, key DESC LIMIT ?',
            [account, dest, limit],
          )
        : _db.select(
            'SELECT key, dest, sender, body, at, sys, failed, kind, width, height, '
            'message_id, batch_id, status '
            'FROM messages WHERE account = ? AND dest = ? AND at < ? '
            'ORDER BY at DESC, key DESC LIMIT ?',
            [account, dest, beforeAt, limit],
          );
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
    final rows = _db.select(
      'SELECT key, dest, sender, body, at, sys, failed, kind, width, height, '
      'message_id, batch_id, status '
      'FROM messages WHERE account = ? AND status = ? '
      'ORDER BY at ASC, key ASC',
      [account, status],
    );
    return [for (final row in rows) _msg(row)];
  }

  Future<void> _importPrefs(SharedPreferences prefs) async {
    final done = _db.select('SELECT value FROM meta WHERE key = ?', [
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
    _db.execute('INSERT OR REPLACE INTO meta (key, value) VALUES (?, ?)', [
      _prefsFlag,
      '1',
    ]);
  }
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
