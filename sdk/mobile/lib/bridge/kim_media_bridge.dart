/// Media port adapter; requires [KimClientBridge] for `mediaUpload`.
library;

import 'dart:io' show Directory, File;

import 'package:kim_mobile/bridge/kim_client_bridge.dart';
import 'package:kim_mobile/core/media.dart';

mixin KimMediaBridge on KimClientBridge implements KimMediaPort {
  @override
  Future<UploadedObject> uploadImage({
    required String token,
    required List<int> bytes,
    required String contentType,
  }) async {
    final _ = token;
    final file = File(
      '${Directory.systemTemp.path}/kim-up-${DateTime.now().microsecondsSinceEpoch}',
    );
    await file.writeAsBytes(bytes, flush: true);
    try {
      final dto = await mediaUpload(
        path: file.path,
        mime: contentType,
        byteSize: bytes.length,
      );
      return UploadedObject(
        key: '',
        url: dto.localPath,
        contentType: contentType,
        bytes: bytes.length,
      );
    } finally {
      if (file.existsSync()) {
        await file.delete();
      }
    }
  }
}
