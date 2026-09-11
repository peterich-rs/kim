import 'package:code_assets/code_assets.dart';
import 'package:flutter_rust_bridge_hooks/flutter_rust_bridge_hooks.dart';

void main(List<String> args) async {
  await build(args, (input, output) async {
    await const FlutterRustBridgeNativeAssetsBuilder(
      cratePath: 'rust',
      assetName: 'src/rust/frb_generated.io.dart',
    ).run(input: input, output: output);
    // Goose is a desktop host (bash/fs/MCP). Phone IM does not ship it.
    if (!_desktopAgentHost(input)) {
      return;
    }
    await const FlutterRustBridgeNativeAssetsBuilder(
      cratePath: 'rust_agent',
      assetName: 'src/rust_agent/frb_generated.io.dart',
    ).run(input: input, output: output);
  });
}

bool _desktopAgentHost(BuildInput input) {
  if (!input.config.buildCodeAssets) {
    return false;
  }
  final os = input.config.code.targetOS;
  return os == OS.macOS || os == OS.windows || os == OS.linux;
}
