/// Display helpers shared by conversation list and chat chrome.
library;

import '../copy.dart';

const _avatarColors = <int>[
  0xFFE17076,
  0xFFFAA775,
  0xFFA695E7,
  0xFF7BC862,
  0xFF6EC9CB,
  0xFF65AADD,
  0xFFEE7AAE,
  0xFFE5C85F,
  0xFF54A0FF,
  0xFF0D9488,
];

String initialOf(String name) {
  final trimmed = name.trim();
  if (trimmed.isEmpty) {
    return '?';
  }
  return trimmed.substring(0, 1).toUpperCase();
}

int avatarColor(String name) {
  var hash = 0;
  for (final unit in name.codeUnits) {
    hash = (hash * 33 + unit) & 0xFFFFFFFF;
  }
  return _avatarColors[hash % _avatarColors.length];
}

String formatListTime(int ts) {
  if (ts <= 0) {
    return '';
  }
  final d = DateTime.fromMillisecondsSinceEpoch(ts);
  final now = DateTime.now();
  final today = DateTime(now.year, now.month, now.day);
  final point = DateTime(d.year, d.month, d.day);
  if (point == today) {
    return _hm(d);
  }
  if (point == today.subtract(const Duration(days: 1))) {
    return Copy.yesterday;
  }
  if (now.difference(point).inDays < 7 && now.difference(point).inDays >= 0) {
    const days = ['周一', '周二', '周三', '周四', '周五', '周六', '周日'];
    return days[d.weekday - 1];
  }
  return '${d.month}/${d.day}';
}

String formatClock(int ts) {
  if (ts <= 0) {
    return '';
  }
  return _hm(DateTime.fromMillisecondsSinceEpoch(ts));
}

String truncate(String text, {int max = 36}) {
  final t = text.trim().replaceAll(RegExp(r'\s+'), ' ');
  if (t.length <= max) {
    return t;
  }
  return '${t.substring(0, max)}…';
}

String _two(int n) => n.toString().padLeft(2, '0');

String _hm(DateTime d) => '${_two(d.hour)}:${_two(d.minute)}';
