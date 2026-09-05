package com.kim.kim_mobile.ota

import android.content.Context
import android.util.Log
import org.json.JSONArray
import java.io.BufferedInputStream
import java.io.File
import java.io.FileInputStream
import java.io.FileOutputStream
import java.net.HttpURLConnection
import java.net.URL
import java.util.zip.ZipInputStream
import kotlin.concurrent.thread

/**
 * Logic SO OTA gate: crash-loop protection, early FFI [System.load], GitHub
 * Releases catalog / download / verify / atomic install. Libapp path is
 * consumed by [OtaLibAppHook] via Flutter shell args on the next cold start
 * after promote.
 */
class OtaGate(
    context: Context,
    private val config: OtaConfig = OtaConfig.from(context),
    private val store: OtaStore = OtaStore(context),
) {

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

    /**
     * Catalog updates via GitHub Releases API (newest first). Soft-fails on
     * network / 403 / rate-limit. Installs the first compatible newer offer.
     */
    fun checkAndMaybeInstall() {
        val url = config.releasesUrl()
        Log.i(TAG, "OTA catalog $url")
        val body = httpGetString(url, githubApi = true) ?: return
        val releases =
            try {
                JSONArray(body)
            } catch (e: Exception) {
                Log.w(TAG, "OTA: invalid releases JSON", e)
                return
            }
        val installed = store.logicVersion
        for (i in 0 until releases.length()) {
            val rel = releases.getJSONObject(i)
            if (rel.optBoolean("draft", false)) continue
            val tag = rel.optString("tag_name", "")
            if (!tag.startsWith(config.tagPrefix)) continue
            val assets = assetUrlByName(rel.optJSONArray("assets") ?: JSONArray())
            val manifestUrl = assets["manifest.json"] ?: continue
            val sigUrl = assets["manifest.json.sig"] ?: continue
            val zipUrl = selectZipUrl(assets) ?: continue

            store.clearStaging()
            val manifestFile = File(store.staging, "manifest.json")
            val sigFile = File(store.staging, "manifest.json.sig")
            try {
                httpGetToFile(manifestUrl, manifestFile, githubApi = true)
                httpGetToFile(sigUrl, sigFile, githubApi = true)
            } catch (t: Throwable) {
                Log.w(TAG, "OTA: skip release $tag (manifest/sig download)", t)
                continue
            }
            val manifestBytes = manifestFile.readBytes()
            val sigBytes = sigFile.readBytes()
            if (!OtaCrypto.verifyEd25519(config.publicKeyRaw, manifestBytes, sigBytes)) {
                Log.w(TAG, "OTA: skip $tag — Ed25519 signature invalid")
                continue
            }
            val manifest =
                try {
                    OtaManifest.parse(String(manifestBytes, Charsets.UTF_8))
                } catch (t: Throwable) {
                    Log.w(TAG, "OTA: skip $tag — bad manifest", t)
                    continue
                }
            if (!manifestCompatible(manifest)) {
                Log.i(TAG, "OTA: skip $tag — host policy (logic=${manifest.logicVersion})")
                continue
            }
            if (!isNewerLogic(manifest.logicVersion, installed)) {
                Log.i(
                    TAG,
                    "OTA: skip $tag — not newer than installed=${installed ?: "null"} " +
                        "logic=${manifest.logicVersion}",
                )
                continue
            }
            Log.i(TAG, "OTA: installing $tag logic=${manifest.logicVersion}")
            downloadVerifyPromote(zipUrl, manifestFile, sigFile, manifest)
            return
        }
        Log.i(TAG, "OTA: no compatible newer release")
    }

    private fun assetUrlByName(assets: JSONArray): Map<String, String> {
        val map = LinkedHashMap<String, String>()
        for (i in 0 until assets.length()) {
            val a = assets.getJSONObject(i)
            val name = a.optString("name", "")
            val url = a.optString("browser_download_url", "")
            if (name.isNotEmpty() && url.isNotEmpty()) {
                map[name] = url
            }
        }
        return map
    }

    /** Prefer `logic-ota-*-arm64-v8a.zip`; else a single allowlisted `.zip`. */
    private fun selectZipUrl(assets: Map<String, String>): String? {
        val preferred =
            assets.entries.firstOrNull { (name, _) ->
                name.startsWith("logic-ota-") && name.endsWith("-arm64-v8a.zip")
            }
        if (preferred != null) return preferred.value
        val zips = assets.filterKeys { it.endsWith(".zip", ignoreCase = true) }
        return if (zips.size == 1) zips.values.first() else null
    }

    private fun manifestCompatible(m: OtaManifest): Boolean {
        if (m.schemaVersion != 1) return false
        if (m.hostLine != config.hostLine) return false
        if (m.engineBuildId != config.engineBuildId) return false
        if (m.abi != "arm64-v8a") return false
        if (m.channel != config.channel) return false
        val vc = config.hostVersionCode
        if (vc < m.minHostVersionCode || vc > m.maxHostVersionCode) return false
        return true
    }

    /**
     * Prefer dotted-numeric compare (`42`, `1.0.42`); else lexicographic after trim.
     * Equal → not newer; installed null/empty → accept.
     */
    internal fun isNewerLogic(candidate: String, installed: String?): Boolean {
        if (installed.isNullOrEmpty()) return true
        if (candidate == installed) return false
        val cp = parseDottedNumeric(candidate)
        val ip = parseDottedNumeric(installed)
        if (cp != null && ip != null) {
            val len = maxOf(cp.size, ip.size)
            for (i in 0 until len) {
                val a = cp.getOrElse(i) { 0L }
                val b = ip.getOrElse(i) { 0L }
                if (a != b) return a > b
            }
            return false
        }
        return candidate.trim() > installed.trim()
    }

    private fun parseDottedNumeric(v: String): List<Long>? {
        val parts = v.trim().split('.')
        if (parts.isEmpty()) return null
        val out = ArrayList<Long>(parts.size)
        for (p in parts) {
            if (p.isEmpty() || !p.all { it.isDigit() }) return null
            out.add(p.toLongOrNull() ?: return null)
        }
        return out
    }

    private fun downloadVerifyPromote(
        zipUrl: String,
        manifestFile: File,
        sigFile: File,
        manifest: OtaManifest,
    ) {
        val zipFile = File(store.staging, "pkg.zip")
        httpGetToFile(zipUrl, zipFile, githubApi = true)

        // Re-verify sig (already checked) and hashes against this zip.
        val manifestBytes = manifestFile.readBytes()
        val sigBytes = sigFile.readBytes()
        if (!OtaCrypto.verifyEd25519(config.publicKeyRaw, manifestBytes, sigBytes)) {
            throw SecurityException("manifest Ed25519 signature invalid")
        }
        if (manifest.schemaVersion != 1) {
            throw IllegalStateException("unsupported schema_version")
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
        if (zipSha != manifest.zipSha256) {
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

    private fun httpGetString(url: String, githubApi: Boolean = false): String? {
        val conn = openGet(url, githubApi, readTimeoutMs = 30_000)
        return try {
            val code = conn.responseCode
            if (code == 403 || code == 429) {
                Log.w(TAG, "OTA GitHub forbidden/rate-limit HTTP $code")
                return null
            }
            if (code !in 200..299) {
                Log.w(TAG, "OTA check HTTP $code")
                null
            } else {
                conn.inputStream.bufferedReader().use { it.readText() }
            }
        } catch (t: Throwable) {
            Log.w(TAG, "OTA network error (ignored)", t)
            null
        } finally {
            conn.disconnect()
        }
    }

    private fun httpGetToFile(url: String, dest: File, githubApi: Boolean = false) {
        val conn = openGet(url, githubApi, readTimeoutMs = 120_000)
        try {
            val code = conn.responseCode
            if (code == 403 || code == 429) {
                throw IllegalStateException("download forbidden/rate-limit HTTP $code for $url")
            }
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

    private fun openGet(url: String, githubApi: Boolean, readTimeoutMs: Int): HttpURLConnection {
        val conn = (URL(url).openConnection() as HttpURLConnection).apply {
            connectTimeout = 15_000
            readTimeout = readTimeoutMs
            instanceFollowRedirects = true
            requestMethod = "GET"
            setRequestProperty("User-Agent", OtaConfig.USER_AGENT)
            if (githubApi) {
                setRequestProperty("Accept", "application/vnd.github+json")
            }
        }
        return conn
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
