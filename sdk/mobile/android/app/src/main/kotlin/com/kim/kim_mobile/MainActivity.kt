package com.kim.kim_mobile

import com.kim.kim_mobile.ota.OtaGate
import com.kim.kim_mobile.ota.OtaLibAppHook
import io.flutter.embedding.android.FlutterActivity
import io.flutter.embedding.engine.FlutterEngine
import io.flutter.embedding.engine.FlutterShellArgs
import io.flutter.plugin.common.MethodChannel

class MainActivity : FlutterActivity() {
    override fun getFlutterShellArgs(): FlutterShellArgs {
        val base = super.getFlutterShellArgs()
        return OtaLibAppHook.mergeShellArgs(base, OtaGate.get(this))
    }

    override fun configureFlutterEngine(flutterEngine: FlutterEngine) {
        super.configureFlutterEngine(flutterEngine)
        val gate = OtaGate.get(this)
        MethodChannel(flutterEngine.dartExecutor.binaryMessenger, CHANNEL)
            .setMethodCallHandler { call, result ->
                when (call.method) {
                    "getStatus" -> result.success(gate.status().toMap())
                    "markHealthy" -> {
                        gate.markHealthy()
                        result.success(null)
                    }
                    "checkNow" -> {
                        gate.checkInBackground()
                        result.success(null)
                    }
                    else -> result.notImplemented()
                }
            }
        MethodChannel(flutterEngine.dartExecutor.binaryMessenger, KEYSTORE_CHANNEL)
            .setMethodCallHandler { call, result ->
                val key = call.argument<String>("key")
                if (key == null) {
                    result.error("bad_args", "key required", null)
                    return@setMethodCallHandler
                }
                when (call.method) {
                    "read" -> result.success(KimKeystore.read(this, key))
                    "write" -> {
                        val value = call.argument<String>("value")
                        if (value == null) {
                            result.error("bad_args", "value required", null)
                        } else {
                            try {
                                KimKeystore.write(this, key, value)
                                result.success(null)
                            } catch (e: Exception) {
                                result.error("keystore", e.message, null)
                            }
                        }
                    }
                    "delete" -> {
                        KimKeystore.delete(this, key)
                        result.success(null)
                    }
                    else -> result.notImplemented()
                }
            }
        // Soft background check after UI is up; apply on next cold start.
        gate.checkInBackground()
    }

    companion object {
        const val CHANNEL = "com.kim.kim_mobile/ota"
        const val KEYSTORE_CHANNEL = "kim.keystore"
    }
}
