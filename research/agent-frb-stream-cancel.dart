// Diagnostic for the pinned flutter_rust_bridge 2.13.0 RustStreamSink.
// Run from the repository root:
// dart --packages=sdk/mobile/.dart_tool/package_config.json research/agent-frb-stream-cancel.dart
// Posts synthetic events into this process only; does not load KIM or call an LLM.
// The final wake-up event models AgentSession.close() -> abort() -> aborted.

import 'dart:async';
import 'dart:ffi';

import 'package:ffi/ffi.dart';
import 'package:flutter_rust_bridge/src/codec/base.dart';
import 'package:flutter_rust_bridge/src/generalized_frb_rust_binding/generalized_frb_rust_binding.dart';
import 'package:flutter_rust_bridge/src/stream/stream_sink.dart';

// Dart_CObject prefix for kString, with enough storage for its native union.
final class CObject extends Struct {
  @Int32()
  external int type;
  external Pointer<Utf8> value;
  @Array(32)
  external Array<Uint8> rest;
}

class TestCodec extends BaseCodec<String, String, Object> {
  const TestCodec();
  @override
  String decodeObject(dynamic raw) => raw as String;
  @override
  String decodeWireSyncType(Object raw) => throw UnimplementedError();
  @override
  void freeWireSyncRust2Dart(Object raw, GeneralizedFrbRustBinding binding) {}
}

final postObject = NativeApi.postCObject
    .cast<NativeFunction<Bool Function(Int64, Pointer<CObject>)>>()
    .asFunction<bool Function(int, Pointer<CObject>)>();
void send(int port, String value) {
  final object = calloc<CObject>();
  final string = value.toNativeUtf8();
  object.ref.type = 5;
  object.ref.value = string;
  final ok = postObject(port, object);
  calloc.free(object);
  malloc.free(string);
  if (!ok) throw StateError('post failed');
}

Future<bool> probe({required bool closeFirst}) async {
  final sink = RustStreamSink<String>();
  final port = int.parse(sink.setupAndSerialize(codec: TestCodec()));
  final inbox = StreamController<String>();
  final sub = sink.stream.listen(
    inbox.add,
    onError: inbox.addError,
    onDone: inbox.close,
  );
  final watch = Stopwatch()..start();
  var neededWake = false;
  final wake = Timer(Duration(milliseconds: 800), () {
    neededWake = true;
    print('  TIMEOUT: no completion after 800ms; wake native stream');
    send(port, 'aborted');
  });
  send(port, 'assistant_finished');
  try {
    await for (final ev in inbox.stream) {
      print('  received $ev');
      break;
    }
  } finally {
    if (closeFirst) send(port, 'aborted');
    print('  before cancel');
    await sub.cancel();
    print('  after cancel (${watch.elapsedMilliseconds}ms)');
    await inbox.close();
    wake.cancel();
  }
  return neededWake;
}

Future<void> main() async {
  print('Actual flutter_rust_bridge 2.13 RustStreamSink; current order:');
  final currentBlocked = await probe(closeFirst: false);
  print('Close signal before cancellation:');
  final closeFirstBlocked = await probe(closeFirst: true);
  if (!currentBlocked || closeFirstBlocked) {
    throw StateError(
      "Observed behavior differs from the reported cancellation dependency",
    );
  }
  print(
    "PASS: current cleanup waits for another event; close signal releases it.",
  );
}
