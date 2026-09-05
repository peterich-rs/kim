package com.kim.kim_mobile

import android.app.Application
import android.util.Log
import com.kim.kim_mobile.ota.OtaGate

/**
 * Early process bootstrap for logic SO OTA. Must run before FlutterLoader
 * initializes the engine so [System.load] of OTA `libkim_client_ffi.so` wins.
 */
class KimApplication : Application() {
    override fun onCreate() {
        super.onCreate()
        try {
            OtaGate.get(this).bootstrap()
        } catch (t: Throwable) {
            Log.e(TAG, "OTA bootstrap failed — continuing with APK builtins", t)
        }
    }

    companion object {
        private const val TAG = "KimApplication"
    }
}
