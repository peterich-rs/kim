/// App / portable skill catalog helpers for skills page + plaza (S-KD 3 / 7 / 9).
library;

import 'dart:convert';
import 'dart:io';

import '../agent_bridge.dart';
import '../core/paths.dart';
import '../state/agent_profiles.dart';
import 'workspace.dart';
import 'workspace_access.dart';

class CatalogSkill {
  const CatalogSkill({
    required this.id,
    required this.name,
    required this.description,
    this.version = '',
    this.origin = '',
    this.className = SkillRef.classApp,
    this.dir = '',
  });

  final String id;
  final String name;
  final String description;
  final String version;
  final String origin;
  final String className;
  final String dir;

  bool get isApp => className == SkillRef.classApp || id.startsWith('kim-');
}

/// Hard-coded S-KD 7 requirements until frontmatter `requires_tools` is parsed.
List<String> requiresToolsForAppSkill(String id) {
  switch (id) {
    case 'kim-im':
      return const [
        'send_message',
        'search_contacts',
        'search_messages',
        'get_conversation_context',
      ];
    case 'kim-memory':
      return const ['fs', 'fs_write'];
    default:
      return const [];
  }
}

List<String> missingToolsForAppSkill(AgentProfile profile, String id) {
  final need = requiresToolsForAppSkill(id);
  if (need.isEmpty) {
    return const [];
  }
  final t = profile.tools;
  final have = <String>{
    if (t.sendMessage) 'send_message',
    if (t.searchContacts) 'search_contacts',
    if (t.searchMessages) 'search_messages',
    if (t.getConversationContext) 'get_conversation_context',
    if (t.listProfiles) 'list_profiles',
    if (t.readClipboard) 'read_clipboard',
    if (t.fs || t.fsWrite) 'fs',
    if (t.fsWrite) 'fs_write',
    if (t.bash) 'bash',
  };
  return [for (final n in need) if (!have.contains(n)) n];
}

AgentToolSet enableRequiredTools(AgentToolSet tools, List<String> missing) {
  var next = tools;
  for (final name in missing) {
    switch (name) {
      case 'send_message':
        next = next.copyWith(sendMessage: true);
      case 'search_contacts':
        next = next.copyWith(searchContacts: true);
      case 'search_messages':
        next = next.copyWith(searchMessages: true);
      case 'get_conversation_context':
        next = next.copyWith(getConversationContext: true);
      case 'list_profiles':
        next = next.copyWith(listProfiles: true);
      case 'read_clipboard':
        next = next.copyWith(readClipboard: true);
      case 'fs':
        next = next.copyWith(fs: true);
      case 'fs_write':
        next = next.copyWith(fs: true, fsWrite: true);
      case 'bash':
        // S-KD 7: never AlwaysAllow bash; only enable the tool with ask_before.
        next = next.copyWith(bash: true);
      default:
        break;
    }
  }
  return next;
}

List<CatalogSkill> parseSkillsJson(String raw) {
  if (raw.trim().isEmpty) {
    return const [];
  }
  final decoded = jsonDecode(raw);
  if (decoded is! Map) {
    return const [];
  }
  final list = decoded['skills'];
  if (list is! List) {
    return const [];
  }
  final out = <CatalogSkill>[];
  for (final item in list) {
    if (item is! Map) {
      continue;
    }
    final map = Map<String, Object?>.from(item);
    final id = '${map['id'] ?? ''}'.trim();
    if (id.isEmpty) {
      continue;
    }
    out.add(
      CatalogSkill(
        id: id,
        name: '${map['name'] ?? id}',
        description: '${map['description'] ?? ''}',
        version: '${map['version'] ?? ''}',
        origin: '${map['origin'] ?? ''}',
        className: '${map['class'] ?? SkillRef.classApp}',
        dir: '${map['dir'] ?? ''}',
      ),
    );
  }
  return out;
}

Future<List<CatalogSkill>> loadAppSkillCatalog({
  required AgentBridge bridge,
  KimPaths? paths,
}) async {
  await bridge.ensure();
  final cache = (paths ?? KimPaths.instance).appSkillCache;
  final raw = await bridge.skillAppCatalogJson(cacheRoot: cache.path);
  return parseSkillsJson(raw);
}

Future<List<CatalogSkill>> loadPortableSkillCatalog({
  required AgentBridge bridge,
  required AgentProfile profile,
  KimPaths? paths,
  WorkspaceAccess? access,
}) async {
  final scan =
      profile.workspace.isRepo || profile.tools.fs || profile.tools.fsWrite;
  if (!scan) {
    return const [];
  }
  await bridge.ensure();
  final p = paths ?? KimPaths.instance;
  final acc = access ?? workspaceAccess;
  final userRoot = await acc.realUserAgentsSkills() ?? '';
  final resolved = await resolveAgentProjectRoot(
    profile: profile,
    paths: p,
    access: acc,
  );
  final raw = await bridge.skillPortableListJson(
    userRoot: userRoot,
    projectRoot: resolved.path,
  );
  return parseSkillsJson(raw);
}

/// Import a portable skill directory into the real `~/.agents/skills/<id>`.
Future<String> importPortableSkillDir({
  required Directory source,
  required String userAgentsSkills,
}) async {
  final skillMd = File('${source.path}/SKILL.md');
  if (!await skillMd.exists()) {
    throw StateError('missing SKILL.md');
  }
  final id = source.uri.pathSegments
      .where((s) => s.isNotEmpty)
      .last;
  if (id.isEmpty || id.startsWith('.')) {
    throw StateError('bad skill id');
  }
  if (id.startsWith('kim-')) {
    throw StateError('kim-* cannot be imported into ~/.agents');
  }
  final destRoot = Directory(userAgentsSkills);
  await destRoot.create(recursive: true);
  final dest = Directory('${destRoot.path}/$id');
  if (await dest.exists()) {
    await dest.delete(recursive: true);
  }
  await _copyDir(source, dest);
  return id;
}

Future<void> _copyDir(Directory from, Directory to) async {
  await to.create(recursive: true);
  await for (final entity in from.list(recursive: false)) {
    final name = entity.uri.pathSegments.where((s) => s.isNotEmpty).last;
    if (name.startsWith('.')) {
      continue;
    }
    if (entity is Directory) {
      await _copyDir(entity, Directory('${to.path}/$name'));
    } else if (entity is File) {
      await entity.copy('${to.path}/$name');
    }
  }
}

SkillRef appSkillRef(CatalogSkill skill) {
  return SkillRef(
    id: skill.id,
    className: SkillRef.classApp,
    origin: skill.origin.isEmpty ? 'bundled' : skill.origin,
    version: skill.version,
    enabled: true,
  );
}
