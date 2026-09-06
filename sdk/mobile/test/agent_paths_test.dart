import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/core/paths.dart';

void main() {
  test('ensureAgentDirs writes AGENTS.md template', () async {
    final root = await Directory.systemTemp.createTemp('kim-agent-paths');
    addTearDown(() => root.delete(recursive: true));
    final paths = KimPaths.forTest(root);
    await paths.ensureAgentDirs();
    expect(paths.agentSessions.existsSync(), isTrue);
    expect(paths.agentWorkspace.existsSync(), isTrue);
    final md = File('${paths.agentWorkspace.path}/AGENTS.md');
    expect(md.existsSync(), isTrue);
    expect(md.readAsStringSync(), contains('KIM Agent'));
  });
}
