package com.kim.kim_mobile.ota

import android.util.Log
import io.flutter.embedding.engine.FlutterShellArgs

/**
 * Cold-start hook for Flutter 3.47 Android embedding: prefer OTA `libapp.so`
 * under `filesDir/ota/current/` via `--aot-shared-library-name=`.
 *
 * FlutterLoader.getSafeAotSharedLibraryName only accepts paths under the app
 * [android.content.Context.getFilesDir] that end with `.so`. Our layout
 * satisfies that. If the engine rejects the path, it falls back to the APK
 * builtin `libapp.so` (see FlutterLoader defaults).
 *
 * Debug / JIT hosts do not load AOT libapp; this flag is effectively a
 * release/profile concern.
 *
 * Reflection is **not** required for the happy path: [MainActivity] overrides
 * [io.flutter.embedding.android.FlutterActivity.getFlutterShellArgs] and merges
 * this argument into the [FlutterEngineGroup] dartVmArgs, which
 * FlutterLoader.ensureInitializationComplete validates.
 */
object OtaLibAppHook {
    private const val TAG = "KimOtaLibApp"
    private const val ARG_PREFIX = "--aot-shared-library-name="

    fun mergeShellArgs(base: FlutterShellArgs, gate: OtaGate): FlutterShellArgs {
        val path = gate.libAppShellArgPath() ?: return base
        val arg = ARG_PREFIX + path
        base.add(arg)
        Log.i(TAG, "Requesting AOT libapp from OTA: $path")
        return base
    }
}
