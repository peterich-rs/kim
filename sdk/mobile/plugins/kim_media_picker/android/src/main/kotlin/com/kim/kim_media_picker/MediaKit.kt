package com.kim.kim_media_picker

import android.Manifest
import android.content.ContentUris
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.Matrix
import android.media.MediaMetadataRetriever
import android.net.Uri
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.provider.MediaStore
import android.provider.Settings
import android.util.LruCache
import android.widget.ImageView
import android.widget.Toast
import androidx.appcompat.app.AlertDialog
import androidx.core.content.ContextCompat
import androidx.exifinterface.media.ExifInterface
import java.io.File
import java.io.FileOutputStream
import java.util.UUID
import java.util.concurrent.Executors

data class MediaItem(
    val id: String,
    val uri: Uri,
    val bucketId: String,
    val bucketName: String,
    val mimeType: String,
    val width: Int,
    val height: Int,
    val size: Long,
)

data class AlbumBucket(
    val id: String,
    val name: String,
    val count: Int,
    val cover: MediaItem?,
)

object PickerSession {
    const val extraAssets = "assets"
    const val extraMaxCount = "maxCount"
    const val extraIndex = "index"
    const val extraCaptureMode = "captureMode"

    var maxCount: Int = 9
    var captureMode: String = "mixed"
    var items: List<MediaItem> = emptyList()
    var preview: List<MediaItem> = emptyList()
    var albums: List<AlbumBucket> = emptyList()
    val selected = LinkedHashMap<String, MediaItem>()
    var resultAssets: ArrayList<HashMap<String, Any>>? = null

    fun reset(maxCount: Int, captureMode: String = "mixed") {
        this.maxCount = maxCount.coerceAtLeast(1)
        this.captureMode = captureMode
        items = emptyList()
        preview = emptyList()
        albums = emptyList()
        selected.clear()
        resultAssets = null
    }

    val single: Boolean get() = maxCount == 1

    fun selectedIndex(id: String): Int {
        val i = selected.keys.indexOf(id)
        return if (i < 0) 0 else i + 1
    }

    fun toggle(item: MediaItem, context: Context): Boolean {
        if (selected.containsKey(item.id)) {
            selected.remove(item.id)
            return true
        }
        if (selected.size >= maxCount) {
            if (maxCount == 1) {
                selected.clear()
                selected[item.id] = item
                return true
            }
            Toast.makeText(context, "最多只能选择${maxCount}张照片", Toast.LENGTH_SHORT).show()
            return false
        }
        selected[item.id] = item
        return true
    }
}

