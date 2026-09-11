import 'package:flutter/foundation.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/agent/host_support.dart';

void main() {
  tearDown(() {
    debugDefaultTargetPlatformOverride = null;
  });

  test('Goose host is desktop only', () {
    debugDefaultTargetPlatformOverride = TargetPlatform.iOS;
    expect(agentHostSupported, isFalse);
    debugDefaultTargetPlatformOverride = TargetPlatform.android;
    expect(agentHostSupported, isFalse);
    debugDefaultTargetPlatformOverride = TargetPlatform.macOS;
    expect(agentHostSupported, isTrue);
    debugDefaultTargetPlatformOverride = TargetPlatform.windows;
    expect(agentHostSupported, isTrue);
    debugDefaultTargetPlatformOverride = TargetPlatform.linux;
    expect(agentHostSupported, isTrue);
  });
}
