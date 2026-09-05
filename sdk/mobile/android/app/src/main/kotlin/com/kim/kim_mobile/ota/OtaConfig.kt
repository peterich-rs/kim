package com.kim.kim_mobile.ota

import android.content.Context
import com.kim.kim_mobile.BuildConfig
import java.nio.charset.StandardCharsets

/**
 * Host identity + GitHub Releases catalog + embedded Ed25519 public key.
 *
 * [hostLine] is MAJOR.MINOR derived from [versionName] (pubspec → Android
 * versionName). Patch-only OTA stays on the same line; Kotlin/resource/plugin/
 * engine/Manifest changes bump minor or major via a full APK release.
 */
data class OtaConfig(
    val hostLine: String,
    val versionName: String,
    val engineBuildId: String,
    val channel: String,
    val githubOwner: String,
    val githubRepo: String,
    val tagPrefix: String,
    val publicKeyPem: String,
) {
    val publicKeyRaw: ByteArray by lazy { OtaCrypto.ed25519PublicKeyFromPem(publicKeyPem) }

    /** List releases (newest first). Catalog is GitHub Releases — no Royal check API. */
    fun releasesUrl(): String =
        "https://api.github.com/repos/$githubOwner/$githubRepo/releases?per_page=30"

    companion object {
        const val USER_AGENT = "KimMobileOTA/1.0"

        /**
         * Derive `x.y` host line from a semver-ish string (`x.y.z`, optional
         * `+build` / `-prerelease` suffix).
         */
        fun hostLineFromVersion(version: String): String {
            val core = version.trim().substringBefore('+').substringBefore('-')
            val parts = core.split('.')
            require(parts.size >= 2 && parts[0].all { it.isDigit() } && parts[1].all { it.isDigit() }) {
                "version must start with x.y, got: $version"
            }
            return "${parts[0]}.${parts[1]}"
        }

        /** Strip `+build` / keep `x.y.z` core for patch baseline compares. */
        fun semverCore(version: String): String =
            version.trim().substringBefore('+').substringBefore('-')

        fun from(context: Context): OtaConfig {
            val pem =
                try {
                    context.assets.open("ota/ed25519_public.pem").use { inp ->
                        String(inp.readBytes(), StandardCharsets.UTF_8)
                    }
                } catch (_: Exception) {
                    PLACEHOLDER_PUBLIC_PEM
                }
            val versionName = BuildConfig.VERSION_NAME
            return OtaConfig(
                hostLine = hostLineFromVersion(versionName),
                versionName = semverCore(versionName),
                engineBuildId = BuildConfig.OTA_ENGINE_BUILD_ID,
                channel = BuildConfig.OTA_CHANNEL,
                githubOwner = BuildConfig.OTA_GITHUB_OWNER,
                githubRepo = BuildConfig.OTA_GITHUB_REPO,
                tagPrefix = BuildConfig.OTA_TAG_PREFIX,
                publicKeyPem = pem,
            )
        }

        /** Fallback if asset missing; must match assets/ota/ed25519_public.pem. */
        const val PLACEHOLDER_PUBLIC_PEM =
            "-----BEGIN PUBLIC KEY-----\n" +
                "MCowBQYDK2VwAyEAU4tV2GY9rXlAHW+PpARhKqg15czMmmcnrCnD5mBfRYc=\n" +
                "-----END PUBLIC KEY-----\n"
    }
}
