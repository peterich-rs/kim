/// Local thread + message cache. Same role as the H5 localStorage inbox:
/// UI persistence only; talk still goes through kim-client.
library;

import 'dart:convert';

import 'package:shared_preferences/shared_preferences.dart';

import '../models/models.dart';

class ConversationStore {
  ConversationStore(this._prefs);

  static const _threadPrefix = 'kim.threads.';
  static const _msgPrefix = 'kim.msgs.';
  static const maxMessages = 400;

  final SharedPreferences _prefs;

  static Future<ConversationStore> load() async {
    return ConversationStore(await SharedPreferences.getInstance());
  }

  List<KimThread> loadThreads(String account) {
    if (account.isEmpty) {
      return const [];
    }
    final raw = _prefs.getString('$_threadPrefix$account');
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

  Future<void> saveThreads(String account, List<KimThread> threads) async {
    if (account.isEmpty) {
      return;
    }
    final slim = threads.map((t) => t.toJson()).toList();
    await _prefs.setString('$_threadPrefix$account', jsonEncode(slim));
  }

  List<KimChatMsg> loadMessages(String account, String dest) {
    if (account.isEmpty || dest.isEmpty) {
      return const [];
    }
    final raw = _prefs.getString(_msgKey(account, dest));
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
    await _prefs.setString(
      _msgKey(account, dest),
      jsonEncode(clipped.map((m) => m.toJson()).toList()),
    );
  }

  Future<void> deleteThread(String account, String dest) async {
    final threads = loadThreads(account).where((t) => t.id != dest).toList();
    await saveThreads(account, threads);
    await _prefs.remove(_msgKey(account, dest));
  }

  String _msgKey(String account, String dest) => '$_msgPrefix$account.$dest';
}
