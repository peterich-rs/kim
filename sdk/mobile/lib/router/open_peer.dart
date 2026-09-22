library;

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import 'package:kim_mobile/core/haptics.dart';
import 'package:kim_mobile/core/layout.dart';
import 'package:kim_mobile/router/app_routes.dart';

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
  final path = AppRoutes.peer(id, title: title);
  if (kimIsWide(context)) {
    context.push(path);
  } else {
    context.push(path);
  }
}
