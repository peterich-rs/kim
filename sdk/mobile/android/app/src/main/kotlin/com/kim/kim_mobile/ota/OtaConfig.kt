package com.kim.kim_mobile.ota

import android.content.Context
import com.kim.kim_mobile.BuildConfig
import java.nio.charset.StandardCharsets

/**
 * Host identity + check URL + embedded Ed25519 public key.
 *
 * Replace `assets/ota/ed25519_public.pem` for release; bump [hostLine] when
 * platform/engine/plugins change.
 */
data class OtaConfig(
    val hostLine: String,
    val engineBuildId: String,
    val channel: String,
    val checkBaseUrl: String,
    val hostVersionCode: Int,
    val publicKeyPem: String,
) {
    val publicKeyRaw: ByteArray by lazy { OtaCrypto.ed25519PublicKeyFromPem(publicKeyPem) }

    fun checkUrl(currentLogicVersion: String?): String {
        val logic = currentLogicVersion ?: "0"
        val base = checkBaseUrl.trimEnd('/')
        return "$base/v1/logic-ota/check" +
            "?host_line=${enc(hostLine)}" +
            "&host_version_code=$hostVersionCode" +
            "&engine_build_id=${enc(engineBuildId)}" +
            "&logic_version=${enc(logic)}" +
            "&abi=arm64-v8a" +
            "&channel=${enc(channel)}"
    }

    private fun enc(s: String): String = java.net.URLEncoder.encode(s, "UTF-8")

    companion object {
        fun from(context: Context): OtaConfig {
            val pem =
                try {
                    context.assets.open("ota/ed25519_public.pem").use { inp ->
                        String(inp.readBytes(), StandardCharsets.UTF_8)
                    }
                } catch (e: Exception) {
                    PLACEHOLDER_PUBLIC_PEM
                }
            return OtaConfig(
                hostLine = BuildConfig.OTA_HOST_LINE,
                engineBuildId = BuildConfig.OTA_ENGINE_BUILD_ID,
                channel = BuildConfig.OTA_CHANNEL,
                checkBaseUrl = BuildConfig.OTA_CHECK_BASE_URL,
                hostVersionCode = BuildConfig.VERSION_CODE,
                publicKeyPem = pem,
            )
        }

        /** Same placeholder as assets; used if asset missing in tests. */
        const val PLACEHOLDER_PUBLIC_PEM =
            "-----BEGIN PUBLIC KEY-----\n" +
                "MCowBQYDK2VwAyEAL67ZGkJiDtNpJPXKnzIqtxLmB70lpqoH7oAyEQV/4rE=\n" +
                "-----END PUBLIC KEY-----\n"
    }
}
