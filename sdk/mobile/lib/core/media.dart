/// Image types + upload port. Bytes go through Rust media_upload.
library;

class UploadedObject {
  const UploadedObject({
    required this.key,
    required this.url,
    required this.contentType,
    required this.bytes,
  });

  final String key;
  final String url;
  final String contentType;
  final int bytes;
}

abstract class KimMediaPort {
  Future<UploadedObject> uploadImage({
    required String token,
    required List<int> bytes,
    required String contentType,
  });
}

/// Worker `kim-media` only accepts these Content-Types.
abstract final class KimImageTypes {
  static const jpeg = 'image/jpeg';
  static const png = 'image/png';
  static const webp = 'image/webp';
  static const gif = 'image/gif';

  static const allowed = {jpeg, 'image/jpg', png, webp, gif};

  static String? sniff(List<int> bytes) {
    if (bytes.length >= 8 &&
        bytes[0] == 0x89 &&
        bytes[1] == 0x50 &&
        bytes[2] == 0x4E &&
        bytes[3] == 0x47) {
      return png;
    }
    if (bytes.length >= 3 &&
        bytes[0] == 0xFF &&
        bytes[1] == 0xD8 &&
        bytes[2] == 0xFF) {
      return jpeg;
    }
    if (bytes.length >= 6 &&
        bytes[0] == 0x47 &&
        bytes[1] == 0x49 &&
        bytes[2] == 0x46 &&
        bytes[3] == 0x38) {
      return gif;
    }
    if (bytes.length >= 12 &&
        bytes[0] == 0x52 &&
        bytes[1] == 0x49 &&
        bytes[2] == 0x46 &&
        bytes[3] == 0x46 &&
        bytes[8] == 0x57 &&
        bytes[9] == 0x45 &&
        bytes[10] == 0x42 &&
        bytes[11] == 0x50) {
      return webp;
    }
    return null;
  }

  static String normalize(String raw, List<int> bytes) {
    var ct = raw.split(';').first.trim().toLowerCase();
    if (ct == 'image/jpg') {
      ct = jpeg;
    }
    if (!allowed.contains(ct)) {
      ct = sniff(bytes) ?? '';
    }
    if (ct == 'image/jpg') {
      ct = jpeg;
    }
    if (!allowed.contains(ct)) {
      throw StateError('unsupported media type');
    }
    return ct;
  }
}
