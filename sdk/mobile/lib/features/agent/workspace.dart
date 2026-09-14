library;

import 'dart:io';

import 'package:flutter/foundation.dart';

import 'package:kim_mobile/core/paths.dart';
import 'package:kim_mobile/features/agent/agent_profiles.dart';
import 'package:kim_mobile/features/agent/workspace_access.dart';

class WorkspaceResolveResult {
  const WorkspaceResolveResult({
    required this.path,
    this.bookmarkBase64 = '',
    this.invalidRepo = false,
  });

  final String path;
  final String bookmarkBase64;
  final bool invalidRepo;
}

/// Resolve the absolute cwd passed to host `projectRoot`.
Future<WorkspaceResolveResult> resolveAgentProjectRoot({
  required AgentProfile profile,
  required KimPaths paths,
  WorkspaceAccess? access,
}) async {
  final ws = profile.workspace;
  final acc = access ?? workspaceAccess;

  if (ws.isRepo) {
    final stored = ws.bookmarkRef.isNotEmpty
        ? ws.bookmarkRef
        : (await acc.loadBookmark(profile.id) ?? '');
    if (!kIsWeb && Platform.isMacOS && stored.isNotEmpty) {
      final accessed = await acc.startAccessing(stored);
      if (accessed != null && await Directory(accessed).exists()) {
        return WorkspaceResolveResult(path: accessed, bookmarkBase64: stored);
      }
      // Stale / denied — do not silently fall back on Release sandbox.
      if (_macosSandboxLikely()) {
        final sandbox = await paths.ensureSandbox(profile.id);
        return WorkspaceResolveResult(path: sandbox.path, invalidRepo: true);
      }
    }
    if (ws.path.isNotEmpty) {
      final dir = Directory(ws.path);
      if (await dir.exists()) {
        if (!kIsWeb &&
            Platform.isMacOS &&
            stored.isEmpty &&
            _macosSandboxLikely()) {
          final sandbox = await paths.ensureSandbox(profile.id);
          return WorkspaceResolveResult(path: sandbox.path, invalidRepo: true);
        }
        return WorkspaceResolveResult(
          path: dir.absolute.path,
          bookmarkBase64: stored,
        );
      }
    }
    final sandbox = await paths.ensureSandbox(profile.id);
    return WorkspaceResolveResult(path: sandbox.path, invalidRepo: true);
  }

  final sandbox = await paths.ensureSandbox(profile.id);
  return WorkspaceResolveResult(path: sandbox.path);
}

bool _macosSandboxLikely() {
  // Debug entitlements turn sandbox off; Release turns it on.
  // Prefer requiring a bookmark whenever one was expected for repo kind.
  return !kDebugMode;
}
