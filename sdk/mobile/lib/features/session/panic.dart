library;

import 'package:flutter_riverpod/flutter_riverpod.dart';

final rustPanicProvider = NotifierProvider<RustPanicNotifier, String?>(
  RustPanicNotifier.new,
);

class RustPanicNotifier extends Notifier<String?> {
  @override
  String? build() => null;

  void setMessage(String message) => state = message;
}
