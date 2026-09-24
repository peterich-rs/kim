import 'package:analyzer/error/error.dart' show DiagnosticSeverity;
import 'package:analyzer/error/listener.dart';
import 'package:custom_lint_builder/custom_lint_builder.dart';

/// Flags `setState` under `lib/features/`. Design widgets and files marked
/// `// kim_lint: allow_ephemeral_set_state` are skipped.
class AvoidSetState extends DartLintRule {
  const AvoidSetState() : super(code: _code);

  static const _code = LintCode(
    name: 'avoid_set_state',
    problemMessage:
        'Business UI state belongs in a Riverpod Notifier, not setState.',
    correctionMessage:
        'Move the field into a Notifier, or mark the file '
        '`// kim_lint: allow_ephemeral_set_state` when the state is only '
        'focus, scroll, or animation.',
    errorSeverity: DiagnosticSeverity.ERROR,
  );

  @override
  void run(
    CustomLintResolver resolver,
    DiagnosticReporter reporter,
    CustomLintContext context,
  ) {
    final path = resolver.source.fullName.replaceAll('\\', '/');
    final content = resolver.source.contents.data;
    final inFeatures = path.contains('/lib/features/');
    final fixture = path.endsWith('/avoid_set_state.dart');
    if (!inFeatures && !fixture) {
      return;
    }
    if (content.contains('kim_lint: allow_ephemeral_set_state')) {
      return;
    }
    context.registry.addMethodInvocation((node) {
      if (node.methodName.name != 'setState') {
        return;
      }
      reporter.atNode(node.methodName, _code);
    });
  }
}
