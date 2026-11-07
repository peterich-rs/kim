/// Media port adapter; requires [KimClientBridge] for `mediaUploadBytes`.
library;

import 'dart:typed_data';

import 'package:kim_mobile/bridge/kim_client_bridge.dart';
import 'package:kim_mobile/core/media.dart';

mixin KimMediaBridge on KimClientBridge implements KimMediaPort {
  @override
  Future<UploadedObject> uploadImage({
    required List<int> bytes,
    required String contentType,
  }) async {
    final dto = await mediaUploadBytes(
      bytes: Uint8List.fromList(bytes),
      mime: contentType,
      width: 0,
      height: 0,
    );
    return UploadedObject(
      key: '',
      url: dto.localPath,
      contentType: contentType,
      bytes: bytes.length,
    );
  }
}