object MediaStoreLoader {
    fun load(context: Context): Pair<List<MediaItem>, List<AlbumBucket>> {
        val items = ArrayList<MediaItem>()
        val projection =
            arrayOf(
                MediaStore.Images.Media._ID,
                MediaStore.Images.Media.BUCKET_ID,
                MediaStore.Images.Media.BUCKET_DISPLAY_NAME,
                MediaStore.Images.Media.MIME_TYPE,
                MediaStore.Images.Media.WIDTH,
                MediaStore.Images.Media.HEIGHT,
                MediaStore.Images.Media.SIZE,
            )
        val sort = "${MediaStore.Images.Media.DATE_ADDED} DESC"
        context.contentResolver
            .query(MediaStore.Images.Media.EXTERNAL_CONTENT_URI, projection, null, null, sort)
            .use { cursor ->
                if (cursor == null) {
                    return emptyList<MediaItem>() to emptyList()
                }
                val idCol = cursor.getColumnIndexOrThrow(MediaStore.Images.Media._ID)
                val bucketIdCol = cursor.getColumnIndexOrThrow(MediaStore.Images.Media.BUCKET_ID)
                val bucketNameCol =
                    cursor.getColumnIndexOrThrow(MediaStore.Images.Media.BUCKET_DISPLAY_NAME)
                val mimeCol = cursor.getColumnIndexOrThrow(MediaStore.Images.Media.MIME_TYPE)
                val wCol = cursor.getColumnIndexOrThrow(MediaStore.Images.Media.WIDTH)
                val hCol = cursor.getColumnIndexOrThrow(MediaStore.Images.Media.HEIGHT)
                val sizeCol = cursor.getColumnIndexOrThrow(MediaStore.Images.Media.SIZE)
                while (cursor.moveToNext()) {
                    val id = cursor.getLong(idCol)
                    val uri =
                        ContentUris.withAppendedId(MediaStore.Images.Media.EXTERNAL_CONTENT_URI, id)
                    items.add(
                        MediaItem(
                            id = uri.toString(),
                            uri = uri,
                            bucketId = cursor.getString(bucketIdCol) ?: "",
                            bucketName = cursor.getString(bucketNameCol) ?: "相册",
                            mimeType = cursor.getString(mimeCol) ?: "image/jpeg",
                            width = cursor.getInt(wCol),
                            height = cursor.getInt(hCol),
                            size = cursor.getLong(sizeCol),
                        ),
                    )
                }
            }
        val groups = LinkedHashMap<String, ArrayList<MediaItem>>()
        for (item in items) {
            groups.getOrPut(item.bucketId) { ArrayList() }.add(item)
        }
        val albums = ArrayList<AlbumBucket>()
        albums.add(AlbumBucket("all", "所有照片", items.size, items.firstOrNull()))
        for (entry in groups.entries) {
            val cover = entry.value.firstOrNull()
            albums.add(
                AlbumBucket(
                    entry.key,
                    cover?.bucketName?.ifEmpty { "相册" } ?: "相册",
                    entry.value.size,
                    cover,
                ),
            )
        }
        return items to albums
    }
}

object MediaExport {
    private const val maxEdge = 2560

    fun export(context: Context, items: Collection<MediaItem>): ArrayList<HashMap<String, Any>> {
        val out = ArrayList<HashMap<String, Any>>(items.size)
        for (item in items) {
            out.add(exportUri(context, item.uri, item.id, item.mimeType))
        }
        return out
    }

    fun exportUri(
        context: Context,
        uri: Uri,
        id: String,
        mimeType: String = "image/jpeg",
    ): HashMap<String, Any> {
        val bytes =
            context.contentResolver.openInputStream(uri).use { input ->
                require(input != null) { "open $uri" }
                input.readBytes()
            }
        return writeImage(context, bytes, id, mimeType)
    }

    fun exportFile(context: Context, src: File): HashMap<String, Any> {
        return writeImage(context, src.readBytes(), src.absolutePath, "image/jpeg")
    }

    private fun writeImage(
        context: Context,
        bytes: ByteArray,
        id: String,
        declaredType: String,
    ): HashMap<String, Any> {
        val mime = normalizeMime(declaredType, bytes)
        val dir = File(context.cacheDir, "kim_media_picker").apply { mkdirs() }
        val ext =
            when (mime) {
                "image/png" -> "png"
                "image/gif" -> "gif"
                "image/webp" -> "webp"
                else -> "jpg"
            }
        val file = File(dir, "${UUID.randomUUID()}.$ext")
        val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
        BitmapFactory.decodeByteArray(bytes, 0, bytes.size, bounds)
        val longest = maxOf(bounds.outWidth, bounds.outHeight).coerceAtLeast(1)
        if (longest <= maxEdge && mime != "image/jpeg") {
            file.writeBytes(bytes)
            return hashMapOf(
                "id" to id,
                "path" to file.absolutePath,
                "width" to bounds.outWidth,
                "height" to bounds.outHeight,
                "size" to file.length(),
                "mimeType" to mime,
                "durationMs" to 0,
            )
        }
        val bitmap = decodeAndDownsample(bytes)
        val format =
            when (mime) {
                "image/png" -> Bitmap.CompressFormat.PNG
                "image/webp" -> Bitmap.CompressFormat.WEBP
                else -> Bitmap.CompressFormat.JPEG
            }
        FileOutputStream(file).use { output ->
            bitmap.compress(format, if (format == Bitmap.CompressFormat.PNG) 100 else 92, output)
        }
        val outMime = if (format == Bitmap.CompressFormat.JPEG) "image/jpeg" else mime
        val outExt = if (outMime == "image/jpeg") "jpg" else ext
        val outFile =
            if (file.extension == outExt) {
                file
            } else {
                File(dir, "${UUID.randomUUID()}.$outExt").also {
                    file.copyTo(it, overwrite = true)
                    file.delete()
                }
            }
        val outBounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
        BitmapFactory.decodeFile(outFile.absolutePath, outBounds)
        bitmap.recycle()
        return hashMapOf(
            "id" to id,
            "path" to outFile.absolutePath,
            "width" to outBounds.outWidth,
            "height" to outBounds.outHeight,
            "size" to outFile.length(),
            "mimeType" to outMime,
            "durationMs" to 0,
        )
    }

