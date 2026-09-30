/// v2 message row: Discord grouping, Telegram own-bubble, send-state icons.
library;

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:gap/gap.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import 'package:kim_mobile/copy.dart';
import 'package:kim_mobile/core/format.dart';
import 'package:kim_mobile/core/layout.dart';
import 'package:kim_mobile/models/models.dart';
import 'package:kim_mobile/design/kim_theme.dart';
import 'package:kim_mobile/design/agent_action_bubble.dart';
import 'package:kim_mobile/design/kim_avatar.dart';
import 'package:kim_mobile/design/kim_hairline.dart';
import 'package:kim_mobile/design/kim_image_viewer.dart';
import 'package:kim_mobile/design/kim_network_image.dart';

part 'message_row/message_row.dart';
part 'message_row/blocks.dart';
part 'message_row/bubble.dart';
part 'message_row/media_body.dart';
part 'message_row/send_state.dart';

const _groupWindow = Duration(minutes: 5);

/// Peer/bot sending spinner waits this long so fast sends stay clean.
const kPeerSendBusyDelay = Duration(milliseconds: 700);

bool kimIsGroupStart(KimChatMsg msg, KimChatMsg? previous) {
  return !_sameGroup(previous, msg);
}

bool kimIsGroupEnd(KimChatMsg msg, KimChatMsg? next) {
  return !_sameGroup(msg, next);
}

/// Both timestamps are milliseconds from the store. Out-of-range values
/// do not group.
bool _sameGroup(KimChatMsg? earlier, KimChatMsg? later) {
  if (earlier == null || later == null || earlier.sys || later.sys) {
    return false;
  }
  if (earlier.sender != later.sender) {
    return false;
  }
  final a = dateTimeFromEpoch(earlier.at);
  final b = dateTimeFromEpoch(later.at);
  if (a == null || b == null) {
    return false;
  }
  final localA = a.toLocal();
  final localB = b.toLocal();
  if (!sameCalendarDay(localA, localB)) {
    return false;
  }
  return b.difference(a).abs() <= _groupWindow;
}

bool kimSameBatch(KimChatMsg msg, KimChatMsg? previous) {
  final id = msg.batchId;
  return id != null && id.isNotEmpty && previous?.batchId == id;
}
