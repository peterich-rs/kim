/// Runtime permission helpers.
/// Notifications may be requested once at launch. Camera / photos are
/// requested by `kim_media_picker` when 拍摄 / 相册 opens.
library;

import 'package:flutter/services.dart';
import 'package:permission_handler/permission_handler.dart';

import 'settings.dart';

abstract final class KimPermissions {
  static Future<PermissionStatus> requestNotifications() {
    return Permission.notification.request();
  }

  /// Ready for a later picker. Do not call at startup.
  static Future<PermissionStatus> requestPhotos() =>
      Permission.photos.request();

  /// Ready for a later picker. Do not call at startup.
  static Future<PermissionStatus> requestCamera() =>
      Permission.camera.request();

  /// Ready for a later picker. Do not call at startup.
  static Future<PermissionStatus> requestMicrophone() =>
      Permission.microphone.request();

  /// Ask once. If denied / plugin missing, do not spam.
  static Future<void> requestNotificationsOnce(SettingsStore settings) async {
    if (settings.notificationsAsked) {
      return;
    }
    await settings.markNotificationsAsked();
    try {
      final status = await Permission.notification.status;
      if (status.isGranted ||
          status.isLimited ||
          status.isPermanentlyDenied ||
          status.isRestricted) {
        return;
      }
      await Permission.notification.request();
    } on MissingPluginException {
      // Host / widget tests without the plugin.
    } catch (_) {
      // Do not crash the shell over a permission prompt.
    }
  }
}
