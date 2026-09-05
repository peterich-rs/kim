package com.kim.kim_mobile.ota

import android.content.Context
import com.kim.kim_mobile.BuildConfig
import java.nio.charset.StandardCharsets

/**
 * Host identity + GitHub Releases catalog + embedded Ed25519 public key.
 *
 * Replace `assets/ota/ed25519_public.pem` for release; bump [hostLine] when
 * platform/engine/plugins change.
 */
data class OtaConfig(
    val hostLine: String,
    val engineBuildId: String,
    val channel: String,
    val githubOwner: String,
    val githubRepo: String,
    val tagPrefix: String,
    val hostVersionCode: Int,
    val publicKeyPem: String,
) {
    val publicKeyRaw: ByteArray by lazy { OtaCrypto.ed25519PublicKeyFromPem(publicKeyPem) }

    /** List releases (newest first). Catalog is GitHub Releases — no Royal check API. */
    fun releasesUrl(): String =
        "https://api.github.com/repos/$githubOwner/$githubRepo/releases?per_page=30"

    companion object {
        const val USER_AGENT = "KimMobileOTA/1.0"

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
                githubOwner = BuildConfig.OTA_GITHUB_OWNER,
                githubRepo = BuildConfig.OTA_GITHUB_REPO,
                tagPrefix = BuildConfig.OTA_TAG_PREFIX,
                hostVersionCode = BuildConfig.VERSION_CODE,
                publicKeyPem = pem,
            )
        }

        /** Fallback if asset missing; must match assets/ota/ed25519_public.pem. */
        const val PLACEHOLDER_PUBLIC_PEM =
            "-----BEGIN PUBLIC KEY-----
" +
            "MCowBQYDK2VwAyEAU4tV2GY9rXlAHW+PpARhKqg15czMmmcnrCnD5mBfRYc=
" +
            "-----END PUBLIC KEY-----
"
 +
                "MCowBQYDK2VwAyEAL67ZGkJiDtNpJPXKnzIqtxLmB70lpqoH7oAyEQV/4rE=\n" +
                "-----END PUBLIC KEY-----\n"
    }
}
