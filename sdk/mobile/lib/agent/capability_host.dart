library;

import 'dart:convert';

import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../agent/mention.dart';
import '../models/models.dart';
import '../state/agent_profiles.dart';
import '../state/auth.dart';
import '../state/contacts.dart';
import '../state/outbox.dart';
import '../state/providers.dart';

class KimCapabilityHost {
  KimCapabilityHost(this.ref);

  final Ref ref;

  Future<String> execute({
    required String name,
    required String argumentsJson,
    required String sessionDest,
    required String profileId,
    String callId = '',
  }) async {
    try {
      final args = _decode(argumentsJson);
      switch (name) {
        case 'search_contacts':
          return await _searchContacts('${args['query'] ?? ''}');
        case 'search_messages':
          return await _searchMessages(
            dest: '${args['dest'] ?? sessionDest}',
            query: '${args['query'] ?? ''}',
            limit: _limit(args['limit'], 20),
          );
        case 'get_conversation_context':
          return await _conversationContext(
            dest: '${args['dest'] ?? sessionDest}',
            limit: _limit(args['limit'], 20),
          );
        case 'list_profiles':
          return await _listProfiles();
        case 'send_message':
          return await _sendMessage(
            dest: '${args['dest'] ?? ''}',
            text: '${args['text'] ?? ''}',
            callId: callId,
          );
        case 'read_clipboard':
          return await _readClipboard();
        default:
          return _err('unknown tool $name');
      }
    } catch (e) {
      return _err('$e');
    }
  }

  Map<String, Object?> _decode(String raw) {
    try {
      final v = jsonDecode(raw);
      if (v is Map) {
        return Map<String, Object?>.from(v);
      }
    } catch (_) {}
    return {};
  }

  int _limit(Object? raw, int fallback) {
    final n = raw is int ? raw : int.tryParse('$raw');
    if (n == null) {
      return fallback;
    }
    if (n < 1) {
      return 1;
    }
    if (n > 50) {
      return 50;
    }
    return n;
  }

  String _err(String message) => jsonEncode({'ok': false, 'error': message});

  String _ok(Map<String, Object?> body) => jsonEncode({'ok': true, ...body});

  Future<String> _searchContacts(String query) async {
    final q = query.trim().toLowerCase();
    final contacts = ref.read(contactsProvider);
    final hits = [
      for (final p in contacts.friends)
        if (!p.isBot &&
            (q.isEmpty ||
                p.account.toLowerCase().contains(q) ||
                p.nickname.toLowerCase().contains(q)))
          {'account': p.account, 'nickname': p.nickname},
    ];
    if (hits.isEmpty && q.isNotEmpty) {
      try {
        final remote = await ref.read(clientPortProvider).searchUsers(query);
        for (final p in remote.take(20)) {
          if (p.isBot) {
            continue;
          }
          hits.add({'account': p.account, 'nickname': p.nickname});
        }
      } catch (e) {
        return _err('$e');
      }
    }
    return jsonEncode({'people': hits.take(20).toList()});
  }

  Future<String> _searchMessages({
    required String dest,
    required String query,
    required int limit,
  }) async {
    final account = ref.read(authProvider).account;
    if (account.isEmpty || query.trim().isEmpty) {
      return jsonEncode({'hits': <Object?>[]});
    }
    final needle = query.trim().toLowerCase();
    final rows = ref.read(conversationStoreProvider).loadMessages(account, dest);
    final hits = <Map<String, Object?>>[];
    for (final m in rows.reversed) {
      if (m.kind != KimMsgKind.text || m.sys) {
        continue;
      }
      if (!m.body.toLowerCase().contains(needle)) {
        continue;
      }
      final body = m.body.length > 500 ? m.body.substring(0, 500) : m.body;
      hits.add({'sender': m.sender, 'body': body, 'at': m.at});
      if (hits.length >= limit) {
        break;
      }
    }
    return jsonEncode({'hits': hits});
  }

  Future<String> _conversationContext({
    required String dest,
    required int limit,
  }) async {
    final account = ref.read(authProvider).account;
    if (account.isEmpty) {
      return jsonEncode({'messages': <Object?>[]});
    }
    final rows = ref.read(conversationStoreProvider).loadMessages(account, dest);
    final text = [
      for (final m in rows)
        if (m.kind == KimMsgKind.text && !m.sys) m,
    ];
    final slice = text.length > limit ? text.sublist(text.length - limit) : text;
    const cap = 8 * 1024;
    final out = <Map<String, Object?>>[];
    var used = 0;
    for (final m in slice.reversed) {
      final line = '${m.sender}: ${m.body}';
      used += line.length;
      if (used > cap) {
        break;
      }
      out.add({'sender': m.sender, 'body': m.body, 'at': m.at});
    }
    return jsonEncode({'messages': out.reversed.toList()});
  }

  Future<String> _listProfiles() async {
    await ref.read(agentProfilesProvider.notifier).ensureLoaded();
    final rows = [
      for (final p in ref.read(agentProfilesProvider))
        if (p.enabled) {'id': p.id, 'display_name': p.displayName},
    ];
    return jsonEncode({'profiles': rows});
  }

  String? _refuseDest(String dest) {
    if (dest.isEmpty) {
      return 'missing dest';
    }
    if (isAgentDest(dest)) {
      return 'refusing to message a local agent';
    }
    if (dest.contains('/')) {
      return 'group dest not supported';
    }
    return null;
  }

  Future<String> _sendMessage({
    required String dest,
    required String text,
    required String callId,
  }) async {
    final refuse = _refuseDest(dest);
    if (refuse != null) {
      return _err(refuse);
    }
    try {
      await ref
          .read(outboxProvider.notifier)
          .sendText(dest, text, clientId: callId);
      return _ok({'client_id': callId});
    } catch (e) {
      return _err('$e');
    }
  }

  Future<String> _readClipboard() async {
    try {
      final data = await Clipboard.getData(Clipboard.kTextPlain);
      return jsonEncode({'text': data?.text ?? ''});
    } catch (e) {
      return _err('$e');
    }
  }

}
