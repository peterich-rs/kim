import 'package:flutter/cupertino.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/router/kim_page.dart';

void main() {
  tearDown(() {
    debugDefaultTargetPlatformOverride = null;
  });

  test('kimPushPage is CupertinoPage on iOS', () {
    debugDefaultTargetPlatformOverride = TargetPlatform.iOS;
    final page = kimPushPage(
      key: const ValueKey('chat'),
      child: const SizedBox(),
    );
    expect(page, isA<CupertinoPage<void>>());
  });

  test('kimPushPage is CupertinoPage on macOS', () {
    debugDefaultTargetPlatformOverride = TargetPlatform.macOS;
    final page = kimPushPage(
      key: const ValueKey('chat'),
      child: const SizedBox(),
    );
    expect(page, isA<CupertinoPage<void>>());
  });

  test('kimPushPage is MaterialPage on Android', () {
    debugDefaultTargetPlatformOverride = TargetPlatform.android;
    final page = kimPushPage(
      key: const ValueKey('chat'),
      child: const SizedBox(),
    );
    expect(page, isA<MaterialPage<void>>());
  });
}
