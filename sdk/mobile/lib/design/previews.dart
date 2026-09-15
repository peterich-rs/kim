library;

import 'package:flutter/material.dart';
import 'package:flutter/widget_previews.dart';

import 'package:kim_mobile/copy.dart';
import 'package:kim_mobile/models/models.dart';
import 'package:kim_mobile/design/kim_theme.dart';
import 'package:kim_mobile/design/conversation_tile.dart';
import 'package:kim_mobile/design/kim_bubble.dart';
import 'package:kim_mobile/design/kim_composer.dart';
import 'package:kim_mobile/design/kim_text_field.dart';

ThemeData _theme() => KimTheme.light();

@Preview(name: 'Conversation tile', group: 'Chat')
Widget previewConversationTile() {
  return MaterialApp(
    theme: _theme(),
    home: Scaffold(
      body: ConversationTile(
        thread: const KimThread(
          id: 'bob',
          kind: ThreadKind.user,
          title: 'Bob',
          lastBody: 'hello',
          lastAt: 0,
          unread: 2,
        ),
        onOpen: () {},
        onDelete: () {},
      ),
    ),
  );
}

@Preview(name: 'Conversation rail', group: 'Chat')
Widget previewConversationRail() {
  return MaterialApp(
    theme: _theme(),
    home: Scaffold(
      body: SizedBox(
        width: 80,
        child: ListView(
          children: [
            ConversationRailAvatar(
              thread: const KimThread(
                id: 'alice',
                kind: ThreadKind.user,
                title: 'Alice',
              ),
              onOpen: () {},
            ),
            ConversationRailAvatar(
              thread: const KimThread(
                id: 'bob',
                kind: ThreadKind.user,
                title: 'Bob',
                unread: 3,
              ),
              selected: true,
              onOpen: () {},
            ),
          ],
        ),
      ),
    ),
  );
}

@Preview(name: 'Message bubble', group: 'Chat')
Widget previewKimBubble() {
  return MaterialApp(
    theme: _theme(),
    home: Scaffold(
      body: Padding(
        padding: const EdgeInsets.all(16),
        child: KimMessageRow(
          message: const KimChatMsg(
            key: '1',
            dest: 'bob',
            sender: 'alice',
            body: 'hello',
            at: 0,
          ),
          isSentByMe: true,
        ),
      ),
    ),
  );
}

@Preview(name: 'Composer', group: 'Chat')
Widget previewKimComposer() {
  return MaterialApp(
    theme: _theme(),
    localizationsDelegates: AppLocalizations.localizationsDelegates,
    supportedLocales: AppLocalizations.supportedLocales,
    locale: const Locale('zh'),
    home: Scaffold(
      body: Align(
        alignment: Alignment.bottomCenter,
        child: KimComposer(
          onSend: (_) {},
          onPickAlbum: () {},
          onTakePhoto: () {},
        ),
      ),
    ),
  );
}

@Preview(name: 'Text field', group: 'Forms')
Widget previewKimTextField() {
  return MaterialApp(
    theme: _theme(),
    localizationsDelegates: AppLocalizations.localizationsDelegates,
    supportedLocales: AppLocalizations.supportedLocales,
    locale: const Locale('zh'),
    home: Scaffold(
      body: Padding(
        padding: const EdgeInsets.all(24),
        child: KimTextField(
          controller: TextEditingController(text: 'alice'),
          label: '账号',
        ),
      ),
    ),
  );
}
