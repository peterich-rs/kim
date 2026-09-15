library;

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import 'package:kim_mobile/core/layout.dart';
import 'package:kim_mobile/models/models.dart';
import 'package:kim_mobile/features/chats/inbox.dart';

void openKimChat(
  BuildContext context,
  WidgetRef ref, {
  required String id,
  ThreadKind kind = ThreadKind.user,
  String? title,
}) {
  ref
      .read(threadsProvider.notifier)
      .ensureThread(id: id, kind: kind, title: title ?? id);
  final path = '/chat/$id';
  if (kimIsWide(context)) {
    context.go(path);
  } else {
    context.push(path);
  }
}
