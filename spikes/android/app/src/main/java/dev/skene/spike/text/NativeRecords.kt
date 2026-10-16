package dev.skene.spike.text

import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.graphics.Typeface
import android.graphics.fonts.Font
import android.graphics.fonts.FontStyle
import android.graphics.text.LineBreakConfig
import android.graphics.text.PositionedGlyphs
import android.graphics.text.TextRunShaper
import android.os.Build
import android.os.LocaleList
import android.text.Layout
import android.text.PrecomputedText
import android.text.StaticLayout
import android.text.TextDirectionHeuristics
import android.text.TextPaint
import android.text.TextUtils
import java.io.File
import java.io.FileOutputStream
import java.text.Bidi
import java.text.BreakIterator
import java.util.Locale
import kotlin.math.ceil
import kotlin.math.floor
import kotlin.math.max
import kotlin.math.roundToInt
import org.json.JSONArray
import org.json.JSONObject

/** One variant of the text-parity corpus, as `text-parity corpus --expanded` spells it. */
class Variant(o: JSONObject) {
  val id: String = o.getString("id")
  val stem: String = o.getString("file_stem")
  val case: String = o.getString("case")
  val size = o.getDouble("size").toFloat()
  val width: Float? = if (o.isNull("width")) null else o.getDouble("width").toFloat()
  val text: String = o.getString("text")
  val locale: String = o.getString("locale")
  val family: String = o.getString("family")
  val weight = o.getInt("weight")
  val italic = o.getBoolean("italic")
  val alignment: String = o.getString("alignment")
  val direction: String = o.getString("direction")
  val maxLines: Int? = if (o.isNull("max_lines")) null else o.getInt("max_lines")
  val ellipsis = o.getBoolean("ellipsis")
  val hyphenation: String = o.getString("hyphenation")
  val lineBreak: String = o.getString("line_break")
  val wordBreak: String = o.getString("word_break")
}

/** The corpus, read from the harness through the Rust library. */
object ParityCorpus {
  private val json by lazy { JSONObject(Parity.corpus()) }
  val version: Int get() = json.getInt("version")
  val variants: List<Variant> by lazy {
    val array = json.getJSONArray("variants")
    List(array.length()) { Variant(array.getJSONObject(it)) }
  }
}

/** The image key of a scale, as the harness spells it: `2`, `2.625`. */
fun scaleKey(scale: Float): String =
  if (scale == floor(scale)) scale.toInt().toString() else scale.toString()

/** The pixel size of an image of a `width` by `height` box at `scale`, with the 8-pixel margin. */
fun imageSize(width: Float, height: Float, scale: Float): Pair<Int, Int> {
  fun pixels(extent: Float) = max(1, ((ceil(max(extent, 0f)) + 16f) * scale).roundToInt())
  return pixels(width) to pixels(height)
}

/** Writes [bitmap] as a PNG at [file]. */
fun writePng(bitmap: Bitmap, file: File) {
  file.parentFile?.mkdirs()
  FileOutputStream(file).use { bitmap.compress(Bitmap.CompressFormat.PNG, 100, it) }
}

/**
 * Whether line [line] of [layout] ends in a hyphen Minikin drew: `Layout` has no public accessor
 * for its hyphen edits, so a line counts as hyphenated where it breaks inside a word (or at a soft
 * hyphen) and its drawn extent exceeds its text's by at least half a hyphen.
 */
fun drewHyphen(layout: Layout, line: Int, text: String, paint: TextPaint): Boolean {
  val start = layout.getLineStart(line)
  val end = layout.getLineEnd(line)

  if (end <= start || line + 1 >= layout.lineCount) return false

  val last = text[end - 1]

  if (last != '\u00AD' && !(last.isLetter() && text.getOrNull(end)?.isLetter() == true)) return false

  val extent = layout.getLineMax(line)
  val plain = Layout.getDesiredWidth(text, start, if (last == '\u00AD') end - 1 else end, paint)

  return extent - plain > paint.measureText("-") * 0.5f
}

/** The milliseconds of [nanos]. */
fun ms(nanos: Long) = nanos / 1e6

/** The key a platform font is compared by: file, face and axes. */
fun fontKey(font: Font): String {
  val axes = font.axes?.joinToString(",") { "${it.tag}=${it.styleValue}" } ?: ""
  return "${font.file?.absolutePath}#${font.ttcIndex}[$axes]"
}

