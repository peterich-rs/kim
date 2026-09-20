import 'dart:io';

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
    await _buildCodexHelper(input, extraEnv, rustMode);
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

/// Sandbox re-exec target. Not linked into `rust_agent`. Phone builds skip it.
Future<void> _buildCodexHelper(
  BuildInput input,
  Map<String, String> extraEnv,
  FlutterRustBridgeBuildMode rustMode,
) async {
  final root = _repoRoot();
  final release = rustMode == FlutterRustBridgeBuildMode.release;
  final env = Map<String, String>.from(Platform.environment)..addAll(extraEnv);
  final proc = await Process.start(
    'cargo',
    [
      'build',
      '-p',
      'kim-codex-helper',
      if (release) '--release',
    ],
    workingDirectory: root.path,
    environment: env,
    mode: ProcessStartMode.inheritStdio,
  );
  final code = await proc.exitCode;
  if (code != 0) {
    throw StateError('cargo build -p kim-codex-helper exited $code');
  }
  if (input.config.code.targetOS != OS.linux) {
    return;
  }
  final profile = release ? 'release' : 'debug';
  final dir = Directory('${root.path}/target/$profile');
  final link = Link('${dir.path}/codex-linux-sandbox');
  if (link.existsSync()) {
    link.deleteSync();
  }
  link.createSync('kim-codex-helper');
}

Directory _repoRoot() {
  var dir = Directory.current;
  for (var i = 0; i < 6; i++) {
    if (File('${dir.path}/crates/kim-codex-helper/Cargo.toml').existsSync()) {
      return dir;
    }
    final parent = dir.parent;
    if (parent.path == dir.path) {
      break;
    }
    dir = parent;
  }
  throw StateError(
    'kim repo root not found from ${Directory.current.path}',
  );
}
