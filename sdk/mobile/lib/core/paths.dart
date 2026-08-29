/// App support / documents / cache / temp via path_provider.
/// Held on the Dart side for a later SQLite path; not passed into KimApi yet.
library;

import 'dart:io';

import 'package:path_provider/path_provider.dart';

class KimPaths {
  KimPaths._({
    required this.documents,
    required this.support,
    required this.cache,
    required this.temp,
  });

  final Directory documents;
  final Directory support;
  final Directory cache;
  final Directory temp;

  static KimPaths? _instance;

  static KimPaths get instance {
    final current = _instance;
    if (current == null) {
      throw StateError('KimPaths.ensure() has not run');
    }
    return current;
  }

  /// Documents + support + cache + temp, creating missing dirs.
  static Future<KimPaths> ensure() async {
    if (_instance != null) {
      return _instance!;
    }
    final documents = await getApplicationDocumentsDirectory();
    final support = await getApplicationSupportDirectory();
    final cache = await getApplicationCacheDirectory();
    final temp = await getTemporaryDirectory();
    await Future.wait([
      documents.create(recursive: true),
      support.create(recursive: true),
      cache.create(recursive: true),
      temp.create(recursive: true),
    ]);
    _instance = KimPaths._(
      documents: documents,
      support: support,
      cache: cache,
      temp: temp,
    );
    return _instance!;
  }

  /// Widget/unit tests: skip path_provider.
  factory KimPaths.forTest(Directory root) {
    Directory dir(String name) {
      final d = Directory('${root.path}/$name');
      d.createSync(recursive: true);
      return d;
    }

    final paths = KimPaths._(
      documents: dir('Documents'),
      support: dir('Support'),
      cache: dir('Cache'),
      temp: dir('Temp'),
    );
    _instance = paths;
    return paths;
  }

  /// Last two path segments, for the shell status line.
  String get dataDirShort => shorten(support.path);

  static String shorten(String path) {
    final parts = path
        .split(RegExp(r'[/\\]'))
        .where((p) => p.isNotEmpty)
        .toList();
    if (parts.length <= 2) {
      return path;
    }
    return '…/${parts[parts.length - 2]}/${parts.last}';
  }
}
