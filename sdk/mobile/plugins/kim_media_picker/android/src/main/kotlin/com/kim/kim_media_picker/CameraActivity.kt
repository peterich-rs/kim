package com.kim.kim_media_picker

import android.Manifest
import android.app.Activity
import android.content.Intent
import android.content.pm.PackageManager
import android.graphics.Color
import android.graphics.drawable.GradientDrawable
import android.media.MediaMetadataRetriever
import android.net.Uri
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.os.SystemClock
import android.view.Gravity
import android.view.MotionEvent
import android.view.View
import android.view.ViewGroup
import android.widget.FrameLayout
import android.widget.ImageView
import android.widget.LinearLayout
import android.widget.TextView
import android.widget.Toast
import androidx.appcompat.app.AppCompatActivity
import androidx.camera.core.Camera
import androidx.camera.core.CameraSelector
import androidx.camera.core.ImageCapture
import androidx.camera.core.ImageCaptureException
import androidx.camera.core.Preview
import androidx.camera.lifecycle.ProcessCameraProvider
import androidx.camera.video.FileOutputOptions
import androidx.camera.video.Quality
import androidx.camera.video.QualitySelector
import androidx.camera.video.Recorder
import androidx.camera.video.Recording
import androidx.camera.video.VideoCapture
import androidx.camera.video.VideoRecordEvent
import androidx.camera.view.PreviewView
import androidx.core.app.ActivityCompat
import androidx.core.content.ContextCompat
import java.io.File
import java.util.concurrent.ExecutorService
import java.util.concurrent.Executors

class CameraActivity : AppCompatActivity() {
    private lateinit var previewView: PreviewView
    private lateinit var capturedView: ImageView
    private lateinit var shutterRing: View
    private lateinit var shutterInner: View
    private lateinit var closeBtn: TextView
    private lateinit var flashBtn: TextView
    private lateinit var switchBtn: TextView
    private lateinit var albumThumb: ImageView
    private lateinit var photoTab: TextView
    private lateinit var videoTab: TextView
    private lateinit var modeRow: LinearLayout
    private lateinit var timerView: TextView
    private lateinit var reviewBar: LinearLayout
    private lateinit var cameraExecutor: ExecutorService
    private var imageCapture: ImageCapture? = null
    private var videoCapture: VideoCapture<Recorder>? = null
    private var recording: Recording? = null
    private var camera: Camera? = null
    private var lensFacing = CameraSelector.LENS_FACING_BACK
    private var flashIndex = 0
    private var capturedFile: File? = null
    private var capturedIsVideo = false
    private var mode = "mixed"
    private var lane = "photo"
    private var longPressRecording = false
    private var recordStartedAt = 0L
    private val main = Handler(Looper.getMainLooper())
    private val thumbs by lazy { ThumbLoader(this) }

