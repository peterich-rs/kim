package com.kim.kim_mobile.ota

import android.content.Context
import android.util.Log
import java.io.BufferedInputStream
import java.io.File
import java.io.FileInputStream
import java.io.FileOutputStream
import java.net.HttpURLConnection
import java.net.URL
import java.util.zip.ZipInputStream
import kotlin.concurrent.thread

/**
 * Logic SO OTA gate: crash-loop protection, early FFI [System.load], check /
 * download / verify / atomic install. Libapp path is consumed by
 * [OtaLibAppHook] via Flutter shell args on the next cold start after promote.
 */
class OtaGate(
    context: Context,
    private val config: OtaConfig = OtaConfig.from(context),
    private val store: OtaStore = OtaStore(context),
) {
    private val appContext = context.applicationContext

    @Volatile
    var ffiLoadedFromOta: Boolean = false
        private set

    @Volatile
    var bootWithOta: Boolean = false
        private set

    /**
     * Call from [android.app.Application.onCreate] before Flutter starts.
     * Returns whether OTA libs should be used this boot.
     */
    fun bootstrap(): Boolean {
        if (store.crashPending) {
            Log.w(TAG, "OTA crash marker set — clearing current, using APK builtins")
            store.clearCurrent()
            return false
        }
        if (!store.otaActive || !store.hasCurrentPackage()) {
            store.otaActive = false
            return false
        }
        // Host identity is fixed at APK build time; clear if abi policy ever expands.
        val ffi = store.currentLibFfi()
        return try {
            System.load(ffi.absolutePath)
            ffiLoadedFromOta = true
            bootWithOta = true
            store.crashPending = true
            Log.i(TAG, "Loaded OTA FFI from ${ffi.absolutePath} logic=${store.logicVersion}")
            true
        } catch (t: Throwable) {
            Log.e(TAG, "System.load OTA FFI failed — clearing", t)
            store.clearCurrent()
            false
        }
    }

    /** Absolute path for `--aot-shared-library-name=` when booting with OTA. */
    fun libAppShellArgPath(): String? {
        if (!bootWithOta) return null
        val f = store.currentLibApp()
        return if (f.isFile) f.canonicalPath else null
    }

    fun status(): OtaStatus =
        OtaStatus(
            active = bootWithOta && store.otaActive,
            logicVersion = store.logicVersion,
            ffiLoadedFromOta = ffiLoadedFromOta,
            libAppPath = if (bootWithOta) store.currentLibApp().absolutePath else null,
            ffiPath = if (ffiLoadedFromOta) store.currentLibFfi().absolutePath else null,
            hostLine = config.hostLine,
            engineBuildId = config.engineBuildId,
        )

    /** Clear crash marker after first healthy Dart frame. */
    fun markHealthy() {
        if (store.crashPending) {
            store.crashPending = false
            Log.i(TAG, "OTA marked healthy")
        }
    }

    /** Soft-fail background check. Safe to call multiple times. */
    fun checkInBackground() {
        thread(name = "kim-ota-check", isDaemon = true) {
            try {
                checkAndMaybeInstall()
            } catch (t: Throwable) {
                Log.w(TAG, "OTA check failed (ignored)", t)
            }
        }
    }

    fun checkAndMaybeInstall() {
        val url = config.checkUrl(store.logicVersion)
        Log.i(TAG, "OTA check $url")
        val body = httpGetString(url) ?: return
        val offer = OtaCheckResponse.parse(body).update ?: run {
            Log.i(TAG, "OTA: no update")
            return
        }
        if (!offerCompatible(offer)) {
            Log.w(TAG, "OTA offer rejected by host policy")
            return
        }
        if (offer.logicVersion == store.logicVersion && store.otaActive) {
            Log.i(TAG, "OTA already at ${offer.logicVersion}")
            return
        }
        downloadVerifyPromote(offer)
    }


    private fun offerCompatible(offer: OtaUpdateOffer): Boolean {
        if (offer.hostLine != config.hostLine) return false
        if (offer.engineBuildId != config.engineBuildId) return false
        if (offer.abi != "arm64-v8a") return false
        if (offer.channel != config.channel) return false
        val vc = config.hostVersionCode
        if (vc < offer.minHostVersionCode || vc > offer.maxHostVersionCode) return false
        return true
    }

    private fun downloadVerifyPromote(offer: OtaUpdateOffer) {
        store.clearStaging()
        val zipFile = File(store.staging, "pkg.zip")
        val manifestFile = File(store.staging, "manifest.json")
        val sigFile = File(store.staging, "manifest.json.sig")
        httpGetToFile(offer.zipUrl, zipFile)
        httpGetToFile(offer.manifestUrl, manifestFile)
        httpGetToFile(offer.signatureUrl, sigFile)

        val manifestBytes = manifestFile.readBytes()
        val sigBytes = sigFile.readBytes()
        if (!OtaCrypto.verifyEd25519(config.publicKeyRaw, manifestBytes, sigBytes)) {
            throw SecurityException("manifest Ed25519 signature invalid")
        }
        val manifest = OtaManifest.parse(String(manifestBytes, Charsets.UTF_8))
        if (manifest.schemaVersion != 1) {
            throw IllegalStateException("unsupported schema_version")
        }
        if (manifest.logicVersion != offer.logicVersion) {
            throw IllegalStateException("logic_version mismatch offer vs manifest")
        }
        if (manifest.hostLine != config.hostLine ||
            manifest.engineBuildId != config.engineBuildId ||
            manifest.abi != "arm64-v8a" ||
            manifest.channel != config.channel
        ) {
            throw IllegalStateException("manifest host mapping mismatch")
        }
        val vc = config.hostVersionCode
        if (vc < manifest.minHostVersionCode || vc > manifest.maxHostVersionCode) {
            throw IllegalStateException("host versionCode outside manifest window")
        }
        val zipBytes = zipFile.readBytes()
        val zipSha = OtaCrypto.sha256Hex(zipBytes)
        if (zipSha != manifest.zipSha256 || zipSha != offer.zipSha256) {
            throw SecurityException("zip sha256 mismatch")
        }
        unzipAllowlisted(zipFile, store.staging)
        for (name in OtaAllowed.NAMES) {
            val art = manifest.artifact(name)
                ?: throw IllegalStateException("manifest missing $name")
            val f = File(store.staging, name)
            if (!f.isFile) throw IllegalStateException("missing unzipped $name")
            val data = f.readBytes()
            if (data.size.toLong() != art.size) {
                throw SecurityException("$name size mismatch")
            }
            if (OtaCrypto.sha256Hex(data) != art.sha256) {
                throw SecurityException("$name sha256 mismatch")
            }
        }
        // Remove non-SO staging junk before promote.
        zipFile.delete()
        manifestFile.delete()
        sigFile.delete()
        store.promoteStagingToCurrent(manifest.logicVersion)
        Log.i(TAG, "OTA staged for next cold start: ${manifest.logicVersion}")
    }

    private fun unzipAllowlisted(zip: File, dest: File) {
        ZipInputStream(BufferedInputStream(FileInputStream(zip))).use { zis ->
            while (true) {
                val entry = zis.nextEntry ?: break
                try {
                    if (entry.isDirectory) {
                        continue
                    }
                    // Flat allowlist only: reject nested paths / traversal.
                    val raw = entry.name.removePrefix("./")
                    if (raw.contains('/') || raw.contains('\\') || raw.contains("..")) {
                        throw SecurityException("zip path not allowlisted: ${entry.name}")
                    }
                    if (raw !in OtaAllowed.NAMES) {
                        throw SecurityException("unexpected zip member: $raw")
                    }
                    val out = File(dest, raw)
                    FileOutputStream(out).use { fos -> zis.copyTo(fos) }
                } finally {
                    zis.closeEntry()
                }
            }
        }
        for (name in OtaAllowed.NAMES) {
            if (!File(dest, name).isFile) {
                throw IllegalStateException("zip missing $name")
            }
        }
    }

    private fun httpGetString(url: String): String? {
        val conn = (URL(url).openConnection() as HttpURLConnection).apply {
            connectTimeout = 15_000
            readTimeout = 30_000
            instanceFollowRedirects = true
            requestMethod = "GET"
        }
        return try {
            val code = conn.responseCode
            if (code !in 200..299) {
                Log.w(TAG, "OTA check HTTP $code")
                null
            } else {
                conn.inputStream.bufferedReader().use { it.readText() }
            }
        } finally {
            conn.disconnect()
        }
    }

    private fun httpGetToFile(url: String, dest: File) {
        val conn = (URL(url).openConnection() as HttpURLConnection).apply {
            connectTimeout = 15_000
            readTimeout = 120_000
            instanceFollowRedirects = true
            requestMethod = "GET"
        }
        try {
            val code = conn.responseCode
            if (code !in 200..299) {
                throw IllegalStateException("download HTTP $code for $url")
            }
            conn.inputStream.use { inp ->
                FileOutputStream(dest).use { out -> inp.copyTo(out) }
            }
        } finally {
            conn.disconnect()
        }
    }

    companion object {
        private const val TAG = "KimOtaGate"

        @Volatile
        private var instance: OtaGate? = null

        fun get(context: Context): OtaGate {
            instance?.let { return it }
            synchronized(this) {
                instance?.let { return it }
                val g = OtaGate(context.applicationContext)
                instance = g
                return g
            }
        }
    }
}

