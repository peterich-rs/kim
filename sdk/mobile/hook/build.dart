import 'package:flutter_rust_bridge_hooks/flutter_rust_bridge_hooks.dart';

void main(List<String> args) async {
  await build(args, (input, output) async {
    await const FlutterRustBridgeNativeAssetsBuilder(
      cratePath: 'rust',
      assetName: 'src/rust/frb_generated.io.dart',
    ).run(input: input, output: output);
    await const FlutterRustBridgeNativeAssetsBuilder(
      cratePath: 'rust_agent',
      assetName: 'src/rust_agent/frb_generated.io.dart',
    ).run(input: input, output: output);
  });
}
