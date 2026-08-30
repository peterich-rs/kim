/// Native camera + album picker. UI lives on Android (CameraX) and iOS
/// (AVFoundation / PhotoKit); Dart only forwards the method channel.
library;

import 'package:flutter/services.dart';

const _channelName = 'kim.media_picker';

/// How the camera shutter behaves.
enum KimCaptureMode {
  /// Photo only. Tap takes a still.
  photo,

  /// Video only. Tap starts / stops like a stock camera app.
  video,

  /// Photo + video. Bottom switcher: 拍照 / 录像.
  /// 拍照: tap still, long-press records (WeChat).
  /// 录像: tap starts / stops.
  mixed,
}

class KimMediaAsset {
  const KimMediaAsset({
    required this.id,
    required this.path,
    required this.width,
    required this.height,
    required this.size,
    required this.mimeType,
    this.durationMs = 0,
  });

  final String id;
  final String path;
  final int width;
  final int height;
  final int size;
  final String mimeType;
  final int durationMs;

  bool get isVideo => mimeType.startsWith('video/');

  factory KimMediaAsset.fromMap(Map<Object?, Object?> raw) {
    return KimMediaAsset(
      id: raw['id'] as String? ?? '',
      path: raw['path'] as String? ?? '',
      width: (raw['width'] as num?)?.toInt() ?? 0,
      height: (raw['height'] as num?)?.toInt() ?? 0,
      size: (raw['size'] as num?)?.toInt() ?? 0,
      mimeType: raw['mimeType'] as String? ?? 'image/jpeg',
      durationMs: (raw['durationMs'] as num?)?.toInt() ?? 0,
    );
  }

  Map<String, Object?> toMap() => {
    'id': id,
    'path': path,
    'width': width,
    'height': height,
    'size': size,
    'mimeType': mimeType,
    'durationMs': durationMs,
  };
}

class KimMediaPickerException implements Exception {
  const KimMediaPickerException(this.code, this.message);

  final String code;
  final String message;

  @override
  String toString() => 'KimMediaPickerException($code, $message)';
}

class KimMediaPicker {
  KimMediaPicker({MethodChannel? channel})
    : _channel = channel ?? const MethodChannel(_channelName);

  static final KimMediaPicker instance = KimMediaPicker();

  final MethodChannel _channel;

  /// Album, one item. Null means cancelled.
  Future<KimMediaAsset?> pickSingle() async {
    final list = _parseList(await _invoke('pickSingle', const {}));
    return list.isEmpty ? null : list.first;
  }

  /// Album, multi-select. Empty list means cancelled.
  Future<List<KimMediaAsset>> pickMultiple({int maxCount = 9}) async {
    final count = maxCount < 1 ? 1 : maxCount;
    return _parseList(await _invoke('pickMultiple', {'maxCount': count}));
  }

  /// Camera. Null means cancelled.
  ///
  /// [KimCaptureMode.mixed] is the WeChat chat shutter: photo / video switch,
  /// tap photo, long-press video, tap-to-record in 录像.
  Future<KimMediaAsset?> capture({
    KimCaptureMode mode = KimCaptureMode.mixed,
  }) async {
    final list = _parseList(await _invoke('capture', {'mode': mode.name}));
    return list.isEmpty ? null : list.first;
  }

  /// Photo-only camera.
  Future<KimMediaAsset?> takePhoto() => capture(mode: KimCaptureMode.photo);

  /// Video-only camera. Tap starts / stops.
  Future<KimMediaAsset?> takeVideo() => capture(mode: KimCaptureMode.video);

  Future<Object?> _invoke(String method, Map<String, Object?> args) async {
    try {
      return await _channel.invokeMethod<Object?>(method, args);
    } on MissingPluginException {
      rethrow;
    } on PlatformException catch (err) {
      throw KimMediaPickerException(err.code, err.message ?? err.code);
    }
  }

  List<KimMediaAsset> _parseList(Object? raw) {
    if (raw is! List) {
      return const [];
    }
    final out = <KimMediaAsset>[];
    for (final row in raw) {
      if (row is Map<Object?, Object?>) {
        final asset = KimMediaAsset.fromMap(row);
        if (asset.path.isNotEmpty) {
          out.add(asset);
        }
      }
    }
    return out;
  }
}