    private fun normalizeMime(declared: String, bytes: ByteArray): String {
        val raw = declared.split(";").first().trim().lowercase()
        val mapped =
            when (raw) {
                "image/jpg", "image/jpeg" -> "image/jpeg"
                "image/png" -> "image/png"
                "image/gif" -> "image/gif"
                "image/webp" -> "image/webp"
                else -> ""
            }
        if (mapped.isNotEmpty()) {
            return mapped
        }
        if (bytes.size >= 8 &&
            bytes[0] == 0x89.toByte() &&
            bytes[1] == 0x50.toByte() &&
            bytes[2] == 0x4E.toByte() &&
            bytes[3] == 0x47.toByte()
        ) {
            return "image/png"
        }
        if (bytes.size >= 3 && bytes[0] == 0xFF.toByte() && bytes[1] == 0xD8.toByte()) {
            return "image/jpeg"
        }
        if (bytes.size >= 6 &&
            bytes[0] == 0x47.toByte() &&
            bytes[1] == 0x49.toByte() &&
            bytes[2] == 0x46.toByte()
        ) {
            return "image/gif"
        }
        return "image/jpeg"
    }

    fun exportVideo(context: Context, src: File): HashMap<String, Any> {
        val dir = File(context.cacheDir, "kim_media_picker").apply { mkdirs() }
        val file = File(dir, "${UUID.randomUUID()}.mp4")
        src.copyTo(file, overwrite = true)
        var width = 0
        var height = 0
        var duration = 0
        val retriever = MediaMetadataRetriever()
        try {
            retriever.setDataSource(file.absolutePath)
            width = retriever.extractMetadata(MediaMetadataRetriever.METADATA_KEY_VIDEO_WIDTH)?.toIntOrNull() ?: 0
            height = retriever.extractMetadata(MediaMetadataRetriever.METADATA_KEY_VIDEO_HEIGHT)?.toIntOrNull() ?: 0
            duration = retriever.extractMetadata(MediaMetadataRetriever.METADATA_KEY_DURATION)?.toIntOrNull() ?: 0
        } catch (_: Exception) {
        } finally {
            retriever.release()
        }
        return hashMapOf(
            "id" to src.absolutePath,
            "path" to file.absolutePath,
            "width" to width,
            "height" to height,
            "size" to file.length(),
            "mimeType" to "video/mp4",
            "durationMs" to duration,
        )
    }

    private fun decodeAndDownsample(bytes: ByteArray): Bitmap {
        val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
        BitmapFactory.decodeByteArray(bytes, 0, bytes.size, bounds)
        val longest = maxOf(bounds.outWidth, bounds.outHeight).coerceAtLeast(1)
        val sample = (longest / maxEdge).coerceAtLeast(1)
        val opts = BitmapFactory.Options().apply { inSampleSize = sample }
        val bitmap =
            BitmapFactory.decodeByteArray(bytes, 0, bytes.size, opts)
                ?: Bitmap.createBitmap(1, 1, Bitmap.Config.ARGB_8888)
        val rotation = exifRotation(bytes)
        if (rotation == 0) {
            return bitmap
        }
        val matrix = Matrix().apply { postRotate(rotation.toFloat()) }
        val out = Bitmap.createBitmap(bitmap, 0, 0, bitmap.width, bitmap.height, matrix, true)
        if (out != bitmap) {
            bitmap.recycle()
        }
        return out
    }

