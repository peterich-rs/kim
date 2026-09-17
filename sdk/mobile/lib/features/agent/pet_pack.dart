library;

import 'dart:convert';

import 'package:flutter/services.dart';

enum PetPhase { idle, running, review, failed, wave }

class PetClip {
  const PetClip({
    required this.row,
    required this.frames,
    required this.fps,
    required this.loop,
  });

  final int row;
  final int frames;
  final double fps;
  final bool loop;

  Duration get duration =>
      Duration(milliseconds: ((frames / fps) * 1000).round());
}

const kDefaultPetFrameWidth = 192;
const kDefaultPetFrameHeight = 208;
const kDefaultPetColumns = 8;
const kDefaultPetRows = 9;

const kDefaultPetClips = <PetPhase, PetClip>{
  PetPhase.idle: PetClip(row: 0, frames: 6, fps: 8.0, loop: true),
  PetPhase.wave: PetClip(row: 3, frames: 4, fps: 8.0, loop: false),
  PetPhase.failed: PetClip(row: 5, frames: 8, fps: 8.0, loop: false),
  PetPhase.running: PetClip(row: 7, frames: 6, fps: 8.0, loop: true),
  PetPhase.review: PetClip(row: 8, frames: 6, fps: 8.0, loop: false),
};

const _kPhaseNames = <PetPhase, List<String>>{
  PetPhase.idle: ['idle'],
  PetPhase.wave: ['waving', 'wave'],
  PetPhase.failed: ['failed'],
  PetPhase.running: ['running'],
  PetPhase.review: ['review'],
};

class PetPack {
  const PetPack({
    required this.id,
    required this.frameWidth,
    required this.frameHeight,
    required this.columns,
    required this.rows,
    required this.clips,
    required this.imageBytes,
  });

  final String id;
  final int frameWidth;
  final int frameHeight;
  final int columns;
  final int rows;
  final Map<PetPhase, PetClip> clips;
  final Uint8List imageBytes;

  PetClip clipFor(PetPhase phase) =>
      clips[phase] ?? clips[PetPhase.idle] ?? kDefaultPetClips[PetPhase.idle]!;

  Rect srcRect(PetPhase phase, int frame) {
    final clip = clipFor(phase);
    final n = clip.frames <= 0 ? 1 : clip.frames;
    final col = ((frame % n) + n) % n;
    return Rect.fromLTWH(
      col * frameWidth.toDouble(),
      clip.row * frameHeight.toDouble(),
      frameWidth.toDouble(),
      frameHeight.toDouble(),
    );
  }

  static PetPack parseJson(Map<dynamic, dynamic> json, Uint8List imageBytes) {
    final id = json['id'] is String && (json['id'] as String).isNotEmpty
        ? json['id'] as String
        : 'default';
    final frame = json['frame'];
    var frameWidth = kDefaultPetFrameWidth;
    var frameHeight = kDefaultPetFrameHeight;
    if (frame is Map) {
      frameWidth = _asInt(frame['width'], kDefaultPetFrameWidth);
      frameHeight = _asInt(frame['height'], kDefaultPetFrameHeight);
    }
    final columns = _asInt(json['columns'], kDefaultPetColumns);
    final rows = _asInt(json['rows'], kDefaultPetRows);

    final clips = <PetPhase, PetClip>{};
    final animations = json['animations'];
    if (animations is Map) {
      final idle =
          _clipForPhase(animations, PetPhase.idle) ??
          kDefaultPetClips[PetPhase.idle]!;
      clips[PetPhase.idle] = idle;
      for (final phase in PetPhase.values) {
        if (phase == PetPhase.idle) {
          continue;
        }
        clips[phase] = _clipForPhase(animations, phase) ?? idle;
      }
    } else {
      clips.addAll(kDefaultPetClips);
    }

    return PetPack(
      id: id,
      frameWidth: frameWidth <= 0 ? kDefaultPetFrameWidth : frameWidth,
      frameHeight: frameHeight <= 0 ? kDefaultPetFrameHeight : frameHeight,
      columns: columns <= 0 ? kDefaultPetColumns : columns,
      rows: rows <= 0 ? kDefaultPetRows : rows,
      clips: clips,
      imageBytes: imageBytes,
    );
  }

  static Future<PetPack> loadAsset(
    AssetBundle bundle, {
    String id = 'default',
  }) async {
    final raw = await bundle.loadString('assets/pets/$id/pet.json');
    final decoded = jsonDecode(raw);
    if (decoded is! Map) {
      throw const FormatException('pet.json must be an object');
    }
    final sheetRaw = decoded['spritesheetPath'];
    var sheet = sheetRaw is String && sheetRaw.isNotEmpty
        ? sheetRaw
        : 'spritesheet.png';
    final file = sheet.split('/').last;
    final key = 'assets/pets/$id/$file';
    ByteData data;
    try {
      data = await bundle.load(key);
    } catch (_) {
      if (file.endsWith('.webp')) {
        data = await bundle.load(
          'assets/pets/$id/${file.substring(0, file.length - 5)}.png',
        );
      } else {
        rethrow;
      }
    }
    final bytes = data.buffer.asUint8List(
      data.offsetInBytes,
      data.lengthInBytes,
    );
    return parseJson(Map<dynamic, dynamic>.from(decoded), bytes);
  }
}

PetClip? _clipForPhase(Map<dynamic, dynamic> animations, PetPhase phase) {
  final names = _kPhaseNames[phase] ?? const <String>[];
  for (final name in names) {
    final clip = _parseClip(
      animations[name],
      loop: phase == PetPhase.idle || phase == PetPhase.running,
    );
    if (clip != null) {
      return clip;
    }
  }
  return null;
}

PetClip? _parseClip(Object? raw, {required bool loop}) {
  if (raw is! Map) {
    return null;
  }
  final loopValue = raw['loop'];
  final frames = _asInt(raw['frames'], 1);
  final fps = _asDouble(raw['fps'], 8);
  return PetClip(
    row: _asInt(raw['row'], 0),
    frames: frames <= 0 ? 1 : frames,
    fps: fps <= 0 ? 8 : fps,
    loop: loopValue is bool ? loopValue : loop,
  );
}

int _asInt(Object? value, int fallback) {
  if (value is int) {
    return value;
  }
  if (value is num) {
    return value.round();
  }
  return fallback;
}

double _asDouble(Object? value, double fallback) {
  if (value is num) {
    return value.toDouble();
  }
  return fallback;
}
