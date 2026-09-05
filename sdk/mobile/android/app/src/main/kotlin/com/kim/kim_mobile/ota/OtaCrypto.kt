package com.kim.kim_mobile.ota

import org.bouncycastle.crypto.params.Ed25519PublicKeyParameters
import org.bouncycastle.crypto.signers.Ed25519Signer
import org.bouncycastle.util.io.pem.PemReader
import java.io.StringReader
import java.security.MessageDigest

/**
 * SHA-256 + Ed25519 verify for logic-OTA manifests.
 *
 * Uses BouncyCastle for Ed25519 only (Android KeyFactory Ed25519 is API 33+ and
 * inconsistent with OpenSSL raw signatures). Pin bcprov in build.gradle.kts.
 */
object OtaCrypto {
    fun sha256Hex(data: ByteArray): String {
        val digest = MessageDigest.getInstance("SHA-256").digest(data)
        return digest.joinToString("") { b -> "%02x".format(b) }
    }

    /**
     * Parse SubjectPublicKeyInfo PEM (`-----BEGIN PUBLIC KEY-----`) produced by
     * `openssl pkey -pubout` for Ed25519. Extracts the raw 32-byte public key.
     */
    fun ed25519PublicKeyFromPem(pem: String): ByteArray {
        PemReader(StringReader(pem)).use { reader ->
            val obj = reader.readPemObject()
                ?: throw IllegalArgumentException("empty PEM")
            val der = obj.content
            // SPKI for Ed25519: 12-byte header + 32-byte key (RFC 8410).
            if (der.size < 32) {
                throw IllegalArgumentException("PEM too short for Ed25519")
            }
            return der.copyOfRange(der.size - 32, der.size)
        }
    }

    /** Verify OpenSSL `pkeyutl -sign -rawin` Ed25519 signature (64 bytes). */
    fun verifyEd25519(
        publicKeyRaw32: ByteArray,
        message: ByteArray,
        signature: ByteArray,
    ): Boolean {
        if (publicKeyRaw32.size != 32 || signature.size != 64) {
            return false
        }
        val params = Ed25519PublicKeyParameters(publicKeyRaw32, 0)
        val signer = Ed25519Signer()
        signer.init(false, params)
        signer.update(message, 0, message.size)
        return signer.verifySignature(signature)
    }
}
