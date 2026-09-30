package com.kim.kim_mobile

import android.content.Context
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

/// `kim.keystore` backend for Android: an AndroidKeyStore AES-256 key wraps
/// each value with AES/GCM/NoPadding; the ciphertext blob lives in a private
/// SharedPreferences file. Same shape as the previous flutter_secure_storage
/// 11 configuration (no biometry, no RNG seeding).
object KimKeystore {
    private const val PREFS = "kim.keystore"
    private const val KEY_ALIAS = "kim.keystore.aes"
    private const val IV_PREFIX = "iv."
    private const val VAL_PREFIX = "val."

    fun read(context: Context, key: String): String? {
        val prefs = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
        val blob = prefs.getString(VAL_PREFIX + key, null) ?: return null
        val iv = prefs.getString(IV_PREFIX + key, null) ?: return null
        return try {
            val cipher = Cipher.getInstance("AES/GCM/NoPadding")
            cipher.init(
                Cipher.DECRYPT_MODE,
                key(),
                GCMParameterSpec(128, Base64.decode(iv, Base64.NO_WRAP))
            )
            String(cipher.doFinal(Base64.decode(blob, Base64.NO_WRAP)), Charsets.UTF_8)
        } catch (_: Exception) {
            null
        }
    }

    fun write(context: Context, key: String, value: String) {
        val prefs = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
        try {
            val cipher = Cipher.getInstance("AES/GCM/NoPadding")
            cipher.init(Cipher.ENCRYPT_MODE, key())
            val blob = Base64.encodeToString(cipher.doFinal(value.toByteArray(Charsets.UTF_8)), Base64.NO_WRAP)
            val iv = Base64.encodeToString(cipher.iv, Base64.NO_WRAP)
            prefs.edit()
                .putString(VAL_PREFIX + key, blob)
                .putString(IV_PREFIX + key, iv)
                .apply()
        } catch (e: Exception) {
            throw IllegalStateException("keystore write failed: ${e.message}", e)
        }
    }

    fun delete(context: Context, key: String) {
        context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
            .edit()
            .remove(VAL_PREFIX + key)
            .remove(IV_PREFIX + key)
            .apply()
    }

    private fun key(): SecretKey {
        val ks = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
        (ks.getKey(KEY_ALIAS, null) as? SecretKey)?.let { return it }
        val generator = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, "AndroidKeyStore")
        generator.init(
            KeyGenParameterSpec.Builder(
                KEY_ALIAS,
                KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT
            )
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .setKeySize(256)
                .build()
        )
        return generator.generateKey()
    }
}