/**
 * Option A: `StaticLayout` over Minikin, filling the harness's record schema.
 *
 * Text is laid out at the device's pixel size (logical size times [density]) as a `TextView`
 * would, and every coordinate is reported in logical pixels. [quality] picks the set: `false` is
 * `BREAK_STRATEGY_SIMPLE` without hyphenation (EditText's and Compose's defaults, and Parley's
 * greedy breaking), `true` is `BREAK_STRATEGY_HIGH_QUALITY` with `HYPHENATION_FREQUENCY_NORMAL`.
 * Below API 33 `StaticLayout` is used for every text (the spike's BoringLayout lesson). Fonts per
 * run come from `TextRunShaper` (API 31) or, below it, from `AFontMatcher`. [subpixel] sets
 * `SUBPIXEL_TEXT_FLAG`, which neither `TextView` nor Compose sets, so that glyph advances stay
 * fractional rather than whole device pixels.
 */
class NativeRecords(
  private val quality: Boolean,
  private val density: Float,
  private val subpixel: Boolean = false,
) {
  private val sdk = Build.VERSION.SDK_INT

  /** The typeface a case's family, weight and slant ask for. */
  fun typeface(variant: Variant): Typeface {
    val base = when (variant.family) {
      "system" -> Typeface.DEFAULT
      "serif" -> Typeface.SERIF
      "monospace" -> Typeface.MONOSPACE
      else -> Typeface.create(variant.family, Typeface.NORMAL)
    }
    return Typeface.create(base, variant.weight, variant.italic)
  }

  /** The paint of [variant] at [pixels] per em. */
  fun paint(variant: Variant, pixels: Float): TextPaint = TextPaint(Paint.ANTI_ALIAS_FLAG).apply {
    isSubpixelText = subpixel
    textSize = pixels
    typeface = typeface(variant)
    textLocales = LocaleList.forLanguageTags(variant.locale)
    color = Color.BLACK
  }

  /** The width in device pixels [variant] is laid out at: its own, or its text's max-content. */
  fun widthPx(variant: Variant, paint: TextPaint): Int = variant.width?.let { (it * density).roundToInt() }
    ?: ceil(Layout.getDesiredWidth(variant.text, paint)).toInt()

  /** Lays [variant] out, noting what the platform could not honour. */
  fun build(variant: Variant, paint: TextPaint, width: Int, notes: MutableList<String>? = null): StaticLayout {
    val text = variant.text
    val builder = StaticLayout.Builder.obtain(text, 0, text.length, paint, width)
      .setIncludePad(false)
      .setUseLineSpacingFromFallbacks(true)
      .setBreakStrategy(if (quality) Layout.BREAK_STRATEGY_HIGH_QUALITY else Layout.BREAK_STRATEGY_SIMPLE)
      .setHyphenationFrequency(
        if (quality) Layout.HYPHENATION_FREQUENCY_NORMAL else Layout.HYPHENATION_FREQUENCY_NONE,
      )
      .setTextDirection(
        when (variant.direction) {
          "ltr" -> TextDirectionHeuristics.LTR
          "rtl" -> TextDirectionHeuristics.RTL
          else -> TextDirectionHeuristics.FIRSTSTRONG_LTR
        },
      )
      .setAlignment(
        when (variant.alignment) {
          "center" -> Layout.Alignment.ALIGN_CENTER
          "end" -> Layout.Alignment.ALIGN_OPPOSITE
          else -> Layout.Alignment.ALIGN_NORMAL
        },
      )

    if (variant.alignment == "justify") builder.setJustificationMode(Layout.JUSTIFICATION_MODE_INTER_WORD)

    variant.maxLines?.let {
      builder.setMaxLines(it)
      if (variant.ellipsis) builder.setEllipsize(TextUtils.TruncateAt.END)
    }

    if (!quality && variant.hyphenation == "auto") notes?.add("automatic hyphenation off in this set (NONE)")

    if (sdk >= 33) {
      val style = when (variant.lineBreak) {
        "strict" -> LineBreakConfig.LINE_BREAK_STYLE_STRICT
        "loose" -> LineBreakConfig.LINE_BREAK_STYLE_LOOSE
        else -> LineBreakConfig.LINE_BREAK_STYLE_NONE
      }
      val word = when (variant.wordBreak) {
        "phrase" -> LineBreakConfig.LINE_BREAK_WORD_STYLE_PHRASE
        else -> LineBreakConfig.LINE_BREAK_WORD_STYLE_NONE
      }
      builder.setLineBreakConfig(LineBreakConfig.Builder().setLineBreakStyle(style).setLineBreakWordStyle(word).build())
    } else if (variant.lineBreak != "normal" || variant.wordBreak == "phrase") {
      notes?.add("LineBreakConfig needs API 33: line_break ${variant.lineBreak}, word_break ${variant.wordBreak} broken as normal")
    }

    if (variant.lineBreak == "anywhere") notes?.add("line_break anywhere unsupported")
    if (variant.wordBreak == "keep-all" || variant.wordBreak == "break-all") {
      notes?.add("word_break ${variant.wordBreak} unsupported")
    }

    return builder.build()
  }

  /** The lines a layout shows: all, or `max_lines`. */
  private fun visibleLines(variant: Variant, layout: Layout) =
    variant.maxLines?.let { minOf(it, layout.lineCount) } ?: layout.lineCount

  private fun font(font: Font?, size: Float, requested: Variant, bold: Boolean?, italic: Boolean?): JSONObject {
    val path = font?.file?.absolutePath
    val index = font?.ttcIndex ?: 0
    val (family, postscript) = path?.let { Parity.names(it, index) } ?: ("" to "")
    val style = font?.style
    val weight = style?.weight ?: requested.weight
    val slanted = style?.slant == FontStyle.FONT_SLANT_ITALIC
    return JSONObject().apply {
      put("family", family)
      put("postscript", postscript)
      put("path", path ?: JSONObject.NULL)
      put("index", index)
      put("size", size)
      put("weight", weight)
      put("italic", slanted)
      put("synthetic_bold", bold ?: (requested.weight >= 600 && requested.weight - weight >= 200))
      put("synthetic_italic", italic ?: (requested.italic && !slanted))
    }
  }

  /** The visible end of line [line]: before an ellipsis, trailing blanks and the line break. */
  private fun visibleEnd(layout: Layout, line: Int, text: String): Int {
    val start = layout.getLineStart(line)
    var end = if (layout.getEllipsisCount(line) > 0) start + layout.getEllipsisStart(line) else layout.getLineEnd(line)
    while (end > start && text[end - 1].isWhitespace()) end--
    return end
  }

  /**
   * Fonts per run and, from API 31, the glyphs Minikin placed: each line's bidi runs shaped
   * through `TextRunShaper` in visual order from the line's left. A run whose glyphs come from
   * more than one font is split per grapheme, each grapheme shaped alone with the line as context.
   */
  private fun runsAndGlyphs(variant: Variant, layout: Layout, paint: TextPaint, lines: Int, notes: MutableList<String>): Pair<JSONArray, JSONArray> {
    val runs = JSONArray()
    val glyphRuns = JSONArray()
    val text = variant.text

    if (sdk < 31) {
      notes.add("runs from AFontMatcher (TextRunShaper needs API 31)")

      for (line in 0 until lines) {
        val start = layout.getLineStart(line)
        val end = visibleEnd(layout, line, text)

        if (end <= start) continue

        val matched = JSONArray(Parity.matchRuns("sans-serif", text.substring(start, end), variant.locale, variant.weight, variant.italic))

        for (at in 0 until matched.length()) {
          val run = matched.getJSONObject(at)
          val info = run.getJSONObject("font")
          val path = info.getString("path")
          val index = info.getInt("index")
          val (family, postscript) = Parity.names(path, index)
          val weight = info.getInt("weight")
          runs.put(JSONObject().apply {
            put("utf16", JSONArray(listOf(start + run.getInt("start"), start + run.getInt("end"))))
            put("line", line)
            put("family", family)
            put("postscript", postscript)
            put("path", path)
            put("index", index)
            put("size", variant.size)
            put("weight", weight)
            put("italic", info.getBoolean("italic"))
            put("synthetic_bold", variant.weight >= 600 && variant.weight - weight >= 200)
            put("synthetic_italic", variant.italic && !info.getBoolean("italic"))
            put("rtl", layout.isRtlCharAt(start + run.getInt("start")))
          })
        }
      }

      return runs to glyphRuns
    }

    val characters = BreakIterator.getCharacterInstance(Locale.forLanguageTag(variant.locale))

    for (line in 0 until lines) {
      val start = layout.getLineStart(line)
      val end = visibleEnd(layout, line, text)

      if (end <= start) continue

      val rtlParagraph = layout.getParagraphDirection(line) == Layout.DIR_RIGHT_TO_LEFT
      val bidi = Bidi(
        text.substring(start, end),
        if (rtlParagraph) Bidi.DIRECTION_RIGHT_TO_LEFT else Bidi.DIRECTION_LEFT_TO_RIGHT,
      )
      val count = bidi.runCount
      val levels = ByteArray(count) { bidi.getRunLevel(it).toByte() }
      val order = Array<Any>(count) { it }
      Bidi.reorderVisually(levels, 0, order, 0, count)
      var x = layout.getLineLeft(line)
      val baseline = layout.getLineBaseline(line).toFloat()

      for (slot in order) {
        val at = slot as Int
        val runStart = start + bidi.getRunStart(at)
        val runEnd = start + bidi.getRunLimit(at)
        val rtl = bidi.getRunLevel(at) % 2 == 1
        val glyphs = TextRunShaper.shapeTextRun(text, runStart, runEnd - runStart, start, end - start, x, baseline, rtl, paint)
        appendGlyphs(glyphRuns, glyphs, line, variant)

        val keys = (0 until glyphs.glyphCount()).map { fontKey(glyphs.getFont(it)) }.distinct()

        if (keys.size <= 1 && glyphs.glyphCount() > 0) {
          val fake = if (sdk >= 35) glyphs.getFakeBold(0) else null
          val fakeItalic = if (sdk >= 35) glyphs.getFakeItalic(0) else null
          runs.put(font(glyphs.getFont(0), variant.size, variant, fake, fakeItalic).apply {
            put("utf16", JSONArray(listOf(runStart, runEnd)))
            put("line", line)
            put("rtl", rtl)
          })
        } else if (glyphs.glyphCount() > 0) {
          characters.setText(text.substring(runStart, runEnd))
          var from = characters.first()
          var to = characters.next()
          var current: JSONObject? = null
          var currentKey = ""

          while (to != BreakIterator.DONE) {
            val single = TextRunShaper.shapeTextRun(text, runStart + from, to - from, start, end - start, 0f, 0f, rtl, paint)

            if (single.glyphCount() > 0) {
              val key = fontKey(single.getFont(0))

              if (current != null && key == currentKey) {
                current.put("utf16", JSONArray(listOf(current.getJSONArray("utf16").getInt(0), runStart + to)))
              } else {
                current?.let { runs.put(it) }
                val fake = if (sdk >= 35) single.getFakeBold(0) else null
                val fakeItalic = if (sdk >= 35) single.getFakeItalic(0) else null
                current = font(single.getFont(0), variant.size, variant, fake, fakeItalic).apply {
                  put("utf16", JSONArray(listOf(runStart + from, runStart + to)))
                  put("line", line)
                  put("rtl", rtl)
                }
                currentKey = key
              }
            }

            from = to
            to = characters.next()
          }

          current?.let { runs.put(it) }
          notes.add("line $line: fonts split per grapheme")
        }

        x += glyphs.advance
      }
    }

    return runs to glyphRuns
  }

  /** Appends Minikin's glyphs as glyph runs in logical pixels, one per font. */
  private fun appendGlyphs(out: JSONArray, glyphs: PositionedGlyphs, line: Int, variant: Variant) {
    var at = 0

    while (at < glyphs.glyphCount()) {
      val font = glyphs.getFont(at)
      val key = fontKey(font)
      var end = at

      while (end < glyphs.glyphCount() && fontKey(glyphs.getFont(end)) == key) end++

      val path = font.file?.absolutePath
      val (family, postscript) = path?.let { Parity.names(it, font.ttcIndex) } ?: ("" to "")
      val list = JSONArray()

      for (i in at until end) {
        list.put(JSONObject().apply {
          put("id", glyphs.getGlyphId(i))
          put("x", (glyphs.getGlyphX(i) / density).toDouble())
          put("y", (glyphs.getGlyphY(i) / density).toDouble())
          put("cluster", 0)
        })
      }

      out.put(JSONObject().apply {
        put("line", line)
        put("font", JSONObject().apply {
          put("family", family)
          put("postscript", postscript)
          put("path", path ?: JSONObject.NULL)
          put("index", font.ttcIndex)
          put("data_len", font.file?.length() ?: 0)
        })
        put("size", variant.size)
        put("coords", JSONArray())
        put("variations", JSONArray().apply {
          font.axes?.forEach { put(JSONObject().put("tag", it.tag).put("value", it.styleValue.toDouble())) }
        })
        put("embolden", Build.VERSION.SDK_INT >= 35 && glyphs.getFakeBold(at))
        put("skew", if (Build.VERSION.SDK_INT >= 35 && glyphs.getFakeItalic(at)) 14.04 else JSONObject.NULL)
        put("glyphs", list)
      })
      at = end
    }
  }

  /** The min-content width: the widest line-break segment, as the A engine would measure it. */
  private fun minContent(variant: Variant, paint: TextPaint): Float {
    val text = variant.text
    val breaks = BreakIterator.getLineInstance(Locale.forLanguageTag(variant.locale))
    breaks.setText(text)
    var widest = 0f
    var start = breaks.first()
    var end = breaks.next()

    while (end != BreakIterator.DONE) {
      var trimmed = end
      while (trimmed > start && text[trimmed - 1].isWhitespace()) trimmed--
      if (trimmed > start) widest = max(widest, Layout.getDesiredWidth(text, start, trimmed, paint))
      start = end
      end = breaks.next()
    }

    return widest
  }

  /** The record of [variant], its image drawn into [root] at [scale]. */
  fun record(variant: Variant, root: File, scale: Float): JSONObject {
    val notes = ArrayList<String>()
    val paint = paint(variant, variant.size * density)
    val width = widthPx(variant, paint)
    val begin = System.nanoTime()
    val layout = build(variant, paint, width, notes)
    val cold = ms(System.nanoTime() - begin)
    val warm = List(5) {
      val again = paint(variant, variant.size * density)
      val started = System.nanoTime()
      build(variant, again, widthPx(variant, again))
      ms(System.nanoTime() - started)
    }.sorted()[2]
    val count = visibleLines(variant, layout)
    val lines = JSONArray()
    var widest = 0f

    for (line in 0 until count) {
      val top = layout.getLineTop(line) / density
      val bottom = layout.getLineBottom(line) / density
      val ascent = -layout.getLineAscent(line) / density
      val descent = layout.getLineDescent(line) / density
      val lineWidth = layout.getLineMax(line) / density
      widest = max(widest, lineWidth)
      val end = layout.getLineEnd(line)
      lines.put(JSONObject().apply {
        put("utf16", JSONArray(listOf(layout.getLineStart(line), end)))
        put("x", (layout.getLineLeft(line) / density).toDouble())
        put("width", lineWidth.toDouble())
        put("top", top.toDouble())
        put("height", (bottom - top).toDouble())
        put("baseline", (layout.getLineBaseline(line) / density).toDouble())
        put("ascent", ascent.toDouble())
        put("descent", descent.toDouble())
        put("leading", (bottom - top - ascent - descent).toDouble())
        put("hyphen", drewHyphen(layout, line, variant.text, paint))
        put("ellipsis", layout.getEllipsisCount(line) > 0)
      })
    }

    val last = count - 1
    val truncated = variant.maxLines != null && layout.lineCount >= variant.maxLines &&
      (layout.getLineEnd(last) < variant.text.length || layout.getEllipsisCount(last) > 0)
    val height = layout.getLineBottom(last) / density
    val (runs, glyphRuns) = runsAndGlyphs(variant, layout, paint, count, notes)
    val record = JSONObject().apply {
      put("variant", variant.id)
      put("lines", lines)
      put("width", widest.toDouble())
      put("height", height.toDouble())
      put("truncated", truncated)
      put("runs", runs)
      put("intrinsic", JSONObject().apply {
        put("min", (minContent(variant, paint) / density).toDouble())
        put("max", (Layout.getDesiredWidth(variant.text, paint) / density).toDouble())
      })
      put("timings", JSONObject().put("cold_ms", cold).put("warm_ms", warm))
      if (glyphRuns.length() > 0) put("glyph_runs", glyphRuns)
      put("notes", JSONArray(notes))
    }

    val boxWidth = variant.width ?: widest
    val (wide, high) = imageSize(boxWidth, height, scale)
    val bitmap = Bitmap.createBitmap(wide, high, Bitmap.Config.ARGB_8888)
    bitmap.eraseColor(Color.WHITE)
    val canvas = Canvas(bitmap)
    canvas.translate(8f * scale, 8f * scale)
    canvas.scale(scale / density, scale / density)
    layout.draw(canvas)
    val name = "png/${variant.stem}@${scaleKey(scale)}x.png"
    writePng(bitmap, File(root, name))
    bitmap.recycle()
    record.put("images", JSONObject().put(scaleKey(scale), name))

    return record
  }

  /**
   * 1000 labels from the corpus's texts at 13 pixels, wrapped at 150 and again at 140, drawn
   * eight columns wide at scale 1: the harness's label timing.
   */
  fun labels(variants: List<Variant>): JSONObject {
    val cases = variants.filter { it.text.isNotEmpty() }.distinctBy { it.case }
    val count = 1000
    val paints = cases.associate { it.case to paint(it, 13f) }
    val params = cases.associate {
      it.case to PrecomputedText.Params.Builder(paints.getValue(it.case))
        .setBreakStrategy(Layout.BREAK_STRATEGY_SIMPLE)
        .setHyphenationFrequency(Layout.HYPHENATION_FREQUENCY_NONE)
        .build()
    }
    val picked = List(count) { cases[it % cases.size] }
    var started = System.nanoTime()
    val texts = picked.map { PrecomputedText.create(it.text, params.getValue(it.case)) }
    val build = ms(System.nanoTime() - started)

    fun layOut(width: Int) = picked.mapIndexed { at, variant ->
      StaticLayout.Builder.obtain(texts[at], 0, texts[at].length, paints.getValue(variant.case), width)
        .setIncludePad(false)
        .setUseLineSpacingFromFallbacks(true)
        .setBreakStrategy(Layout.BREAK_STRATEGY_SIMPLE)
        .setHyphenationFrequency(Layout.HYPHENATION_FREQUENCY_NONE)
        .build()
    }

    started = System.nanoTime()
    val laid = layOut(150)
    val layout = ms(System.nanoTime() - started)
    started = System.nanoTime()
    layOut(140)
    val relayout = ms(System.nanoTime() - started)
    val tops = FloatArray(8)
    val placed = laid.mapIndexed { at, label ->
      val slot = at % 8
      val y = tops[slot]
      tops[slot] += label.height + 4f
      Triple(label, slot * 160f, y)
    }
    val bitmap = Bitmap.createBitmap(160 * 8, ceil(tops.max()).toInt().coerceAtLeast(1), Bitmap.Config.ARGB_8888)
    bitmap.eraseColor(Color.WHITE)
    val canvas = Canvas(bitmap)
    started = System.nanoTime()

    for ((label, x, y) in placed) {
      canvas.save()
      canvas.translate(x, y)
      label.draw(canvas)
      canvas.restore()
    }

    val draw = ms(System.nanoTime() - started)
    bitmap.recycle()

    return JSONObject().apply {
      put("count", count)
      put("size", 13)
      put("width", 150)
      put("build_ms", build)
      put("layout_ms", layout)
      put("relayout_ms", relayout)
      put("draw_ms", draw)
      put("lines", laid.sumOf { it.lineCount })
    }
  }

  /** Every variant's record and image under [root], with the set's header and timings. */
  fun run(root: File, device: String): JSONObject {
    val variants = ParityCorpus.variants
    val first = paint(variants.first(), 17f * density)
    val started = System.nanoTime()
    StaticLayout.Builder.obtain("Hello", 0, 5, first, 1000).build()
    val firstLayout = ms(System.nanoTime() - started)
    val records = JSONArray()

    for (variant in variants) records.put(record(variant, root, density))

    val set = JSONObject().apply {
      put("schema", 1)
      put("corpus_version", ParityCorpus.version)
      put("engine", JSONObject().apply {
        put("name", "staticlayout")
        put("version", "Android ${Build.VERSION.RELEASE} (API $sdk)")
        put("rasterizer", "skia (software Canvas)")
        put("settings", JSONObject().apply {
          put("break_strategy", if (quality) "high_quality" else "simple")
          put("subpixel_text", subpixel.toString())
          put("hyphenation", if (quality) "normal" else "none")
          put("include_pad", "false")
          put("fallback_line_spacing", "true")
          put("density", density.toString())
          put("layout_pixels", "size times density")
        })
      })
      put("platform", JSONObject().apply {
        put("os", "android")
        put("os_version", "${Build.VERSION.RELEASE} (API $sdk)")
        put("device", device)
      })
      put("scales", JSONArray(listOf(density.toDouble())))
      put("timings", JSONObject().apply {
        put("first_layout_ms", firstLayout)
        put("labels", labels(variants))
      })
      put("records", records)
    }

    File(root, "records.json").writeText(set.toString(1))

    return set
  }
}
