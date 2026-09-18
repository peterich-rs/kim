import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

/// Widget tests do not load the macOS workspace plugin; unmocked
/// `invokeMethod` never completes under fake async.
Future<void> testExecutable(Future<void> Function() testMain) async {
  TestWidgetsFlutterBinding.ensureInitialized();
  TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
      .setMockMethodCallHandler(const MethodChannel('kim.workspace'), (
        call,
      ) async {
        return null;
      });
  await testMain();
}
