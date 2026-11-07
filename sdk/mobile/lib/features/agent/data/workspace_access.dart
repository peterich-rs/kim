/// Directory picker + macOS security-scoped bookmarks for agent repo workspaces.
library;

import 'dart:async';
import 'dart:io';

import 'package:file_picker/file_picker.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

const _kChannel = 'kim.workspace';

class PickedWorkspace {
  const PickedWorkspace({required this.path, this.bookmarkBase64 = ''});

  final String path;
  final String bookmarkBase64;
}

/// Picker + start/stop access stay platform. Bookmark bytes live in Rust
/// (`agent_device_overlay.workspace_bookmark` via the profile save path and
/// `workspace_grants.bookmark`); this class no longer stores anything.
class WorkspaceAccess {
  WorkspaceAccess({MethodChannel? channel, this._pickDirectoryFallback})
    : _channel = channel ?? const MethodChannel(_kChannel);

  final MethodChannel _channel;
  final Future<String?> Function()? _pickDirectoryFallback;

  /// Active security-scoped bookmarks for this process.
  final Set<String> _held = {};

  Future<PickedWorkspace?> pickDirectory() async {
    if (!kIsWeb && Platform.isMacOS) {
      try {
        final raw = await _channel.invokeMethod<dynamic>('pickDirectory');
        if (raw is Map) {
          final path = '${raw['path'] ?? ''}';
          final bookmark = '${raw['bookmarkBase64'] ?? ''}';
          if (path.isNotEmpty) {
            return PickedWorkspace(path: path, bookmarkBase64: bookmark);
          }
        }
        return null;
      } on MissingPluginException {
        // Fall through to file_picker in tests / unsupported embeds.
      }
    }
    final fallback = _pickDirectoryFallback;
    if (fallback != null) {
      final path = await fallback();
      if (path == null || path.isEmpty) {
        return null;
      }
      return PickedWorkspace(path: path);
    }
    final path = await FilePicker.getDirectoryPath(
      dialogTitle: 'Select repository',
    );
    if (path == null || path.isEmpty) {
      return null;
    }
    return PickedWorkspace(path: path);
  }

  /// Returns resolved absolute path when access succeeds.
  Future<String?> startAccessing(String bookmarkBase64) async {
    if (bookmarkBase64.isEmpty || kIsWeb || !Platform.isMacOS) {
      return null;
    }
    try {
      final raw = await _channel.invokeMethod<dynamic>('startAccessing', {
        'bookmarkBase64': bookmarkBase64,
      });
      if (raw is Map && raw['ok'] == true) {
        _held.add(bookmarkBase64);
        final path = '${raw['path'] ?? ''}';
        return path.isEmpty ? null : path;
      }
    } on PlatformException {
      return null;
    } on MissingPluginException {
      return null;
    }
    return null;
  }

  Future<void> stopAccessing(String bookmarkBase64) async {
    if (bookmarkBase64.isEmpty || !_held.remove(bookmarkBase64)) {
      return;
    }
    if (kIsWeb || !Platform.isMacOS) {
      return;
    }
    try {
      await _channel.invokeMethod<void>('stopAccessing', {
        'bookmarkBase64': bookmarkBase64,
      });
    } on MissingPluginException {
      // ignore
    } on PlatformException {
      // ignore
    }
  }

  Future<void> stopAll() async {
    final held = _held.toList();
    for (final b in held) {
      await stopAccessing(b);
    }
  }

  /// Real `~/.agents/skills` (S-KD 23). Null when unavailable.
  Future<String?> realUserAgentsSkills() async {
    if (!kIsWeb && Platform.isMacOS) {
      try {
        final path = await _channel
            .invokeMethod<String>('realHomeAgentsSkills')
            .timeout(const Duration(milliseconds: 400));
        if (path != null && path.isNotEmpty) {
          return path;
        }
      } on MissingPluginException {
        // fall through
      } on PlatformException {
        // fall through
      } on TimeoutException {
        // Tests / missing plugin must not block save or session_open.
      }
    }
    final home = Platform.environment['HOME'];
    if (home == null || home.isEmpty) {
      return null;
    }
    return '$home/.agents/skills';
  }
}

final workspaceAccess = WorkspaceAccess();

final workspaceAccessProvider = Provider<WorkspaceAccess>(
  (ref) => workspaceAccess,
);

/// Live plugin path wins; empty/whitespace live falls back to overlay.
String resolveUserAgentsSkills({String? live, String overlay = ''}) {
  final trimmedLive = live?.trim() ?? '';
  if (trimmedLive.isNotEmpty) {
    return trimmedLive;
  }
  return overlay.trim();
}

class SkillHostPaths {
  const SkillHostPaths({this.userAgentsSkills = ''});

  final String userAgentsSkills;
}

/// Session-only paths injected into host JSON. Never written into AgentSpec.
Future<SkillHostPaths> skillHostPaths({
  required WorkspaceAccess access,
  String overlay = '',
}) async {
  String? live;
  try {
    live = await access.realUserAgentsSkills();
  } catch (_) {
    live = null;
  }
  return SkillHostPaths(
    userAgentsSkills: resolveUserAgentsSkills(live: live, overlay: overlay),
  );
}
