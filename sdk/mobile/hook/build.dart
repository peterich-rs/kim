import 'package:code_assets/code_assets.dart';
import 'package:flutter_rust_bridge_hooks/flutter_rust_bridge_hooks.dart';

void main(List<String> args) async {
  await build(args, (input, output) async {
    final extraEnv = _appleLinkeditStripWorkaround(input);
    final rustMode = _rustBuildMode(input);
    await FlutterRustBridgeNativeAssetsBuilder(
      cratePath: 'rust',
      assetName: 'src/rust/frb_generated.io.dart',
      buildMode: rustMode,
      extraCargoEnvironmentVariables: extraEnv,
    ).run(input: input, output: output);
    // Goose is a desktop host (bash/fs/MCP). Phone IM does not ship it.
    if (!_desktopAgentHost(input)) {
      return;
    }
    await FlutterRustBridgeNativeAssetsBuilder(
      cratePath: 'rust_agent',
      assetName: 'src/rust_agent/frb_generated.io.dart',
      buildMode: rustMode,
      extraCargoEnvironmentVariables: extraEnv,
    ).run(input: input, output: output);
  });
}

/// Flutter debug (JIT) sets `linkingEnabled` to false; profile/release (AOT)
/// set it to true. Native Assets no longer pass BuildMode into the hook, so
/// this is the signal that maps Flutter debug → `cargo build` (breakpoints)
/// and Flutter profile/release → `cargo build --release`.
FlutterRustBridgeBuildMode _rustBuildMode(BuildInput input) {
  if (!input.config.buildCodeAssets) {
    return FlutterRustBridgeBuildMode.release;
  }
  return input.config.linkingEnabled
      ? FlutterRustBridgeBuildMode.release
      : FlutterRustBridgeBuildMode.debug;
}

/// Cargo `--release` implicitly passes `-C strip=debuginfo` when `debug` is
/// off. rustc 1.95's llvm-objcopy then emits a Mach-O whose `LC_SYMTAB.stroff`
/// is not 8-byte aligned, and dyld on macOS 27 rejects it with
/// `mis-aligned LINKEDIT string pool` (rust-lang/rust#157750).
///
/// FRB/native_toolchain_rust do not work around this. Disable rustc strip
/// on Apple cdylibs until rustc/LLVM aligns the string pool.
Map<String, String> _appleLinkeditStripWorkaround(BuildInput input) {
  if (!input.config.buildCodeAssets) {
    return const {};
  }
  final os = input.config.code.targetOS;
  if (os != OS.macOS && os != OS.iOS) {
    return const {};
  }
  return const {'CARGO_PROFILE_RELEASE_STRIP': 'none'};
}

bool _desktopAgentHost(BuildInput input) {
  if (!input.config.buildCodeAssets) {
    return false;
  }
  final os = input.config.code.targetOS;
  return os == OS.macOS || os == OS.windows || os == OS.linux;
}
