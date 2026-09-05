package com.kim.kim_mobile.ota

import android.content.Context
import android.content.SharedPreferences
import java.io.File

/**
 * On-disk layout under filesDir/ota/{staging,current,previous} plus prefs for
 * logic_version / crash marker.
 */
class OtaStore(context: Context) {
    private val root: File = File(context.filesDir, "ota").also { it.mkdirs() }
    val staging: File = File(root, "staging").also { it.mkdirs() }
    val current: File = File(root, "current").also { it.mkdirs() }
    val previous: File = File(root, "previous").also { it.mkdirs() }

    private val prefs: SharedPreferences =
        context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)

    fun currentLibApp(): File = File(current, OtaAllowed.LIB_APP)

    fun currentLibFfi(): File = File(current, OtaAllowed.LIB_FFI)

    fun hasCurrentPackage(): Boolean =
        currentLibApp().isFile && currentLibFfi().isFile

    var logicVersion: String?
        get() = prefs.getString(KEY_LOGIC_VERSION, null)
        set(value) {
            prefs.edit().putString(KEY_LOGIC_VERSION, value).apply()
        }

    var otaActive: Boolean
        get() = prefs.getBoolean(KEY_OTA_ACTIVE, false)
        set(value) {
            prefs.edit().putBoolean(KEY_OTA_ACTIVE, value).apply()
        }

    var crashPending: Boolean
        get() = prefs.getBoolean(KEY_CRASH_PENDING, false)
        set(value) {
            prefs.edit().putBoolean(KEY_CRASH_PENDING, value).apply()
        }

    fun clearCurrent() {
        current.listFiles()?.forEach { it.delete() }
        otaActive = false
        logicVersion = null
        crashPending = false
    }

    /** Atomic-ish promote: current → previous, staging → current. */
    fun promoteStagingToCurrent(newLogicVersion: String) {
        previous.listFiles()?.forEach { it.delete() }
        current.listFiles()?.forEach { src ->
            src.copyTo(File(previous, src.name), overwrite = true)
            src.delete()
        }
        staging.listFiles()?.forEach { src ->
            src.copyTo(File(current, src.name), overwrite = true)
            src.delete()
        }
        logicVersion = newLogicVersion
        otaActive = true
    }

    fun clearStaging() {
        staging.listFiles()?.forEach { it.deleteRecursively() }
        staging.mkdirs()
    }

    companion object {
        private const val PREFS = "kim_logic_ota"
        private const val KEY_LOGIC_VERSION = "logic_version"
        private const val KEY_OTA_ACTIVE = "ota_active"
        private const val KEY_CRASH_PENDING = "ota_crash_pending"
    }
}
