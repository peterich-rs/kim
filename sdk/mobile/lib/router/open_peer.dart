library;

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import '../core/haptics.dart';
import '../core/layout.dart';

void openKimPeerProfile(
  BuildContext context,
  WidgetRef ref, {
  required String id,
  String? title,
}) {
  if (id.isEmpty) {
    return;
  }
  KimHaptics.selection();
  final q = (title == null || title.isEmpty)
      ? ''
      : '?title=${Uri.encodeQueryComponent(title)}';
  final path = '/peer/$id$q';
  if (kimIsWide(context)) {
    context.push(path);
  } else {
    context.push(path);
  }
}
