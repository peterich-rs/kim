/// Resolve the absolute cwd passed to host `projectRoot`.
/// Picker + security-scoped bookmark stay platform; after resolution the
/// granted path is registered with Rust (`workspace_grants`) and reused.
library;

import 'dart:io';

import 'package:flutter/foundation.dart';

import 'package:kim_mobile/bridge/kim_ports.dart';
import 'package:kim_mobile/features/agent/providers/agent_profiles.dart';
import 'package:kim_mobile/features/agent/data/workspace_access.dart';

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

/// Resolve the absolute cwd for a profile.
///
/// Order: registered Rust grant → bookmark access → stored path → Rust
/// sandbox fallback. Sandboxes and grants are Rust-owned state.
Future<WorkspaceResolveResult> resolveAgentProjectRoot({
  required AgentProfile profile,
  KimClientPort? client,
  WorkspaceAccess? access,
}) async {
  final acc = access ?? workspaceAccess;

  if (profile.workspace.isRepo) {
    // 1. Rust-registered grant from a previous resolve.
    String stored = profile.workspace.bookmarkRef;
    if (client != null) {
      try {
        final granted = await client.workspaceGrant(profile.id);
        if (granted != null &&
            granted.isNotEmpty &&
            await Directory(granted).exists()) {
          return WorkspaceResolveResult(path: granted);
        }
      } catch (_) {
        // Store not attached / phone — fall through.
      }
      try {
        final grantBookmark = await client.workspaceGrantBookmark(profile.id);
        if (grantBookmark.isNotEmpty) {
          stored = grantBookmark;
        }
      } catch (_) {}
    }

    // 2. Security-scoped bookmark (macOS). Bytes live in Rust (overlay /
    // grant); start/stop stays platform.
    if (!kIsWeb && Platform.isMacOS && stored.isNotEmpty) {
      final accessed = await acc.startAccessing(stored);
      if (accessed != null && await Directory(accessed).exists()) {
        await _register(client, profile.id, accessed, bookmark: stored);
        return WorkspaceResolveResult(path: accessed, bookmarkBase64: stored);
      }
      if (_macosSandboxLikely()) {
        return _sandbox(client, profile.id, invalidRepo: true);
      }
    }

    // 3. Plain path (non-sandboxed platforms / debug).
    if (profile.workspace.path.isNotEmpty) {
      final dir = Directory(profile.workspace.path);
      if (await dir.exists()) {
        if (!kIsWeb &&
            Platform.isMacOS &&
            stored.isEmpty &&
            _macosSandboxLikely()) {
          return _sandbox(client, profile.id, invalidRepo: true);
        }
        await _register(
          client,
          profile.id,
          dir.absolute.path,
          bookmark: stored,
        );
        return WorkspaceResolveResult(
          path: dir.absolute.path,
          bookmarkBase64: stored,
        );
      }
    }
    return _sandbox(client, profile.id, invalidRepo: true);
  }

  return _sandbox(client, profile.id);
}

Future<WorkspaceResolveResult> _sandbox(
  KimClientPort? client,
  String profileId, {
  bool invalidRepo = false,
}) async {
  if (client != null) {
    try {
      final path = await client.ensureAgentSandbox(profileId);
      return WorkspaceResolveResult(path: path, invalidRepo: invalidRepo);
    } catch (_) {
      // Fall through to temp dir below.
    }
  }
  return WorkspaceResolveResult(
    path: '${Directory.systemTemp.path}/kim-agent/$profileId',
    invalidRepo: invalidRepo,
  );
}

Future<void> _register(
  KimClientPort? client,
  String profileId,
  String path, {
  String? bookmark,
}) async {
  if (client == null) {
    return;
  }
  try {
    await client.workspaceGrantRegister(
      profileId: profileId,
      path: path,
      bookmark: bookmark,
    );
  } catch (_) {
    // Non-fatal: the bookmark stays the source for the next resolve.
  }
}

bool _macosSandboxLikely() {
  // Debug entitlements turn sandbox off; Release turns it on.
  // Prefer requiring a bookmark whenever one was expected for repo kind.
  return !kDebugMode;
}
