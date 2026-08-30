package com.kim.kim_media_picker

import android.app.Activity
import android.content.Intent
import io.flutter.embedding.engine.plugins.FlutterPlugin
import io.flutter.embedding.engine.plugins.activity.ActivityAware
import io.flutter.embedding.engine.plugins.activity.ActivityPluginBinding
import io.flutter.plugin.common.MethodCall
import io.flutter.plugin.common.MethodChannel
import io.flutter.plugin.common.PluginRegistry

class KimMediaPickerPlugin :
    FlutterPlugin,
    MethodChannel.MethodCallHandler,
    ActivityAware,
    PluginRegistry.ActivityResultListener {
    private lateinit var channel: MethodChannel
    private var activity: Activity? = null
    private var pending: MethodChannel.Result? = null
    private var binding: ActivityPluginBinding? = null

    override fun onAttachedToEngine(binding: FlutterPlugin.FlutterPluginBinding) {
        channel = MethodChannel(binding.binaryMessenger, "kim.media_picker")
        channel.setMethodCallHandler(this)
    }

    override fun onDetachedFromEngine(binding: FlutterPlugin.FlutterPluginBinding) {
        channel.setMethodCallHandler(null)
    }

    override fun onAttachedToActivity(binding: ActivityPluginBinding) {
        activity = binding.activity
        this.binding = binding
        binding.addActivityResultListener(this)
    }

    override fun onDetachedFromActivityForConfigChanges() {
        detachActivity()
    }

    override fun onReattachedToActivityForConfigChanges(binding: ActivityPluginBinding) {
        onAttachedToActivity(binding)
    }

    override fun onDetachedFromActivity() {
        detachActivity()
    }

    private fun detachActivity() {
        binding?.removeActivityResultListener(this)
        binding = null
        activity = null
    }

    override fun onMethodCall(call: MethodCall, result: MethodChannel.Result) {
        val act = activity
        if (act == null) {
            result.error("unavailable", "no activity", null)
            return
        }
        if (pending != null) {
            result.error("already_active", "picker already open", null)
            return
        }
        when (call.method) {
            "pickSingle" -> openAlbum(act, result, 1)
            "pickMultiple", "pickAlbum" -> {
                val max = (call.argument<Number>("maxCount")?.toInt() ?: 9).coerceAtLeast(1)
                openAlbum(act, result, max)
            }
            "capture", "takePhoto" -> {
                val mode =
                    call.argument<String>("mode")
                        ?: if (call.method == "takePhoto") "photo" else "mixed"
                PickerSession.reset(1, mode)
                pending = result
                act.startActivityForResult(
                    Intent(act, CameraActivity::class.java)
                        .putExtra(PickerSession.extraCaptureMode, mode),
                    REQ_CAMERA,
                )
            }
            else -> result.notImplemented()
        }
    }

    private fun openAlbum(act: Activity, result: MethodChannel.Result, max: Int) {
        PickerSession.reset(max)
        pending = result
        act.startActivityForResult(
            Intent(act, AlbumActivity::class.java)
                .putExtra(PickerSession.extraMaxCount, max),
            REQ_ALBUM,
        )
    }

    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?): Boolean {
        if (requestCode != REQ_ALBUM && requestCode != REQ_CAMERA) {
            return false
        }
        val result = pending ?: return true
        pending = null
        if (resultCode != Activity.RESULT_OK) {
            PickerSession.resultAssets = null
            result.success(emptyList<Map<String, Any>>())
            return true
        }
        val assets = PickerSession.resultAssets
        PickerSession.resultAssets = null
        result.success(assets ?: emptyList<Map<String, Any>>())
        return true
    }

    companion object {
        private const val REQ_ALBUM = 0x4B01
        private const val REQ_CAMERA = 0x4B02
    }
}
