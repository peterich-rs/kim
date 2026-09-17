/// App / portable skill catalog helpers for skills page + plaza (S-KD 3 / 7 / 9).
library;

import 'dart:convert';
import 'dart:io';

import 'package:kim_mobile/bridge/goose_bridge.dart';
import 'package:kim_mobile/core/paths.dart';
import 'package:kim_mobile/features/agent/agent_profiles.dart';
import 'package:kim_mobile/features/agent/workspace.dart';
import 'package:kim_mobile/features/agent/workspace_access.dart';

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

  /// Second line of plaza / skills rows: frontmatter `description`.
  String get listDescription => description.trim();

  /// Where this package was found. UI chips: internal / download / global / project.
  SkillShelf get shelf => skillShelfOf(this);
}

/// Skill package source shown as a chip in pickers.
enum SkillShelf { internal, download, global, project }

SkillShelf skillShelfOf(CatalogSkill skill) {
  final origin = skill.origin.trim().toLowerCase();
  if (skill.isApp) {
    if (origin == 'cache' || origin == 'cloud' || origin == 'download') {
      return SkillShelf.download;
    }
    return SkillShelf.internal;
  }
  if (origin == 'project') {
    return SkillShelf.project;
  }
  return SkillShelf.global;
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
  final t = projectToolSet(profile.resolveCapabilities());
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
  return [
    for (final n in need)
      if (!have.contains(n)) n,
  ];
}

AgentToolSet enableRequiredTools(AgentToolSet tools, List<String> missing) {
  return projectToolSet(
    enableRequiredCapabilities(
      capabilitiesFromLegacy(tools, const []),
      missing,
    ),
  );
}

/// Plaza / skills assign: persist required caps (not tools-only) + skill ref.
AgentProfile assignAppSkillToProfile({
  required AgentProfile profile,
  required CatalogSkill skill,
  required bool enableMissingTools,
}) {
  var caps = List<CapabilityRef>.from(profile.resolveCapabilities());
  var perms = Map<String, String>.from(profile.permissionOverrides);
  if (enableMissingTools) {
    final missing = missingToolsForAppSkill(profile, skill.id);
    if (missing.isNotEmpty) {
      caps = enableRequiredCapabilities(caps, missing);
      if (missing.contains('bash')) {
        perms['bash'] = perms['bash'] == 'never_allow'
            ? 'never_allow'
            : 'ask_before';
      }
    }
  }
  final skills = [
    for (final s in profile.skills)
      if (s.id != skill.id) s,
    appSkillRef(skill),
  ];
  return profile
      .withCapabilities(caps)
      .copyWith(skills: skills, permissionOverrides: perms);
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

Future<List<CatalogSkill>> loadPortableSkills({
  required AgentBridge bridge,
  String userRoot = '',
  String projectRoot = '',
}) async {
  if (userRoot.trim().isEmpty && projectRoot.trim().isEmpty) {
    return const [];
  }
  await bridge.ensure();
  final raw = await bridge.skillPortableListJson(
    userRoot: userRoot,
    projectRoot: projectRoot,
  );
  return parseSkillsJson(raw);
}

Future<List<CatalogSkill>> loadPortableSkillCatalog({
  required AgentBridge bridge,
  required AgentProfile profile,
  KimPaths? paths,
  WorkspaceAccess? access,
}) async {
  final p = paths ?? KimPaths.instance;
  final acc = access ?? workspaceAccess;
  final userRoot = await acc.realUserAgentsSkills() ?? '';
  var projectRoot = '';
  if (profile.workspace.isRepo) {
    final resolved = await resolveAgentProjectRoot(
      profile: profile,
      paths: p,
      access: acc,
    );
    if (!resolved.invalidRepo) {
      projectRoot = resolved.path;
    }
  }
  return loadPortableSkills(
    bridge: bridge,
    userRoot: userRoot,
    projectRoot: projectRoot,
  );
}

/// App catalog first; portable fills remaining ids. Sorted by name.
List<CatalogSkill> mergeSkillCatalogs({
  required List<CatalogSkill> app,
  required List<CatalogSkill> portable,
}) {
  final byId = <String, CatalogSkill>{};
  for (final skill in portable) {
    byId[skill.id] = skill;
  }
  for (final skill in app) {
    byId[skill.id] = skill;
  }
  final out = byId.values.toList()
    ..sort((a, b) => a.name.toLowerCase().compareTo(b.name.toLowerCase()));
  return out;
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
  final id = source.uri.pathSegments.where((s) => s.isNotEmpty).last;
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
