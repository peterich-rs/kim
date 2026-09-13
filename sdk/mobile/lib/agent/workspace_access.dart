/// Directory picker + macOS security-scoped bookmarks for agent repo workspaces.
library;

import 'dart:io';

import 'package:file_picker/file_picker.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:flutter_secure_storage/flutter_secure_storage.dart';

import '../core/settings.dart';

const _kChannel = 'kim.workspace';
const _kBookmarkPrefix = 'agent.workspace_bookmark.';

class PickedWorkspace {
  const PickedWorkspace({required this.path, this.bookmarkBase64 = ''});

  final String path;
  final String bookmarkBase64;
}

class WorkspaceAccess {
  WorkspaceAccess({
    MethodChannel? channel,
    FlutterSecureStorage? secure,
    Future<String?> Function()? pickDirectoryFallback,
  }) : _channel = channel ?? const MethodChannel(_kChannel),
       _secure = secure ?? SettingsStore.productionSecureStorage(),
       _pickDirectoryFallback = pickDirectoryFallback;

  final MethodChannel _channel;
  final FlutterSecureStorage _secure;
  final Future<String?> Function()? _pickDirectoryFallback;

  /// Active security-scoped bookmarks for this process.
  final Set<String> _held = {};

  String bookmarkKey(String profileId) => '$_kBookmarkPrefix$profileId';

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

  Future<void> saveBookmark(String profileId, String bookmarkBase64) async {
    final key = bookmarkKey(profileId);
    if (bookmarkBase64.isEmpty) {
      await _secure.delete(key: key);
      return;
    }
    await _secure.write(key: key, value: bookmarkBase64);
  }

  Future<String?> loadBookmark(String profileId) async {
    return _secure.read(key: bookmarkKey(profileId));
  }

  Future<void> clearBookmark(String profileId) async {
    final existing = await loadBookmark(profileId);
    if (existing != null && existing.isNotEmpty) {
      await stopAccessing(existing);
    }
    await _secure.delete(key: bookmarkKey(profileId));
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
        final path = await _channel.invokeMethod<String>('realHomeAgentsSkills');
        if (path != null && path.isNotEmpty) {
          return path;
        }
      } on MissingPluginException {
        // fall through
      } on PlatformException {
        // fall through
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
