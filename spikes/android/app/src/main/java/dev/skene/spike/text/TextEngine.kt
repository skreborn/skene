package dev.skene.spike.text

import android.graphics.Paint
import android.os.Build
import android.text.BoringLayout
import android.text.Layout
import android.text.PrecomputedText
import android.text.StaticLayout
import android.text.TextDirectionHeuristics
import android.text.TextPaint
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.text.BreakIterator
import java.util.Locale
import kotlin.math.ceil
import kotlin.math.max

/**
 * A StaticLayout/BoringLayout text engine answering the Rust leaf with numbers only.
 *
 * The answers go into [buffer], a direct buffer the Rust side reads after each call. Layouts are
 * cached by (text id, width); intrinsics by text id. Pinned parameters match Compose's defaults:
 * no font padding, fallback line spacing, simple breaking, no hyphenation.
 *
 * [useBoring] takes BoringLayout for single-line text; below API 33 BoringLayout cannot apply
 * fallback line spacing, so it is off there by default. [precompute] measures each text once into
 * a PrecomputedText that intrinsics and every layout reuse.
 */
class TextEngine(
  private val corpus: Corpus,
  val useBoring: Boolean = Build.VERSION.SDK_INT >= 33,
  val precompute: Boolean = true,
) {
  val buffer: ByteBuffer = ByteBuffer.allocateDirect(64).order(ByteOrder.nativeOrder())
  private val paints = Corpus.SIZES.map { size ->
    TextPaint(Paint.ANTI_ALIAS_FLAG).apply { textSize = size }
  }
  private val boring = arrayOfNulls<BoringLayout.Metrics>(corpus.leaves)
  private val boringKnown = BooleanArray(corpus.leaves)
  private val widths = arrayOfNulls<FloatArray>(corpus.leaves)
  private val measured = arrayOfNulls<CharSequence>(corpus.leaves)
  private val params = paints.map { paint ->
    PrecomputedText.Params.Builder(paint)
      .setBreakStrategy(Layout.BREAK_STRATEGY_SIMPLE)
      .setHyphenationFrequency(Layout.HYPHENATION_FREQUENCY_NONE)
      .setTextDirection(TextDirectionHeuristics.FIRSTSTRONG_LTR)
      .build()
  }
  private val cache = HashMap<Long, Laid>()

  /** Layout-cache hits and misses since construction. */
  var hits = 0
  var misses = 0

  /** One layout of one text at one width, and the numbers the Rust side reads from it. */
  class Laid(
    val layout: Layout,
    val width: Int,
    val used: Float,
    val height: Int,
    val first: Int,
    val last: Int,
    val lines: Int,
    val boring: Boolean,
  )

  fun paint(id: Int): TextPaint = paints[corpus.style(id)]

  /** The text of [id], precomputed once when [precompute] is on. */
  fun source(id: Int): CharSequence {
    if (!precompute) return corpus.text(id)

    measured[id]?.let { return it }

    val text = PrecomputedText.create(corpus.text(id), params[corpus.style(id)])
    measured[id] = text
    return text
  }

  private fun boringOf(id: Int): BoringLayout.Metrics? {
    if (!useBoring) return null

    if (!boringKnown[id]) {
      val text = source(id)
      boring[id] = if (Build.VERSION.SDK_INT >= 33) {
        BoringLayout.isBoring(text, paint(id), TextDirectionHeuristics.FIRSTSTRONG_LTR, true, null)
      } else {
        BoringLayout.isBoring(text, paint(id))
      }
      boringKnown[id] = true
    }

    return boring[id]
  }

  /** Builds a fresh layout of text [id] at [width] pixels, bypassing every cache. */
  fun build(id: Int, width: Int): Layout {
    val text = source(id)
    val paint = paint(id)
    val metrics = boringOf(id)

    if (metrics != null && metrics.width <= width) {
      return if (Build.VERSION.SDK_INT >= 33) {
        BoringLayout.make(
          text, paint, width, Layout.Alignment.ALIGN_NORMAL, metrics, false, null, width, true,
        )
      } else {
        BoringLayout.make(text, paint, width, Layout.Alignment.ALIGN_NORMAL, 1f, 0f, metrics, false)
      }
    }

    return StaticLayout.Builder.obtain(text, 0, text.length, paint, width)
      .setIncludePad(false)
      .setUseLineSpacingFromFallbacks(true)
      .setBreakStrategy(Layout.BREAK_STRATEGY_SIMPLE)
      .setHyphenationFrequency(Layout.HYPHENATION_FREQUENCY_NONE)
      .setTextDirection(TextDirectionHeuristics.FIRSTSTRONG_LTR)
      .build()
  }

  /** The cached layout of text [id] at [width] pixels. */
  fun laid(id: Int, width: Int): Laid {
    val key = (id.toLong() shl 32) or width.toLong()
    val found = cache[key]

    if (found != null) {
      hits++
      return found
    }

    misses++
    val layout = build(id, width)
    var used = 0f

    for (line in 0 until layout.lineCount) {
      used = max(used, layout.getLineWidth(line))
    }

    val ceiled = ceil(used)
    val laid = Laid(
      layout = layout,
      width = width,
      used = if (ceiled <= width) ceiled else used,
      height = layout.height,
      first = layout.getLineBaseline(0),
      last = layout.getLineBaseline(layout.lineCount - 1),
      lines = layout.lineCount,
      boring = layout is BoringLayout,
    )
    cache[key] = laid
    return laid
  }

  /** Min-content and max-content widths of text [id], ceiled to whole pixels. */
  fun intrinsicWidths(id: Int): FloatArray {
    widths[id]?.let { return it }

    val text = source(id)
    val paint = paint(id)
    val metrics = boringOf(id)
    val pre = text as? PrecomputedText
    val max = ceil(
      metrics?.width?.toFloat() ?: pre?.getWidth(0, text.length) ?: Layout.getDesiredWidth(text, paint),
    )
    val breaks = BreakIterator.getLineInstance(Locale.getDefault())
    breaks.setText(corpus.text(id))

    var min = 0f
    var start = breaks.first()
    var end = breaks.next()

    while (end != BreakIterator.DONE) {
      var trimmed = end

      while (trimmed > start && text[trimmed - 1].isWhitespace()) trimmed--

      if (trimmed > start) {
        min = max(min, pre?.getWidth(start, trimmed) ?: Layout.getDesiredWidth(text, start, trimmed, paint))
      }

      start = end
      end = breaks.next()
    }

    val result = floatArrayOf(minOf(ceil(min), max), max)
    widths[id] = result
    return result
  }

  /** JNI: writes min, max, then the height and the first and last baselines at max. */
  fun intrinsics(id: Int) {
    val (min, max) = intrinsicWidths(id).let { it[0] to it[1] }
    val atMax = laid(id, max.toInt())
    buffer.putFloat(0, min)
    buffer.putFloat(4, max)
    buffer.putFloat(8, atMax.height.toFloat())
    buffer.putFloat(12, atMax.first.toFloat())
    buffer.putFloat(16, atMax.last.toFloat())
  }

  /** JNI: writes the used width, the height and the first and last baselines at [width]. */
  fun layout(id: Int, width: Int) {
    val laid = laid(id, width)
    buffer.putFloat(0, laid.used)
    buffer.putFloat(4, laid.height.toFloat())
    buffer.putFloat(8, laid.first.toFloat())
    buffer.putFloat(12, laid.last.toFloat())
  }

  /** Drops every cached layout and intrinsic, keeping nothing warm but the platform's caches. */
  fun clear() {
    cache.clear()
    widths.fill(null)
    measured.fill(null)
  }
}
