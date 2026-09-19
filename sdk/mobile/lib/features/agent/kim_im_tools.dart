library;

import 'dart:convert';

import 'package:flutter/services.dart';
import 'package:uuid/uuid.dart';

import 'package:kim_mobile/bridge/kim_bridge.dart';
import 'package:kim_mobile/features/agent/mention.dart';
import 'package:kim_mobile/models/models.dart';

/// Executes deferred KIM tools the Goose host yields to Dart.
class KimImTools {
  KimImTools(this.client);

  final KimClientPort client;

  Future<String> execute({
    required String name,
    required String argumentsJson,
    required String currentDest,
  }) async {
    Map<String, Object?> args = {};
    try {
      final raw = jsonDecode(argumentsJson);
      if (raw is Map) {
        args = Map<String, Object?>.from(raw);
      }
    } catch (_) {}
    try {
      switch (name) {
        case 'search_contacts':
          return await _searchContacts('${args['query'] ?? ''}');
        case 'search_messages':
          return await _searchMessages(
            query: '${args['query'] ?? ''}',
            dest: '${args['dest'] ?? ''}',
            fallbackDest: currentDest,
          );
        case 'get_conversation_context':
          return await _conversationContext(
            dest: '${args['dest'] ?? ''}',
            fallbackDest: currentDest,
            limit: _asInt(args['limit'], 20),
          );
        case 'list_profiles':
          return await _listProfiles();
        case 'send_message':
          return await _sendMessage(
            dest: '${args['dest'] ?? ''}',
            text: '${args['text'] ?? ''}',
          );
        case 'read_clipboard':
          return await _readClipboard();
        default:
          return jsonEncode({'ok': false, 'error': 'unknown tool $name'});
      }
    } catch (e) {
      return jsonEncode({'ok': false, 'error': '$e'});
    }
  }

  Future<String> _searchContacts(String query) async {
    final q = query.trim().toLowerCase();
    final friends = await client.friendList();
    var hits = friends;
    if (q.isNotEmpty) {
      hits = [
        for (final p in friends)
          if (p.account.toLowerCase().contains(q) ||
              p.nickname.toLowerCase().contains(q))
            p,
      ];
    }
    if (hits.isEmpty && q.isNotEmpty) {
      hits = await client.searchUsers(query);
    }
    return jsonEncode({
      'ok': true,
      'people': [
        for (final p in hits.take(20))
          {'account': p.account, 'nickname': p.nickname, 'kind': p.kind},
      ],
    });
  }

  Future<String> _searchMessages({
    required String query,
    required String dest,
    required String fallbackDest,
  }) async {
    final q = query.trim();
    if (q.isEmpty) {
      return jsonEncode({'ok': false, 'error': 'query required'});
    }
    final thread = dest.trim().isEmpty ? fallbackDest : dest.trim();
    final rows = await client.searchMessages(
      q,
      dest: thread.isEmpty ? null : thread,
    );
    return jsonEncode({
      'ok': true,
      'messages': [
        for (final m in rows.take(20))
          {
            'dest': m.dest,
            'sender': m.sender,
            'body': m.body,
            'at': m.at.toInt(),
          },
      ],
    });
  }

  Future<String> _conversationContext({
    required String dest,
    required String fallbackDest,
    required int limit,
  }) async {
    final thread = dest.trim().isEmpty ? fallbackDest : dest.trim();
    if (thread.isEmpty) {
      return jsonEncode({'ok': false, 'error': 'dest required'});
    }
    final rows = await client.searchMessages('', dest: thread);
    final take = limit.clamp(1, 50);
    return jsonEncode({
      'ok': true,
      'dest': thread,
      'messages': [
        for (final m in rows.take(take))
          {'sender': m.sender, 'body': m.body, 'at': m.at.toInt()},
      ],
    });
  }

  Future<String> _listProfiles() async {
    final rows = await client.listAgentProfiles();
    return jsonEncode({
      'ok': true,
      'profiles': [
        for (final r in rows)
          {
            'id': r.profileId,
            'display_name': r.nickname,
            'server_account': r.serverAccount,
          },
      ],
    });
  }

  Future<String> _sendMessage({
    required String dest,
    required String text,
  }) async {
    final to = dest.trim();
    final body = text.trim();
    if (to.isEmpty || body.isEmpty) {
      return jsonEncode({'ok': false, 'error': 'dest and text required'});
    }
    if (isAgentDest(to) || isServerBotAccount(to)) {
      return jsonEncode({
        'ok': false,
        'error': 'send_message cannot target an agent',
      });
    }
    await client.enqueueMessage(
      dest: to,
      kind: ThreadKind.user,
      content: KimOutgoingContent.text(body),
      clientId: const Uuid().v4(),
    );
    return jsonEncode({'ok': true, 'dest': to});
  }

  Future<String> _readClipboard() async {
    final data = await Clipboard.getData(Clipboard.kTextPlain);
    return jsonEncode({'ok': true, 'text': data?.text ?? ''});
  }
}

int _asInt(Object? raw, int fallback) {
  if (raw is int) {
    return raw;
  }
  return int.tryParse('$raw') ?? fallback;
}
