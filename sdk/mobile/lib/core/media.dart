/// Image upload to kim-media. Bytes never go through WGateway.
library;

import 'dart:convert';
import 'dart:io';
import 'dart:typed_data';

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

class KimMediaClient implements KimMediaPort {
  KimMediaClient({this.origin = defaultOrigin, this._http});

  static const defaultOrigin = 'https://upload.kim.ainexc.com';
  static const maxBytes = 5 * 1024 * 1024;

  final String origin;
  final HttpClient? _http;

  @override
  Future<UploadedObject> uploadImage({
    required String token,
    required List<int> bytes,
    required String contentType,
  }) async {
    if (token.trim().isEmpty) {
      throw StateError('JWT required');
    }
    if (bytes.isEmpty) {
      throw StateError('empty body');
    }
    if (bytes.length > maxBytes) {
      throw StateError('too large');
    }
    final payload = bytes is Uint8List ? bytes : Uint8List.fromList(bytes);
    final ct = KimImageTypes.normalize(contentType, payload);
    final uri = Uri.parse('${origin.replaceAll(RegExp(r'/$'), '')}/v1/objects');
    final client = _http ?? HttpClient();
    final owned = _http == null;
    try {
      final req = await client.postUrl(uri);
      req.headers.set(HttpHeaders.authorizationHeader, 'Bearer $token');
      req.headers.set(
        HttpHeaders.contentTypeHeader,
        ct.split(';').first.trim(),
      );
      req.contentLength = payload.length;
      req.add(payload);
      final resp = await req.close();
      final body = await utf8.decodeStream(resp);
      if (resp.statusCode < 200 || resp.statusCode >= 300) {
        throw StateError('upload ${resp.statusCode}: $body');
      }
      final decoded = jsonDecode(body);
      if (decoded is! Map) {
        throw StateError('upload: bad response');
      }
      final url = decoded['url'];
      if (url is! String || url.isEmpty) {
        throw StateError('upload: missing url');
      }
      return UploadedObject(
        key: decoded['key'] is String ? decoded['key'] as String : '',
        url: url,
        contentType: decoded['contentType'] is String
            ? decoded['contentType'] as String
            : ct,
        bytes: decoded['bytes'] is num
            ? (decoded['bytes'] as num).toInt()
            : bytes.length,
      );
    } finally {
      if (owned) {
        client.close(force: true);
      }
    }
  }
}
