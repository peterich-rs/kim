package com.kim.kim_media_picker

import android.app.Activity
import android.content.Intent
import android.content.pm.PackageManager
import android.graphics.Color
import android.graphics.Typeface
import android.graphics.drawable.GradientDrawable
import android.os.Bundle
import android.view.Gravity
import android.view.View
import android.view.ViewGroup
import android.widget.FrameLayout
import android.widget.ImageView
import android.widget.LinearLayout
import android.widget.TextView
import androidx.appcompat.app.AppCompatActivity
import androidx.core.app.ActivityCompat
import androidx.core.content.ContextCompat
import androidx.recyclerview.widget.GridLayoutManager
import androidx.recyclerview.widget.LinearLayoutManager
import androidx.recyclerview.widget.RecyclerView

class AlbumActivity : AppCompatActivity() {
    private lateinit var thumbs: ThumbLoader
    private lateinit var grid: RecyclerView
    private lateinit var albumList: RecyclerView
    private lateinit var albumScrim: View
    private lateinit var titleView: TextView
    private lateinit var previewBtn: TextView
    private lateinit var sendBtn: TextView
    private lateinit var emptyView: TextView
    private var bucketId = "all"
    private var shown: List<MediaItem> = emptyList()
    private var albumOpen = false

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        thumbs = ThumbLoader(this)
        PickerSession.maxCount =
            intent.getIntExtra(PickerSession.extraMaxCount, PickerSession.maxCount).coerceAtLeast(1)
        setContentView(buildUi())
        if (PickerPerms.hasImages(this)) {
            load()
        } else {
            ActivityCompat.requestPermissions(this, PickerPerms.imagePermissions(), REQ_PERM)
        }
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
        if (grantResults.any { it == PackageManager.PERMISSION_GRANTED }) {
            load()
        } else {
            PickerPerms.deniedDialog(this, "请在设置中开启相册权限") { finishCancelled() }
        }
    }

    private fun load() {
        val (items, albums) = MediaStoreLoader.load(this)
        PickerSession.items = items
        PickerSession.albums = albums
        applyBucket("all")
        albumList.adapter = AlbumAdapter()
    }

    private fun applyBucket(id: String) {
        bucketId = id
        shown =
            if (id == "all") {
                PickerSession.items
            } else {
                PickerSession.items.filter { it.bucketId == id }
            }
        val name = PickerSession.albums.firstOrNull { it.id == id }?.name ?: "所有照片"
        titleView.text = "$name ▾"
        emptyView.visibility = if (shown.isEmpty()) View.VISIBLE else View.GONE
        grid.adapter = GridAdapter()
        refreshBar()
    }

    private fun refreshBar() {
        val n = PickerSession.selected.size
        previewBtn.isEnabled = n > 0
        previewBtn.alpha = if (n > 0) 1f else 0.35f
        sendBtn.isEnabled = n > 0
        sendBtn.text =
            if (n == 0 || PickerSession.single) {
                "发送"
            } else {
                "发送($n)"
            }
        val bg = GradientDrawable()
        bg.cornerRadius = dp(4).toFloat()
        if (n > 0) {
            bg.setColor(ContextCompat.getColor(this, R.color.kim_picker_accent))
            sendBtn.setTextColor(Color.WHITE)
        } else {
            bg.setColor(0xFFD8D8D8.toInt())
            sendBtn.setTextColor(0xFF888888.toInt())
        }
        sendBtn.background = bg
        grid.adapter?.notifyDataSetChanged()
    }

    private fun finishCancelled() {
        setResult(Activity.RESULT_CANCELED)
        finish()
    }

    private fun finishOk() {
        PickerSession.resultAssets = MediaExport.export(this, PickerSession.selected.values)
        setResult(Activity.RESULT_OK)
        finish()
    }

    private fun openPreview(index: Int) {
        startActivityForResult(
            Intent(this, PreviewActivity::class.java).putExtra(PickerSession.extraIndex, index),
            REQ_PREVIEW,
        )
    }

    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        super.onActivityResult(requestCode, resultCode, data)
        if (requestCode != REQ_PREVIEW) {
            return
        }
        if (resultCode == Activity.RESULT_OK) {
            setResult(Activity.RESULT_OK)
            finish()
            return
        }
        refreshBar()
    }

    private fun toggleAlbumSheet() {
        albumOpen = !albumOpen
        val vis = if (albumOpen) View.VISIBLE else View.GONE
        albumScrim.visibility = vis
        albumList.visibility = vis
    }

    private fun buildUi(): View {
        val root = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setBackgroundColor(ContextCompat.getColor(this@AlbumActivity, R.color.kim_picker_bg))
        }
        root.addView(buildToolbar(), LinearLayout.LayoutParams(MATCH, dp(52)))
        val body = FrameLayout(this)
        grid = RecyclerView(this).apply {
            layoutManager = GridLayoutManager(this@AlbumActivity, 4)
            setBackgroundColor(ContextCompat.getColor(this@AlbumActivity, R.color.kim_picker_bg))
            itemAnimator = null
        }
        emptyView = TextView(this).apply {
            text = "没有照片"
            gravity = Gravity.CENTER
            setTextColor(ContextCompat.getColor(this@AlbumActivity, R.color.kim_picker_muted))
            visibility = View.GONE
        }
        albumScrim = View(this).apply {
            setBackgroundColor(0x99000000.toInt())
            visibility = View.GONE
            setOnClickListener { toggleAlbumSheet() }
        }
        albumList = RecyclerView(this).apply {
            layoutManager = LinearLayoutManager(this@AlbumActivity)
            setBackgroundColor(ContextCompat.getColor(this@AlbumActivity, R.color.kim_picker_bg))
            visibility = View.GONE
            layoutParams = FrameLayout.LayoutParams(MATCH, dp(360), Gravity.TOP)
        }
        body.addView(grid, FrameLayout.LayoutParams(MATCH, MATCH))
        body.addView(emptyView, FrameLayout.LayoutParams(MATCH, MATCH))
        body.addView(albumScrim, FrameLayout.LayoutParams(MATCH, MATCH))
        body.addView(albumList)
        root.addView(body, LinearLayout.LayoutParams(MATCH, 0, 1f))
        root.addView(buildBottom(), LinearLayout.LayoutParams(MATCH, dp(52)))
        return root
    }

    private fun buildToolbar(): View {
        val bar = FrameLayout(this).apply {
            setBackgroundColor(ContextCompat.getColor(this@AlbumActivity, R.color.kim_picker_bg))
        }
        val cancel = TextView(this).apply {
            text = "取消"
            setTextColor(ContextCompat.getColor(this@AlbumActivity, R.color.kim_picker_text))
            textSize = 16f
            gravity = Gravity.CENTER_VERTICAL
            setPadding(dp(16), 0, dp(16), 0)
            setOnClickListener { finishCancelled() }
        }
        titleView = TextView(this).apply {
            text = "所有照片 ▾"
            setTextColor(ContextCompat.getColor(this@AlbumActivity, R.color.kim_picker_text))
            textSize = 17f
            typeface = Typeface.DEFAULT_BOLD
            gravity = Gravity.CENTER
            setOnClickListener { toggleAlbumSheet() }
        }
        bar.addView(cancel, FrameLayout.LayoutParams(WRAP, MATCH, Gravity.START))
        bar.addView(titleView, FrameLayout.LayoutParams(WRAP, MATCH, Gravity.CENTER))
        return bar
    }

    private fun buildBottom(): View {
        val bar = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            setBackgroundColor(ContextCompat.getColor(this@AlbumActivity, R.color.kim_picker_bar))
            setPadding(dp(16), 0, dp(12), 0)
        }
        previewBtn = TextView(this).apply {
            text = "预览"
            setTextColor(ContextCompat.getColor(this@AlbumActivity, R.color.kim_picker_text))
            textSize = 16f
            setOnClickListener {
                if (PickerSession.selected.isEmpty()) {
                    return@setOnClickListener
                }
                PickerSession.preview = PickerSession.selected.values.toList()
                openPreview(0)
            }
        }
        val spacer = View(this)
        sendBtn = TextView(this).apply {
            text = "发送"
            textSize = 14f
            typeface = Typeface.DEFAULT_BOLD
            gravity = Gravity.CENTER
            setPadding(dp(14), dp(6), dp(14), dp(6))
            setOnClickListener {
                if (PickerSession.selected.isNotEmpty()) {
                    finishOk()
                }
            }
        }
        bar.addView(previewBtn, LinearLayout.LayoutParams(WRAP, WRAP))
        bar.addView(spacer, LinearLayout.LayoutParams(0, 1, 1f))
        bar.addView(sendBtn, LinearLayout.LayoutParams(WRAP, WRAP))
        refreshBar()
        return bar
    }

    private inner class GridAdapter : RecyclerView.Adapter<GridHolder>() {
        override fun onCreateViewHolder(parent: ViewGroup, viewType: Int): GridHolder {
            val cell = FrameLayout(parent.context)
            val gap = dp(2)
            cell.setPadding(gap / 2, gap / 2, gap / 2, gap / 2)
            val img = ImageView(parent.context).apply {
                scaleType = ImageView.ScaleType.CENTER_CROP
                layoutParams = FrameLayout.LayoutParams(MATCH, MATCH)
            }
            val dim = View(parent.context).apply {
                setBackgroundColor(0x33000000.toInt())
                visibility = View.GONE
            }
            val badge = TextView(parent.context).apply {
                gravity = Gravity.CENTER
                textSize = 12f
                setTextColor(Color.WHITE)
                layoutParams = FrameLayout.LayoutParams(dp(24), dp(24), Gravity.TOP or Gravity.END).apply {
                    topMargin = dp(6)
                    marginEnd = dp(6)
                }
            }
            cell.addView(img)
            cell.addView(dim, FrameLayout.LayoutParams(MATCH, MATCH))
            cell.addView(badge)
            cell.layoutParams = RecyclerView.LayoutParams(MATCH, parent.measuredWidth / 4)
            return GridHolder(cell, img, dim, badge)
        }

        override fun getItemCount(): Int = shown.size

        override fun onBindViewHolder(holder: GridHolder, position: Int) {
            val item = shown[position]
            val size = resources.displayMetrics.widthPixels / 4
            holder.itemView.layoutParams.height = size
            thumbs.load(item.uri, holder.image, size)
            val index = PickerSession.selectedIndex(item.id)
            holder.dim.visibility = if (index > 0) View.VISIBLE else View.GONE
            val circle = GradientDrawable()
            circle.shape = GradientDrawable.OVAL
            if (index > 0) {
                circle.setColor(ContextCompat.getColor(this@AlbumActivity, R.color.kim_picker_accent))
                holder.badge.text = if (PickerSession.single) "" else index.toString()
            } else {
                circle.setColor(0x33000000.toInt())
                circle.setStroke(dp(1), Color.WHITE)
                holder.badge.text = ""
            }
            holder.badge.background = circle
            holder.badge.setOnClickListener {
                if (PickerSession.toggle(item, this@AlbumActivity)) {
                    refreshBar()
                }
            }
            holder.image.setOnClickListener {
                PickerSession.preview = shown
                openPreview(position)
            }
        }
    }

    private class GridHolder(
        view: View,
        val image: ImageView,
        val dim: View,
        val badge: TextView,
    ) : RecyclerView.ViewHolder(view)

    private inner class AlbumAdapter : RecyclerView.Adapter<AlbumHolder>() {
        override fun onCreateViewHolder(parent: ViewGroup, viewType: Int): AlbumHolder {
            val row = LinearLayout(parent.context).apply {
                orientation = LinearLayout.HORIZONTAL
                gravity = Gravity.CENTER_VERTICAL
                setPadding(dp(16), dp(10), dp(16), dp(10))
            }
            val cover = ImageView(parent.context).apply {
                scaleType = ImageView.ScaleType.CENTER_CROP
                layoutParams = LinearLayout.LayoutParams(dp(56), dp(56))
            }
            val name = TextView(parent.context).apply {
                setTextColor(ContextCompat.getColor(this@AlbumActivity, R.color.kim_picker_text))
                textSize = 16f
                setPadding(dp(12), 0, 0, 0)
            }
            row.addView(cover)
            row.addView(name, LinearLayout.LayoutParams(0, WRAP, 1f))
            row.layoutParams = RecyclerView.LayoutParams(MATCH, WRAP)
            return AlbumHolder(row, cover, name)
        }

        override fun getItemCount(): Int = PickerSession.albums.size

        override fun onBindViewHolder(holder: AlbumHolder, position: Int) {
            val album = PickerSession.albums[position]
            holder.name.text = "${album.name}（${album.count}）"
            val cover = album.cover
            if (cover != null) {
                thumbs.load(cover.uri, holder.cover, dp(56))
            } else {
                holder.cover.setImageResource(R.color.kim_picker_thumb)
            }
            holder.itemView.setOnClickListener {
                applyBucket(album.id)
                toggleAlbumSheet()
            }
        }
    }

    private class AlbumHolder(
        view: View,
        val cover: ImageView,
        val name: TextView,
    ) : RecyclerView.ViewHolder(view)

    companion object {
        private const val REQ_PERM = 71
        private const val REQ_PREVIEW = 72
        private const val MATCH = ViewGroup.LayoutParams.MATCH_PARENT
        private const val WRAP = ViewGroup.LayoutParams.WRAP_CONTENT
    }
}