    private fun exifRotation(bytes: ByteArray): Int {
        return try {
            val exif = ExifInterface(bytes.inputStream())
            when (exif.getAttributeInt(ExifInterface.TAG_ORIENTATION, ExifInterface.ORIENTATION_NORMAL)) {
                ExifInterface.ORIENTATION_ROTATE_90 -> 90
                ExifInterface.ORIENTATION_ROTATE_180 -> 180
                ExifInterface.ORIENTATION_ROTATE_270 -> 270
                else -> 0
            }
        } catch (_: Exception) {
            0
        }
    }
}

class ThumbLoader(context: Context) {
    private val resolver = context.applicationContext.contentResolver
    private val cache = LruCache<String, Bitmap>(64)
    private val exec = Executors.newFixedThreadPool(4)
    private val main = Handler(Looper.getMainLooper())

    fun load(uri: Uri, view: ImageView, size: Int) {
        val key = "$uri@$size"
        val hit = cache.get(key)
        if (hit != null) {
            view.setImageBitmap(hit)
            return
        }
        view.setImageResource(R.color.kim_picker_thumb)
        view.tag = key
        exec.execute {
            val bmp = decode(uri, size) ?: return@execute
            cache.put(key, bmp)
            main.post {
                if (view.tag == key) {
                    view.setImageBitmap(bmp)
                }
            }
        }
    }

    private fun decode(uri: Uri, size: Int): Bitmap? {
        return try {
            resolver.openInputStream(uri).use { input ->
                if (input == null) {
                    return null
                }
                val bytes = input.readBytes()
                val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
                BitmapFactory.decodeByteArray(bytes, 0, bytes.size, bounds)
                val longest = maxOf(bounds.outWidth, bounds.outHeight).coerceAtLeast(1)
                val opts = BitmapFactory.Options().apply {
                    inSampleSize = (longest / size).coerceAtLeast(1)
                }
                BitmapFactory.decodeByteArray(bytes, 0, bytes.size, opts)
            }
        } catch (_: Exception) {
            null
        }
    }
}

object PickerPerms {
    fun imagePermissions(): Array<String> {
        return if (Build.VERSION.SDK_INT >= 33) {
            arrayOf(
                Manifest.permission.READ_MEDIA_IMAGES,
                Manifest.permission.READ_MEDIA_VISUAL_USER_SELECTED,
            )
        } else {
            arrayOf(Manifest.permission.READ_EXTERNAL_STORAGE)
        }
    }

    fun hasImages(context: Context): Boolean {
        return imagePermissions().any {
            ContextCompat.checkSelfPermission(context, it) == PackageManager.PERMISSION_GRANTED
        }
    }

    fun hasCamera(context: Context): Boolean {
        return ContextCompat.checkSelfPermission(context, Manifest.permission.CAMERA) ==
            PackageManager.PERMISSION_GRANTED
    }

    fun hasMic(context: Context): Boolean {
        return ContextCompat.checkSelfPermission(context, Manifest.permission.RECORD_AUDIO) ==
            PackageManager.PERMISSION_GRANTED
    }

    fun capturePermissions(needAudio: Boolean): Array<String> {
        return if (needAudio) {
            arrayOf(Manifest.permission.CAMERA, Manifest.permission.RECORD_AUDIO)
        } else {
            arrayOf(Manifest.permission.CAMERA)
        }
    }

    fun openSettings(context: Context) {
        context.startActivity(
            Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS).apply {
                data = Uri.fromParts("package", context.packageName, null)
            },
        )
    }

    fun deniedDialog(context: Context, message: String, onCancel: () -> Unit) {
        AlertDialog.Builder(context)
            .setMessage(message)
            .setNegativeButton("取消") { _, _ -> onCancel() }
            .setPositiveButton("去设置") { _, _ ->
                openSettings(context)
                onCancel()
            }
            .setCancelable(false)
            .show()
    }
}

fun Context.dp(value: Int): Int = (value * resources.displayMetrics.density).toInt()
