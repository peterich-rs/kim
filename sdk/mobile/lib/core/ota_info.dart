/// Read-only Logic SO OTA status from Android [MethodChannel].
library;

import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';

class OtaInfo {
  const OtaInfo({
    required this.active,
    required this.logicVersion,
    required this.ffiLoadedFromOta,
    required this.ffiPath,
    required this.libAppPath,
    required this.hostLine,
    required this.engineBuildId,
  });

  final bool active;
  final String? logicVersion;
  final bool ffiLoadedFromOta;
  final String? ffiPath;
  final String? libAppPath;
  final String hostLine;
  final String engineBuildId;

  static const none = OtaInfo(
    active: false,
    logicVersion: null,
    ffiLoadedFromOta: false,
    ffiPath: null,
    libAppPath: null,
    hostLine: '',
    engineBuildId: '',
  );

  String get debugLabel {
    if (!active) {
      return 'OTA: off';
    }
    final v = logicVersion ?? '?';
    return 'OTA: $v';
  }

  factory OtaInfo.fromMap(Map<dynamic, dynamic> map) {
    return OtaInfo(
      active: map['active'] == true,
      logicVersion: map['logicVersion'] as String?,
      ffiLoadedFromOta: map['ffiLoadedFromOta'] == true,
      ffiPath: map['ffiPath'] as String?,
      libAppPath: map['libAppPath'] as String?,
      hostLine: (map['hostLine'] as String?) ?? '',
      engineBuildId: (map['engineBuildId'] as String?) ?? '',
    );
  }
}

class OtaBridge {
  static const _channel = MethodChannel('com.kim.kim_mobile/ota');

  static Future<OtaInfo> status() async {
    if (kIsWeb || !Platform.isAndroid) {
      return OtaInfo.none;
    }
    try {
      final raw = await _channel.invokeMethod<dynamic>('getStatus');
      if (raw is Map) {
        return OtaInfo.fromMap(raw);
      }
    } catch (e, st) {
      debugPrint('OtaBridge.status failed: $e\n$st');
    }
    return OtaInfo.none;
  }

  static Future<void> markHealthy() async {
    if (kIsWeb || !Platform.isAndroid) {
      return;
    }
    try {
      await _channel.invokeMethod<void>('markHealthy');
    } catch (e) {
      debugPrint('OtaBridge.markHealthy failed: $e');
    }
  }
}
