import 'package:custom_lint_builder/custom_lint_builder.dart';

import 'src/avoid_set_state.dart';

PluginBase createPlugin() => _KimLints();

class _KimLints extends PluginBase {
  @override
  List<LintRule> getLintRules(CustomLintConfigs configs) => const [
    AvoidSetState(),
  ];
}
