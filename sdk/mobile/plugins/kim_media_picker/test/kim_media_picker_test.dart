import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:kim_media_picker/kim_media_picker.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  const channel = MethodChannel('kim.media_picker');
  late KimMediaPicker picker;

  setUp(() {
    picker = KimMediaPicker(channel: channel);
  });

  tearDown(() {
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(channel, null);
  });

  test('pickMultiple maps native rows and drops empty paths', () async {
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(channel, (call) async {
          expect(call.method, 'pickMultiple');
          expect(call.arguments, {'maxCount': 9});
          return [
            {
              'id': 'a',
              'path': '/tmp/a.jpg',
              'width': 100,
              'height': 80,
              'size': 12,
              'mimeType': 'image/jpeg',
            },
            {'id': 'b', 'path': '', 'width': 1, 'height': 1, 'size': 1},
          ];
        });
    final got = await picker.pickMultiple();
    expect(got, hasLength(1));
    expect(got.single.path, '/tmp/a.jpg');
    expect(got.single.width, 100);
  });

  test('pickSingle returns the first asset or null', () async {
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(channel, (call) async {
          expect(call.method, 'pickSingle');
          return [
            {
              'id': 'one',
              'path': '/tmp/one.jpg',
              'width': 10,
              'height': 10,
              'size': 2,
              'mimeType': 'image/jpeg',
            },
          ];
        });
    expect((await picker.pickSingle())?.path, '/tmp/one.jpg');
  });

  test('pickMultiple cancel is an empty list', () async {
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(channel, (call) async => <Object?>[]);
    expect(await picker.pickMultiple(maxCount: 3), isEmpty);
  });

  test('takePhoto is capture with photo mode', () async {
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(channel, (call) async {
          expect(call.method, 'capture');
          expect(call.arguments, {'mode': 'photo'});
          return [
            {
              'id': 'shot',
              'path': '/tmp/shot.jpg',
              'width': 1920,
              'height': 1080,
              'size': 99,
              'mimeType': 'image/jpeg',
            },
          ];
        });
    final shot = await picker.takePhoto();
    expect(shot?.path, '/tmp/shot.jpg');
    expect(shot?.isVideo, isFalse);
  });

  test('takeVideo is capture with video mode', () async {
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(channel, (call) async {
          expect(call.method, 'capture');
          expect(call.arguments, {'mode': 'video'});
          return [
            {
              'id': 'clip',
              'path': '/tmp/clip.mp4',
              'width': 1280,
              'height': 720,
              'size': 99,
              'mimeType': 'video/mp4',
              'durationMs': 1500,
            },
          ];
        });
    final clip = await picker.takeVideo();
    expect(clip?.isVideo, isTrue);
    expect(clip?.durationMs, 1500);
  });

  test('capture defaults to mixed', () async {
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(channel, (call) async {
          expect(call.method, 'capture');
          expect(call.arguments, {'mode': 'mixed'});
          return <Object?>[];
        });
    expect(await picker.capture(), isNull);
  });

  test('platform errors become KimMediaPickerException', () async {
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(channel, (call) async {
          throw PlatformException(code: 'permission_denied', message: 'no');
        });
    expect(
      () => picker.takePhoto(),
      throwsA(
        isA<KimMediaPickerException>().having(
          (e) => e.code,
          'code',
          'permission_denied',
        ),
      ),
    );
  });
}
