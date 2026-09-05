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
        // Soft background check after UI is up; apply on next cold start.
        gate.checkInBackground()
    }

    companion object {
        const val CHANNEL = "com.kim.kim_mobile/ota"
    }
}