    private val allowsPhoto get() = mode != "video"
    private val allowsVideo get() = mode != "photo"
    private val videoLane get() = !allowsPhoto || lane == "video"

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        cameraExecutor = Executors.newSingleThreadExecutor()
        mode = intent.getStringExtra(PickerSession.extraCaptureMode)
            ?: PickerSession.captureMode
        lane = if (mode == "video") "video" else "photo"
        setContentView(buildUi())
        applyLaneChrome()
        loadAlbumThumb()
        val perms = PickerPerms.capturePermissions(allowsVideo)
        if (perms.all { ContextCompat.checkSelfPermission(this, it) == PackageManager.PERMISSION_GRANTED } ||
            PickerPerms.hasCamera(this)
        ) {
            startCamera()
        } else {
            ActivityCompat.requestPermissions(this, perms, REQ_PERM)
        }
    }

    override fun onDestroy() {
        recording?.stop()
        recording = null
        cameraExecutor.shutdown()
        super.onDestroy()
    }

    override fun onRequestPermissionsResult(
        requestCode: Int,
        permissions: Array<out String>,
        grantResults: IntArray,
    ) {
        super.onRequestPermissionsResult(requestCode, permissions, grantResults)
        if (requestCode != REQ_PERM) {
            return
        }
        if (PickerPerms.hasCamera(this)) {
            startCamera()
        } else {
            PickerPerms.deniedDialog(this, "请在设置中开启相机权限") { finishCancelled() }
        }
    }

    private fun startCamera() {
        val future = ProcessCameraProvider.getInstance(this)
        future.addListener(
            {
                bindCamera(future.get())
            },
            ContextCompat.getMainExecutor(this),
        )
    }

    private fun bindCamera(provider: ProcessCameraProvider) {
        val preview = Preview.Builder().build().also {
            it.surfaceProvider = previewView.surfaceProvider
        }
        imageCapture =
            ImageCapture.Builder()
                .setCaptureMode(ImageCapture.CAPTURE_MODE_MINIMIZE_LATENCY)
                .setFlashMode(flashMode())
                .build()
        val recorder =
            Recorder.Builder()
                .setQualitySelector(QualitySelector.from(Quality.HD))
                .build()
        videoCapture = VideoCapture.withOutput(recorder)
        val selector = CameraSelector.Builder().requireLensFacing(lensFacing).build()
        provider.unbindAll()
        camera =
            try {
                when {
                    allowsPhoto && allowsVideo ->
                        provider.bindToLifecycle(this, selector, preview, imageCapture, videoCapture)
                    allowsVideo ->
                        provider.bindToLifecycle(this, selector, preview, videoCapture)
                    else ->
                        provider.bindToLifecycle(this, selector, preview, imageCapture)
                }
            } catch (_: Exception) {
                provider.bindToLifecycle(this, selector, preview, imageCapture)
            }
        applyFlash()
    }

    private fun flashMode(): Int {
        return when (flashIndex % 3) {
            1 -> ImageCapture.FLASH_MODE_ON
            2 -> ImageCapture.FLASH_MODE_AUTO
            else -> ImageCapture.FLASH_MODE_OFF
        }
    }

    private fun applyFlash() {
        imageCapture?.flashMode = flashMode()
        flashBtn.text =
            when (flashIndex % 3) {
                1 -> "闪光灯开"
                2 -> "自动"
                else -> "闪光灯关"
            }
    }

    private fun applyLaneChrome() {
        val photoOn = !videoLane
        photoTab.setTextColor(if (photoOn) Color.WHITE else 0x99FFFFFF.toInt())
        videoTab.setTextColor(if (photoOn) 0x99FFFFFF.toInt() else Color.WHITE)
        photoTab.textSize = if (photoOn) 16f else 14f
        videoTab.textSize = if (photoOn) 14f else 16f
        modeRow.visibility = if (mode == "mixed") View.VISIBLE else View.GONE
        shutterInner.background = oval(if (videoLane) 0xFFE53935.toInt() else Color.WHITE)
        if (recording == null) {
            timerView.visibility = View.GONE
        }
    }

    private fun onShutterTap() {
        if (recording != null) {
            stopRecording()
            return
        }
        if (videoLane) {
            startRecording(fromLongPress = false)
        } else {
            takePhoto()
        }
    }

    private fun takePhoto() {
        val capture = imageCapture ?: return
        val file = File(cacheDir, "kim_capture_${System.currentTimeMillis()}.jpg")
        val options = ImageCapture.OutputFileOptions.Builder(file).build()
        shutterRing.isEnabled = false
        capture.takePicture(
            options,
            cameraExecutor,
            object : ImageCapture.OnImageSavedCallback {
                override fun onImageSaved(outputFileResults: ImageCapture.OutputFileResults) {
                    runOnUiThread {
                        shutterRing.isEnabled = true
                        capturedFile = file
                        capturedIsVideo = false
                        showReview(still = file, video = false)
                    }
                }

                override fun onError(exception: ImageCaptureException) {
                    runOnUiThread { shutterRing.isEnabled = true }
                }
            },
        )
    }

    private fun startRecording(fromLongPress: Boolean) {
        if (!allowsVideo || recording != null) {
            return
        }
        if (!PickerPerms.hasMic(this) && allowsVideo) {
            ActivityCompat.requestPermissions(
                this,
                arrayOf(Manifest.permission.RECORD_AUDIO),
                REQ_MIC,
            )
            Toast.makeText(this, "录像需要麦克风权限", Toast.LENGTH_SHORT).show()
            return
        }
        val capture = videoCapture ?: return
        val file = File(cacheDir, "kim_capture_${System.currentTimeMillis()}.mp4")
        var pending = capture.output.prepareRecording(this, FileOutputOptions.Builder(file).build())
        if (PickerPerms.hasMic(this)) {
            pending = pending.withAudioEnabled()
        }
        longPressRecording = fromLongPress
        recordStartedAt = SystemClock.elapsedRealtime()
        capturedFile = file
        capturedIsVideo = true
        recording =
            pending.start(ContextCompat.getMainExecutor(this)) { event ->
                if (event is VideoRecordEvent.Finalize) {
                    recording = null
                    timerView.visibility = View.GONE
                    shutterInner.background = oval(if (videoLane) 0xFFE53935.toInt() else Color.WHITE)
                    val tooShort = SystemClock.elapsedRealtime() - recordStartedAt < 500
                    if (event.hasError() || tooShort) {
                        file.delete()
                        capturedFile = null
                        if (tooShort && !event.hasError()) {
                            Toast.makeText(this, "录像时间太短", Toast.LENGTH_SHORT).show()
                        }
                        return@start
                    }
                    showReview(still = file, video = true)
                }
            }
        timerView.visibility = View.VISIBLE
        shutterInner.background = roundRect(0xFFE53935.toInt(), dp(6))
        tickTimer()
    }

    private fun stopRecording() {
        recording?.stop()
        recording = null
    }

    private fun tickTimer() {
        if (recording == null) {
            return
        }
        val sec = ((SystemClock.elapsedRealtime() - recordStartedAt) / 1000).toInt()
        timerView.text = String.format("%02d:%02d", sec / 60, sec % 60)
        if (sec >= 60) {
            stopRecording()
            return
        }
        main.postDelayed({ tickTimer() }, 250)
    }

    private fun showReview(still: File, video: Boolean) {
        if (video) {
            val retriever = MediaMetadataRetriever()
            try {
                retriever.setDataSource(still.absolutePath)
                capturedView.setImageBitmap(retriever.frameAtTime)
            } catch (_: Exception) {
                capturedView.setImageDrawable(null)
            } finally {
                retriever.release()
            }
        } else {
            capturedView.setImageURI(Uri.fromFile(still))
        }
        capturedView.visibility = View.VISIBLE
        previewView.visibility = View.INVISIBLE
        shutterRing.visibility = View.GONE
        shutterInner.visibility = View.GONE
        flashBtn.visibility = View.GONE
        switchBtn.visibility = View.GONE
        albumThumb.visibility = View.GONE
        closeBtn.visibility = View.GONE
        modeRow.visibility = View.GONE
        timerView.visibility = View.GONE
        reviewBar.visibility = View.VISIBLE
    }

    private fun retake() {
        capturedFile?.delete()
        capturedFile = null
        capturedIsVideo = false
        capturedView.setImageDrawable(null)
        capturedView.visibility = View.GONE
        previewView.visibility = View.VISIBLE
        shutterRing.visibility = View.VISIBLE
        shutterInner.visibility = View.VISIBLE
        flashBtn.visibility = View.VISIBLE
        switchBtn.visibility = View.VISIBLE
        albumThumb.visibility = View.VISIBLE
        closeBtn.visibility = View.VISIBLE
        reviewBar.visibility = View.GONE
        applyLaneChrome()
    }

    private fun useCapture() {
        val file = capturedFile ?: return
        PickerSession.resultAssets =
            arrayListOf(
                if (capturedIsVideo) {
                    MediaExport.exportVideo(this, file)
                } else {
                    MediaExport.exportFile(this, file)
                },
            )
        file.delete()
        setResult(Activity.RESULT_OK)
        finish()
    }

    private fun finishCancelled() {
        setResult(Activity.RESULT_CANCELED)
        finish()
    }

    private fun loadAlbumThumb() {
        if (!PickerPerms.hasImages(this)) {
            return
        }
        val (items, _) = MediaStoreLoader.load(this)
        val cover = items.firstOrNull() ?: return
        thumbs.load(cover.uri, albumThumb, dp(48))
    }

    private fun openAlbum() {
        startActivityForResult(
            Intent(this, AlbumActivity::class.java)
                .putExtra(PickerSession.extraMaxCount, 1),
            REQ_ALBUM,
        )
    }

    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        super.onActivityResult(requestCode, resultCode, data)
        if (requestCode == REQ_ALBUM && resultCode == Activity.RESULT_OK) {
            setResult(Activity.RESULT_OK)
            finish()
        }
    }

    private fun buildUi(): View {
        val root = FrameLayout(this).apply { setBackgroundColor(Color.BLACK) }
        previewView = PreviewView(this).apply {
            scaleType = PreviewView.ScaleType.FILL_CENTER
            implementationMode = PreviewView.ImplementationMode.PERFORMANCE
        }
        capturedView = ImageView(this).apply {
            scaleType = ImageView.ScaleType.CENTER_CROP
            visibility = View.GONE
            setBackgroundColor(Color.BLACK)
        }
        closeBtn = chip("✕").apply {
            textSize = 22f
            setOnClickListener { finishCancelled() }
        }
        flashBtn = chip("闪光灯关").apply {
            setOnClickListener {
                flashIndex += 1
                applyFlash()
            }
        }
        timerView = TextView(this).apply {
            setTextColor(Color.WHITE)
            textSize = 16f
            gravity = Gravity.CENTER
            visibility = View.GONE
        }
        shutterRing = View(this).apply {
            background = ring(Color.WHITE, dp(4), dp(72))
        }
        shutterInner = View(this).apply {
            background = oval(Color.WHITE)
        }
        val shutterWrap = FrameLayout(this)
        shutterWrap.addView(shutterRing, FrameLayout.LayoutParams(dp(72), dp(72), Gravity.CENTER))
        shutterWrap.addView(shutterInner, FrameLayout.LayoutParams(dp(56), dp(56), Gravity.CENTER))
        shutterWrap.setOnTouchListener { _, event -> handleShutterTouch(event) }
        albumThumb = ImageView(this).apply {
            scaleType = ImageView.ScaleType.CENTER_CROP
            background = roundRect(0xFF2A2A2A.toInt(), dp(6))
            clipToOutline = true
            setOnClickListener { openAlbum() }
        }
        switchBtn = chip("翻转").apply {
            setOnClickListener {
                lensFacing =
                    if (lensFacing == CameraSelector.LENS_FACING_BACK) {
                        CameraSelector.LENS_FACING_FRONT
                    } else {
                        CameraSelector.LENS_FACING_BACK
                    }
                startCamera()
            }
        }
        photoTab = chip("拍照").apply {
            setOnClickListener {
                if (recording != null) return@setOnClickListener
                lane = "photo"
                applyLaneChrome()
            }
        }
        videoTab = chip("录像").apply {
            setOnClickListener {
                if (recording != null) return@setOnClickListener
                lane = "video"
                applyLaneChrome()
            }
        }
        modeRow = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER
        }
        modeRow.addView(photoTab)
        modeRow.addView(videoTab)
        val retake = chip("重拍").apply {
            textSize = 16f
            setOnClickListener { retake() }
        }
        val use = TextView(this).apply {
            text = "✓"
            gravity = Gravity.CENTER
            textSize = 22f
            setTextColor(Color.WHITE)
            background = oval(ContextCompat.getColor(this@CameraActivity, R.color.kim_picker_accent))
            layoutParams = LinearLayout.LayoutParams(dp(56), dp(56))
            setOnClickListener { useCapture() }
        }
        reviewBar = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            visibility = View.GONE
            setPadding(dp(24), dp(16), dp(24), dp(32))
        }
        reviewBar.addView(retake)
        reviewBar.addView(View(this), LinearLayout.LayoutParams(0, 1, 1f))
        reviewBar.addView(use)

        val bottom = FrameLayout(this)
        bottom.addView(
            albumThumb,
            FrameLayout.LayoutParams(dp(48), dp(48), Gravity.CENTER_VERTICAL or Gravity.START).apply {
                marginStart = dp(28)
            },
        )
        bottom.addView(shutterWrap, FrameLayout.LayoutParams(dp(80), dp(80), Gravity.CENTER))
        bottom.addView(
            switchBtn,
            FrameLayout.LayoutParams(WRAP, WRAP, Gravity.CENTER_VERTICAL or Gravity.END).apply {
                marginEnd = dp(24)
            },
        )

        root.addView(previewView, FrameLayout.LayoutParams(MATCH, MATCH))
        root.addView(capturedView, FrameLayout.LayoutParams(MATCH, MATCH))
        root.addView(
            closeBtn,
            FrameLayout.LayoutParams(WRAP, WRAP, Gravity.TOP or Gravity.START).apply {
                topMargin = dp(16)
                marginStart = dp(12)
            },
        )
        root.addView(
            flashBtn,
            FrameLayout.LayoutParams(WRAP, WRAP, Gravity.TOP or Gravity.END).apply {
                topMargin = dp(16)
                marginEnd = dp(12)
            },
        )
        root.addView(
            timerView,
            FrameLayout.LayoutParams(WRAP, WRAP, Gravity.TOP or Gravity.CENTER_HORIZONTAL).apply {
                topMargin = dp(20)
            },
        )
        root.addView(
            modeRow,
            FrameLayout.LayoutParams(MATCH, WRAP, Gravity.BOTTOM).apply {
                bottomMargin = dp(140)
            },
        )
        root.addView(
            bottom,
            FrameLayout.LayoutParams(MATCH, dp(140), Gravity.BOTTOM),
        )
        root.addView(
            reviewBar,
            FrameLayout.LayoutParams(MATCH, WRAP, Gravity.BOTTOM),
        )
        return root
    }

    private var downAt = 0L
    private val longPress = Runnable {
        if (allowsVideo && !videoLane && recording == null) {
            startRecording(fromLongPress = true)
        }
    }

    private fun handleShutterTouch(event: MotionEvent): Boolean {
        when (event.actionMasked) {
            MotionEvent.ACTION_DOWN -> {
                downAt = SystemClock.elapsedRealtime()
                if (allowsVideo && !videoLane && recording == null) {
                    main.postDelayed(longPress, 350)
                }
            }
            MotionEvent.ACTION_UP, MotionEvent.ACTION_CANCEL -> {
                main.removeCallbacks(longPress)
                val held = SystemClock.elapsedRealtime() - downAt
                if (longPressRecording && recording != null) {
                    longPressRecording = false
                    stopRecording()
                } else if (event.actionMasked == MotionEvent.ACTION_UP && held < 350) {
                    onShutterTap()
                }
            }
        }
        return true
    }

    private fun chip(label: String): TextView {
        return TextView(this).apply {
            text = label
            setTextColor(Color.WHITE)
            textSize = 14f
            setPadding(dp(10), dp(8), dp(10), dp(8))
        }
    }

    private fun oval(color: Int): GradientDrawable {
        return GradientDrawable().apply {
            shape = GradientDrawable.OVAL
            setColor(color)
        }
    }

    private fun ring(color: Int, stroke: Int, size: Int): GradientDrawable {
        return GradientDrawable().apply {
            shape = GradientDrawable.OVAL
            setColor(Color.TRANSPARENT)
            setStroke(stroke, color)
            setSize(size, size)
        }
    }

    private fun roundRect(color: Int, radius: Int): GradientDrawable {
        return GradientDrawable().apply {
            setColor(color)
            cornerRadius = radius.toFloat()
        }
    }

    companion object {
        private const val REQ_PERM = 81
        private const val REQ_ALBUM = 82
        private const val REQ_MIC = 83
        private const val MATCH = ViewGroup.LayoutParams.MATCH_PARENT
        private const val WRAP = ViewGroup.LayoutParams.WRAP_CONTENT
    }
}
