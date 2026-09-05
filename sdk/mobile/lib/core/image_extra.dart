/// Compact `{"w":n,"h":n}` on `MessageReq.extra` for type=2 image talks.
library;

import 'dart:convert';

import '../copy.dart';
import '../models/models.dart';
import 'format.dart';

const mediaHost = 'media.kim.ainexc.com';

final _imageExt = RegExp(
  r'\.(?:jpe?g|png|webp|gif)(?:\?|$)',
  caseSensitive: false,
);

final _videoExt = RegExp(
  r'\.(?:mp4|mov|webm|m4v)(?:\?|$)',
  caseSensitive: false,
);

class ImageSize {
  const ImageSize({required this.width, required this.height});

  final int width;
  final int height;
}

String encodeImageExtra({required int width, required int height}) {
  if (width <= 0 || height <= 0) {
    return '';
  }
  return jsonEncode({'w': width, 'h': height});
}

ImageSize? parseImageExtra(String extra) {
  final raw = extra.trim();
  if (!raw.startsWith('{')) {
    return null;
  }
  try {
    final decoded = jsonDecode(raw);
    if (decoded is! Map) {
      return null;
    }
    final w = decoded['w'];
    final h = decoded['h'];
    if (w is! num || h is! num || w <= 0 || h <= 0) {
      return null;
    }
    return ImageSize(width: w.round(), height: h.round());
  } catch (_) {
    return null;
  }
}

bool isMediaUrl(String body) {
  final url = body.trim();
  if (!url.startsWith('http://') && !url.startsWith('https://')) {
    return false;
  }
  final uri = Uri.tryParse(url);
  if (uri == null) {
    return _imageExt.hasMatch(url);
  }
  if (uri.host == mediaHost) {
    return true;
  }
  return _imageExt.hasMatch(uri.path);
}

bool isRemoteUrl(String body) {
  final url = body.trim();
  return url.startsWith('http://') || url.startsWith('https://');
}

KimMsgKind kindFromWire({
  required String body,
  required String extra,
  int type = 0,
}) {
  if (type == 4) {
    return KimMsgKind.video;
  }
  if (type == 2 || parseImageExtra(extra) != null || isMediaUrl(body)) {
    return KimMsgKind.image;
  }
  return KimMsgKind.text;
}

String previewBody(KimChatMsg msg) {
  if (msg.sys) {
    return msg.body;
  }
  if (msg.isVideo) {
    return Copy.videoMessage;
  }
  if (msg.isImage) {
    return Copy.imageMessage;
  }
  return previewSnippet(msg.body);
}

/// List preview for a stored last_body / inbox lastBody. Never show a media URL.
String previewSnippet(String body) {
  final text = body.trim();
  if (text.isEmpty) {
    return '';
  }
  if (text == Copy.imageMessage || text == Copy.videoMessage) {
    return text;
  }
  if (_videoExt.hasMatch(text)) {
    return Copy.videoMessage;
  }
  if (isMediaUrl(text) || _imageExt.hasMatch(text)) {
    return Copy.imageMessage;
  }
  return truncate(text);
}
