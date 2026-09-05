package com.kim.kim_mobile.ota

import org.json.JSONObject

/** Signed logic-OTA manifest (schema_version = 1). Patch-only dynamic semver. */
data class OtaManifest(
    val schemaVersion: Int,
    val hostLine: String,
    val engineBuildId: String,
    val logicVersion: String,
    val abi: String,
    val channel: String,
    val zipSha256: String,
    val artifacts: List<OtaArtifact>,
    val createdAt: String? = null,
    val notes: String? = null,
) {
    fun artifact(name: String): OtaArtifact? = artifacts.firstOrNull { it.name == name }

    companion object {
        fun parse(json: String): OtaManifest {
            val o = JSONObject(json)
            val arts = o.getJSONArray("artifacts")
            val list = ArrayList<OtaArtifact>(arts.length())
            for (i in 0 until arts.length()) {
                list.add(OtaArtifact.parse(arts.getJSONObject(i)))
            }
            return OtaManifest(
                schemaVersion = o.getInt("schema_version"),
                hostLine = o.getString("host_line"),
                engineBuildId = o.getString("engine_build_id"),
                logicVersion = o.getString("logic_version"),
                abi = o.getString("abi"),
                channel = o.getString("channel"),
                zipSha256 = o.getString("zip_sha256"),
                artifacts = list,
                createdAt = if (o.has("created_at") && !o.isNull("created_at")) o.getString("created_at") else null,
                notes = if (o.has("notes") && !o.isNull("notes")) o.getString("notes") else null,
            )
        }
    }
}

data class OtaArtifact(
    val name: String,
    val sha256: String,
    val size: Long,
) {
    companion object {
        fun parse(o: JSONObject): OtaArtifact =
            OtaArtifact(
                name = o.getString("name"),
                sha256 = o.getString("sha256"),
                size = o.getLong("size"),
            )
    }
}

/** Snapshot exposed to Dart via MethodChannel. */
data class OtaStatus(
    val active: Boolean,
    val logicVersion: String?,
    val ffiLoadedFromOta: Boolean,
    val libAppPath: String?,
    val ffiPath: String?,
    val hostLine: String,
    val engineBuildId: String,
) {
    fun toMap(): Map<String, Any?> =
        mapOf(
            "active" to active,
            "logicVersion" to logicVersion,
            "ffiLoadedFromOta" to ffiLoadedFromOta,
            "libAppPath" to libAppPath,
            "ffiPath" to ffiPath,
            "hostLine" to hostLine,
            "engineBuildId" to engineBuildId,
        )
}

/** Allowlisted zip members only. */
object OtaAllowed {
    const val LIB_APP = "libapp.so"
    const val LIB_FFI = "libkim_client_ffi.so"
    val NAMES = setOf(LIB_APP, LIB_FFI)
}

/** Parsed `x.y.z` (no prerelease / build metadata). */
data class SemverTriple(val major: Long, val minor: Long, val patch: Long) {
    val hostLine: String get() = "$major.$minor"

    companion object {
        fun parse(raw: String): SemverTriple? {
            val core = raw.trim().substringBefore('+').substringBefore('-')
            val parts = core.split('.')
            if (parts.size != 3) return null
            val nums = parts.map { p ->
                if (p.isEmpty() || !p.all { it.isDigit() }) return null
                p.toLongOrNull() ?: return null
            }
            return SemverTriple(nums[0], nums[1], nums[2])
        }
    }
}
