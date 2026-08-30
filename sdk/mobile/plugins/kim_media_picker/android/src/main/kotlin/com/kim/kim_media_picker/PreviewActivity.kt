package com.kim.kim_media_picker

import android.app.Activity
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
import androidx.core.content.ContextCompat
import androidx.recyclerview.widget.RecyclerView
import androidx.viewpager2.widget.ViewPager2

class PreviewActivity : AppCompatActivity() {
    private lateinit var pager: ViewPager2
    private lateinit var badge: TextView
    private lateinit var sendBtn: TextView
    private lateinit var thumbs: ThumbLoader
    private var items: List<MediaItem> = emptyList()

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        thumbs = ThumbLoader(this)
        items = PickerSession.preview.ifEmpty { PickerSession.items }
        if (items.isEmpty()) {
            finish()
            return
        }
        val start = intent.getIntExtra(PickerSession.extraIndex, 0).coerceIn(0, items.lastIndex)
        setContentView(buildUi())
        pager.adapter = PageAdapter()
        pager.setCurrentItem(start, false)
        pager.registerOnPageChangeCallback(
            object : ViewPager2.OnPageChangeCallback() {
                override fun onPageSelected(position: Int) {
                    refreshChrome()
                }
            },
        )
        refreshChrome()
    }

    private fun current(): MediaItem = items[pager.currentItem]

    private fun refreshChrome() {
        val item = current()
        val index = PickerSession.selectedIndex(item.id)
        val circle = GradientDrawable()
        circle.shape = GradientDrawable.OVAL
        if (index > 0) {
            circle.setColor(ContextCompat.getColor(this, R.color.kim_picker_accent))
            badge.text = index.toString()
        } else {
            circle.setColor(0x33000000.toInt())
            circle.setStroke(dp(1), Color.WHITE)
            badge.text = ""
        }
        badge.background = circle
        val n = PickerSession.selected.size
        sendBtn.isEnabled = n > 0
        sendBtn.text = if (n > 0) "发送($n)" else "发送"
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
    }

    private fun finishOk() {
        PickerSession.resultAssets = MediaExport.export(this, PickerSession.selected.values)
        setResult(Activity.RESULT_OK)
        finish()
    }

    private fun buildUi(): View {
        val root = FrameLayout(this).apply { setBackgroundColor(Color.BLACK) }
        pager = ViewPager2(this)
        val top = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            setPadding(dp(8), dp(12), dp(12), dp(12))
        }
        val back = TextView(this).apply {
            text = "‹"
            setTextColor(Color.WHITE)
            textSize = 28f
            setPadding(dp(12), 0, dp(12), 0)
            setOnClickListener { finish() }
        }
        badge = TextView(this).apply {
            gravity = Gravity.CENTER
            setTextColor(Color.WHITE)
            textSize = 13f
            typeface = Typeface.DEFAULT_BOLD
            setOnClickListener {
                if (PickerSession.toggle(current(), this@PreviewActivity)) {
                    refreshChrome()
                }
            }
        }
        top.addView(back)
        top.addView(View(this), LinearLayout.LayoutParams(0, 1, 1f))
        top.addView(badge, LinearLayout.LayoutParams(dp(28), dp(28)))

        val bottom = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            setPadding(dp(16), dp(12), dp(12), dp(24))
            setBackgroundColor(0xCC000000.toInt())
        }
        sendBtn = TextView(this).apply {
            text = "发送"
            textSize = 14f
            typeface = Typeface.DEFAULT_BOLD
            gravity = Gravity.CENTER
            setPadding(dp(14), dp(6), dp(14), dp(6))
            setOnClickListener {
                if (PickerSession.selected.isEmpty()) {
                    PickerSession.toggle(current(), this@PreviewActivity)
                }
                if (PickerSession.selected.isNotEmpty()) {
                    finishOk()
                }
            }
        }
        bottom.addView(View(this), LinearLayout.LayoutParams(0, 1, 1f))
        bottom.addView(sendBtn)

        root.addView(pager, FrameLayout.LayoutParams(MATCH, MATCH))
        root.addView(top, FrameLayout.LayoutParams(MATCH, WRAP, Gravity.TOP))
        root.addView(bottom, FrameLayout.LayoutParams(MATCH, WRAP, Gravity.BOTTOM))
        return root
    }

    private inner class PageAdapter : RecyclerView.Adapter<PageHolder>() {
        override fun onCreateViewHolder(parent: ViewGroup, viewType: Int): PageHolder {
            val image = ImageView(parent.context).apply {
                scaleType = ImageView.ScaleType.FIT_CENTER
                setBackgroundColor(Color.BLACK)
                layoutParams = ViewGroup.LayoutParams(MATCH, MATCH)
            }
            return PageHolder(image)
        }

        override fun getItemCount(): Int = items.size

        override fun onBindViewHolder(holder: PageHolder, position: Int) {
            thumbs.load(items[position].uri, holder.image, resources.displayMetrics.widthPixels)
        }
    }

    private class PageHolder(val image: ImageView) : RecyclerView.ViewHolder(image)

    companion object {
        private const val MATCH = ViewGroup.LayoutParams.MATCH_PARENT
        private const val WRAP = ViewGroup.LayoutParams.WRAP_CONTENT
    }
}
