package dev.skene.spike.text

import android.content.Context
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.graphics.Typeface
import android.graphics.fonts.FontFamily
import android.graphics.text.TextRunShaper
import android.os.Build
import android.text.StaticLayout
import android.text.TextPaint
import android.view.View

/**
 * Three bands of the same grey text on a grey ground, drawn through the hardware renderer: a
 * `StaticLayout` (option A), `Canvas.drawTextRun` in a single-font typeface (option C's floor),
 * and `Canvas.drawGlyphs` of Minikin's own glyphs (option C, API 31). High-contrast text, where
 * the renderer applies it, turns a band's text black or white with an outline of the other.
 */
class ContrastView(context: Context) : View(context) {
  private val text = "High contrast text Hamburgefonstiv"
  private val paint = TextPaint(Paint.ANTI_ALIAS_FLAG).apply {
    textSize = 64f
    color = TEXT
  }
  private val layout = StaticLayout.Builder.obtain(text, 0, text.length, paint, 2000)
    .setIncludePad(false)
    .build()

  override fun onDraw(canvas: Canvas) {
    canvas.drawColor(GROUND)
    canvas.save()
    canvas.translate(20f, 20f)
    layout.draw(canvas)
    canvas.restore()

    val baseline = 20f + BAND + layout.getLineBaseline(0)

    if (Build.VERSION.SDK_INT >= 31) {
      val glyphs = TextRunShaper.shapeTextRun(text, 0, text.length, 0, text.length, 20f, baseline, false, paint)
      val font = glyphs.getFont(0)
      val single = Typeface.CustomFallbackBuilder(FontFamily.Builder(font).build()).build()
      val runPaint = Paint(paint).apply { typeface = single }
      canvas.drawTextRun(text, 0, text.length, 0, text.length, 20f, baseline, false, runPaint)

      val ids = IntArray(glyphs.glyphCount()) { glyphs.getGlyphId(it) }
      val positions = FloatArray(ids.size * 2) {
        if (it % 2 == 0) glyphs.getGlyphX(it / 2) else glyphs.getGlyphY(it / 2) + BAND
      }
      canvas.drawGlyphs(ids, 0, positions, 0, ids.size, font, Paint(paint))
    } else {
      canvas.drawTextRun(text, 0, text.length, 0, text.length, 20f, baseline, false, paint)
    }
  }

  companion object {
    /** The height of a band, in pixels. */
    const val BAND = 120
    /** The text's grey. */
    val TEXT = Color.rgb(0x77, 0x77, 0x77)
    /** The ground's grey. */
    val GROUND = Color.rgb(0xBB, 0xBB, 0xBB)

    /** The layout's line height, for sizing the view. */
    fun height(): Int = BAND * 3 + 40
  }
}
